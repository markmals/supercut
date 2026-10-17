mod pcm;

use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};

pub use pcm::{PcmStream, decode_window};

/// Stream and container facts about an audio file.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Probe {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u32,
    /// Bits per second of the audio stream, when the container states it.
    pub bit_rate: Option<u64>,
    /// Seconds, as the container reports it (estimated from the bit rate for some MP3s).
    pub duration: f64,
}

pub fn probe(path: &Path) -> anyhow::Result<Probe> {
    #[derive(Deserialize)]
    struct Report {
        streams: Vec<Stream>,
        format: Format,
    }
    #[derive(Deserialize)]
    struct Stream {
        codec_name: String,
        sample_rate: String,
        channels: u32,
        bit_rate: Option<String>,
    }
    #[derive(Deserialize)]
    struct Format {
        duration: String,
        bit_rate: Option<String>,
    }

    let output = run(
        "ffprobe",
        [
            OsStr::new("-v"),
            OsStr::new("error"),
            OsStr::new("-select_streams"),
            OsStr::new("a:0"),
            OsStr::new("-show_entries"),
            OsStr::new("stream=codec_name,sample_rate,channels,bit_rate:format=duration,bit_rate"),
            OsStr::new("-of"),
            OsStr::new("json"),
            path.as_os_str(),
        ],
    )?;
    let report: Report = serde_json::from_slice(&output.stdout).context("parsing ffprobe output")?;
    let stream = report.streams.into_iter().next().with_context(|| format!("{} has no audio stream", path.display()))?;
    let bit_rate = stream.bit_rate.or(report.format.bit_rate).and_then(|rate| rate.parse().ok());
    Ok(Probe {
        codec: stream.codec_name,
        sample_rate: stream.sample_rate.parse().context("ffprobe sample rate")?,
        channels: stream.channels,
        bit_rate,
        duration: report.format.duration.parse().context("ffprobe duration")?,
    })
}

/// Runs `program` to completion; a non-zero exit becomes an error carrying its stderr.
pub fn run<I, S>(program: &str, args: I) -> anyhow::Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("running {program}; is it installed and on PATH?"))?;
    if !output.status.success() {
        bail!("{program} failed ({}): {}", output.status, String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(output)
}
