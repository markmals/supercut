//! faster-whisper: Whisper on CTranslate2, for any platform's CPU, driven through the
//! whisper-ctranslate2 command line. It times words the way OpenAI's Whisper does, start and end
//! alike, and writes the same JSON.

use std::ffi::OsStr;
use std::path::Path;

use anyhow::Context as _;

use super::openai_json::{self, Made};
use crate::config::TranscriptionConfig;
use crate::error::{CliError, ErrorCode};
use crate::media;
use crate::transcript::Transcript;

const ENGINE: &str = "faster-whisper";

pub fn transcribe(config: &TranscriptionConfig, audio: &Path) -> anyhow::Result<Transcript> {
    let model = config.model();
    let scratch = tempfile::tempdir().context("making a scratch folder for whisper-ctranslate2")?;
    media::run(
        "whisper-ctranslate2",
        [
            audio.as_os_str(),
            OsStr::new("--model"),
            OsStr::new(model),
            OsStr::new("--language"),
            OsStr::new(&config.language),
            // CTranslate2 picks the fastest number format and thread count for the machine: on a
            // 16-core i9, 8-bit weights on its own thread count beat full precision by 1.8 times
            // and every hyperthread by 1.5 times, with the same words.
            OsStr::new("--compute_type"),
            OsStr::new("auto"),
            OsStr::new("--word_timestamps"),
            OsStr::new("True"),
            // Conditioning on the text so far is what sends Whisper into minutes-long loops.
            OsStr::new("--condition_on_previous_text"),
            OsStr::new("False"),
            OsStr::new("--output_format"),
            OsStr::new("json"),
            OsStr::new("--output_dir"),
            scratch.path().as_os_str(),
            OsStr::new("--verbose"),
            OsStr::new("False"),
        ],
    )
    .with_context(|| {
        CliError::new(ErrorCode::ExternalTool, "whisper-ctranslate2 did not transcribe the episode")
            .hint("check `whisper-ctranslate2 --help` runs and that transcription.model is a model size it knows, e.g. large-v3-turbo")
    })?;
    let stem = audio.file_stem().context("the audio file has no name")?.to_string_lossy();
    let written = scratch.path().join(format!("{stem}.json"));
    let json = std::fs::read_to_string(&written).with_context(|| {
        format!("reading the transcript whisper-ctranslate2 wrote to {}", written.display())
    })?;
    openai_json::parse(&json, &Made { engine: ENGINE, model, language: &config.language })
}
