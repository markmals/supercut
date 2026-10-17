//! mlx-whisper: Whisper on Apple silicon, driven through its command line.

use std::ffi::OsStr;
use std::path::Path;

use anyhow::Context as _;

use super::openai_json::{self, Made};
use crate::config::TranscriptionConfig;
use crate::error::{CliError, ErrorCode};
use crate::media;
use crate::transcript::Transcript;

const ENGINE: &str = "mlx-whisper";
const OUTPUT_NAME: &str = "transcript";

pub fn transcribe(config: &TranscriptionConfig, audio: &Path) -> anyhow::Result<Transcript> {
    let model = config.model();
    let scratch = tempfile::tempdir().context("making a scratch folder for mlx_whisper")?;
    media::run(
        "mlx_whisper",
        [
            OsStr::new("--model"),
            OsStr::new(model),
            OsStr::new("--language"),
            OsStr::new(&config.language),
            OsStr::new("--word-timestamps"),
            OsStr::new("True"),
            // Conditioning on the text so far is what sends Whisper into minutes-long loops.
            OsStr::new("--condition-on-previous-text"),
            OsStr::new("False"),
            OsStr::new("--output-format"),
            OsStr::new("json"),
            OsStr::new("--output-dir"),
            scratch.path().as_os_str(),
            OsStr::new("--output-name"),
            OsStr::new(OUTPUT_NAME),
            OsStr::new("--verbose"),
            OsStr::new("False"),
            audio.as_os_str(),
        ],
    )
    .with_context(|| {
        CliError::new(ErrorCode::ExternalTool, "mlx_whisper did not transcribe the episode")
            .hint("check `mlx_whisper --help` runs and that transcription.model names a model it can fetch")
    })?;
    let written = scratch.path().join(format!("{OUTPUT_NAME}.json"));
    let json = std::fs::read_to_string(&written)
        .with_context(|| format!("reading the transcript mlx_whisper wrote to {}", written.display()))?;
    openai_json::parse(&json, &Made { engine: ENGINE, model, language: &config.language })
}
