mod envelope;
pub mod moment;

use std::path::Path;
use std::process::Command;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

pub use envelope::{Envelope, FRAME_SECONDS};

use crate::library::{Episode, ShowLibrary};
use crate::media::{PcmStream, Probe};

const ENVELOPE_SAMPLE_RATE: u32 = 16_000;
const SAMPLES_PER_FRAME: usize = (ENVELOPE_SAMPLE_RATE as f64 * FRAME_SECONDS) as usize;

/// What one decode of an audio file tells us, cached per episode in `analysis/<id>.json`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioAnalysis {
    pub probe: Probe,
    /// Decoded length in seconds; exact, unlike the container's estimate.
    pub duration: f64,
    pub loudness: Loudness,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Loudness {
    /// EBU R128 integrated loudness, in LUFS.
    pub integrated_lufs: f64,
    /// Loudness range, in LU.
    pub range_lu: f64,
}

/// The episode's cached analysis, computed on first use.
pub fn episode_analysis(show: &ShowLibrary, episode: &Episode) -> anyhow::Result<AudioAnalysis> {
    let path = show.analysis_path(&episode.id);
    if path.exists() && show.envelope_path(&episode.id).exists() {
        return crate::library::read_json(&path);
    }
    eprintln!("analyzing {} ({})", episode.id, episode.title);
    let (analysis, envelope) = analyze_file(&show.audio_path(episode)?)?;
    envelope.save(&show.envelope_path(&episode.id))?;
    crate::library::write_json_atomically(&path, &analysis)?;
    Ok(analysis)
}

/// The episode's level envelope, analyzing the episode first if needed.
pub fn episode_envelope(show: &ShowLibrary, episode: &Episode) -> anyhow::Result<Envelope> {
    episode_analysis(show, episode)?;
    Envelope::load(&show.envelope_path(&episode.id))
}

/// Decodes `path` once, measuring loudness and the level envelope together.
pub fn analyze_file(path: &Path) -> anyhow::Result<(AudioAnalysis, Envelope)> {
    let probe = crate::media::probe(path)?;
    let mut command = Command::new("ffmpeg");
    command
        .args(["-nostdin", "-hide_banner", "-nostats", "-v", "info", "-i"])
        .arg(path)
        .arg("-filter_complex")
        .arg(format!(
            "[0:a:0]asplit=2[l][e];[l]ebur128=framelog=quiet[lo];\
             [e]aresample={ENVELOPE_SAMPLE_RATE},aformat=sample_fmts=flt:channel_layouts=mono[eo]"
        ))
        .args(["-map", "[lo]", "-f", "null", "-", "-map", "[eo]", "-f", "f32le", "pipe:1"]);
    let mut stream = PcmStream::spawn(command)?;

    let mut mean_squares = Vec::new();
    let mut chunk = vec![0.0f32; SAMPLES_PER_FRAME * 512];
    let mut pending: Vec<f32> = Vec::with_capacity(SAMPLES_PER_FRAME);
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        for &sample in &chunk[..count] {
            pending.push(sample);
            if pending.len() == SAMPLES_PER_FRAME {
                mean_squares.push(mean_square(&pending));
                pending.clear();
            }
        }
    }
    let log = stream.finish()?;
    let decoded_samples = mean_squares.len() * SAMPLES_PER_FRAME + pending.len();
    if !pending.is_empty() {
        mean_squares.push(mean_square(&pending));
    }
    let analysis = AudioAnalysis {
        probe,
        duration: decoded_samples as f64 / f64::from(ENVELOPE_SAMPLE_RATE),
        loudness: parse_ebur128_summary(&log).with_context(|| format!("measuring loudness of {}", path.display()))?,
    };
    Ok((analysis, Envelope::from_mean_squares(mean_squares)))
}

fn mean_square(samples: &[f32]) -> f64 {
    samples.iter().map(|sample| f64::from(*sample).powi(2)).sum::<f64>() / samples.len() as f64
}

/// Reads `I:` and `LRA:` from the summary the ebur128 filter logs when it finishes.
fn parse_ebur128_summary(log: &str) -> anyhow::Result<Loudness> {
    let summary = log.rsplit_once("Summary:").context("ffmpeg logged no ebur128 summary")?.1;
    let value = |label: &str| -> anyhow::Result<f64> {
        let line = summary
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with(label))
            .with_context(|| format!("no `{label}` in the ebur128 summary"))?;
        let number = line[label.len()..].split_whitespace().next().unwrap_or_default();
        number.parse().with_context(|| format!("unreadable `{line}` in the ebur128 summary"))
    };
    Ok(Loudness { integrated_lufs: value("I:")?, range_lu: value("LRA:")? })
}

#[cfg(test)]
mod tests {
    use super::parse_ebur128_summary;

    #[test]
    fn reads_integrated_loudness_and_range_from_the_summary() {
        let log = "[Parsed_ebur128_0 @ 0x1] Summary:\n\n  Integrated loudness:\n    I:         -16.4 LUFS\n    \
                   Threshold: -26.6 LUFS\n\n  Loudness range:\n    LRA:         6.1 LU\n    Threshold:  -36.7 LUFS\n";
        let loudness = parse_ebur128_summary(log).unwrap();
        assert_eq!(loudness.integrated_lufs, -16.4);
        assert_eq!(loudness.range_lu, 6.1);
    }
}
