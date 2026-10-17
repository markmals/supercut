//! whisper.cpp: the portable engine, driven through `whisper-cli`.

use std::ffi::OsStr;
use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;

use crate::config::TranscriptionConfig;
use crate::error::{CliError, ErrorCode};
use crate::media;
use crate::transcript::Transcript;

mod words;

const ENGINE: &str = "whisper-cpp";
const OUTPUT_NAME: &str = "transcript";

pub fn transcribe(config: &TranscriptionConfig, audio: &Path) -> anyhow::Result<Transcript> {
    let scratch = tempfile::tempdir().context("making a scratch folder for whisper-cli")?;
    let output_base = scratch.path().join(OUTPUT_NAME);
    let alignment = alignment_preset(&config.model)?;
    media::run(
        "whisper-cli",
        [
            OsStr::new("-m"),
            OsStr::new(&config.model),
            OsStr::new("-l"),
            OsStr::new(&config.language),
            // Carrying context between windows is what sends Whisper into minutes-long loops.
            OsStr::new("-mc"),
            OsStr::new("0"),
            // Token times straight from the decoder are seconds off; aligning the attention
            // weights (which flash attention does not keep) puts them on the words.
            OsStr::new("-dtw"),
            OsStr::new(&alignment),
            OsStr::new("-nfa"),
            // The full JSON is the only output that carries per-token times.
            OsStr::new("-ojf"),
            OsStr::new("-of"),
            output_base.as_os_str(),
            OsStr::new("-np"),
            OsStr::new("-f"),
            audio.as_os_str(),
        ],
    )
    .with_context(|| {
        CliError::new(ErrorCode::ExternalTool, "whisper-cli did not transcribe the episode")
            .hint(format!(
                "check whisper-cli is installed, transcription.model is a ggml model file and `-dtw {alignment}` names its size"
            ))
    })?;
    let written = output_base.with_extension("json");
    let json = std::fs::read_to_string(&written)
        .with_context(|| format!("reading the transcript whisper-cli wrote to {}", written.display()))?;
    parse(&json, config)
}

/// The model size `-dtw` wants, read off the model file name: `ggml-base.en-q5_0.bin` is
/// `base.en`.
fn alignment_preset(model: &str) -> anyhow::Result<String> {
    let stem = Path::new(model)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| CliError::invalid(format!("transcription.model `{model}` is not a model file name")))?;
    let name = stem.strip_prefix("ggml-").unwrap_or(stem);
    let size = match name.split_once("-q") {
        Some((size, _quantization)) => size,
        None => name,
    };
    Ok(size.to_owned())
}

/// Turns whisper.cpp's full JSON into the shared transcript.
pub fn parse(json: &str, config: &TranscriptionConfig) -> anyhow::Result<Transcript> {
    let report: Report = serde_json::from_str(json).context("parsing whisper-cli JSON")?;
    let mut segments = Vec::with_capacity(report.transcription.len());
    for segment in &report.transcription {
        segments.extend(super::segment_of(words::of(segment)?));
    }
    Ok(Transcript {
        engine: ENGINE.to_owned(),
        model: config.model.clone(),
        language: config.language.clone(),
        unreliable: Vec::new(),
        segments,
    })
}

#[derive(Deserialize)]
struct Report {
    transcription: Vec<Segment>,
}

#[derive(Deserialize)]
struct Segment {
    offsets: Offsets,
    #[serde(default)]
    tokens: Vec<Token>,
}

#[derive(Deserialize)]
struct Token {
    text: String,
    /// Where the aligned attention weights put the token, in hundredths of a second; `-1` for
    /// the control tokens, which carry no speech.
    t_dtw: f64,
    p: Option<f32>,
}

#[derive(Deserialize)]
struct Offsets {
    /// Milliseconds from the start of the audio.
    to: f64,
}

#[cfg(test)]
mod tests {
    use crate::config::{TranscriptionConfig, TranscriptionEngine};

    #[test]
    fn reads_the_model_size_off_the_file_name() {
        assert_eq!(super::alignment_preset("/tmp/ggml-base.en.bin").unwrap(), "base.en");
        assert_eq!(super::alignment_preset("~/models/ggml-large-v3-turbo-q5_0.bin").unwrap(), "large-v3-turbo");
    }

    pub(super) fn config() -> TranscriptionConfig {
        TranscriptionConfig {
            engine: TranscriptionEngine::WhisperCpp,
            model: "/tmp/ggml-base.en.bin".into(),
            language: "en".into(),
        }
    }
}
