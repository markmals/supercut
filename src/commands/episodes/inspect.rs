use std::path::PathBuf;

use crate::analysis::moment;
use crate::commands::Context;
use crate::error::{CliError, ErrorCode};
use crate::output::Format;
use crate::timecode::Time;
use crate::transcript::Transcript;

/// Show the audio around a moment of an episode: levels, words and quiet spans, for placing cuts.
///
/// Levels are RMS dBFS of the mono mix per `--step`. `non_speech` lists the stretches without
/// words, `silence` or `sound` (music, laughter, effects) by level; a cut edge inside `sound`
/// needs a fade or a move to the stretch's edge. `--spectrogram` also writes a PNG of the
/// window: music shows as steady horizontal lines, speech as stacked moving bands.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug.
    show: String,
    /// Episode id.
    id: String,
    /// The moment to look at (`h:mm:ss.sss`, `m:ss` or seconds).
    #[arg(long)]
    at: Time,
    /// Seconds shown, centred on `--at`.
    #[arg(long, default_value_t = 20.0)]
    window: f64,
    /// Seconds per level reading.
    #[arg(long, default_value_t = 0.1)]
    step: f64,
    /// Level, in dBFS, below which audio counts as silence.
    #[arg(long, default_value_t = -45.0, allow_hyphen_values = true)]
    quiet_db: f32,
    /// Also write a spectrogram PNG of the window here.
    #[arg(long)]
    spectrogram: Option<PathBuf>,
    /// `json` (default) or `text` (one line per level step with a bar and the words).
    #[arg(long, value_enum, default_value_t)]
    output: Format,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    if args.window <= 0.0 || args.step <= 0.0 {
        return Err(CliError::invalid("`--window` and `--step` must be positive").into());
    }
    let (_, show) = context.show(&args.show)?;
    let index = show.load_synced_index()?;
    let episode = index.get(&args.id)?;
    let envelope = crate::analysis::episode_envelope(&show, episode)?;
    let transcript_path = show.transcript_path(&episode.id);
    let transcript: Option<Transcript> = if transcript_path.exists() {
        Some(crate::library::read_json(&transcript_path)?)
    } else {
        None
    };
    let from = (args.at.seconds() - args.window / 2.0).max(0.0);
    let to = (args.at.seconds() + args.window / 2.0).min(envelope.duration());
    if let Some(path) = &args.spectrogram {
        write_spectrogram(&show.audio_path(episode)?, from, to - from, path)?;
    }
    let moment =
        moment::inspect(&envelope, transcript.as_ref(), from, to, args.step, args.quiet_db);
    match args.output {
        Format::Json => crate::output::print_json(&moment),
        Format::Text => {
            println!("{}", moment::text(&moment));
            Ok(())
        }
    }
}

fn write_spectrogram(
    audio: &std::path::Path,
    start: f64,
    duration: f64,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-y",
            "-ss",
            &format!("{start:.3}"),
            "-t",
            &format!("{duration:.3}"),
            "-i",
        ])
        .arg(audio)
        .args([
            "-lavfi",
            "showspectrumpic=s=1200x480:legend=1:scale=log:fscale=log:stop=8000",
            "-frames:v",
            "1",
        ])
        .arg(path)
        .output()?;
    if !output.status.success() {
        let message = format!(
            "ffmpeg could not draw the spectrogram: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
        return Err(CliError::new(ErrorCode::ExternalTool, message).into());
    }
    eprintln!("spectrogram written to {}", path.display());
    Ok(())
}
