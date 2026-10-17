//! mlx-whisper: Whisper on Apple silicon, driven through its command line.

use std::ffi::OsStr;
use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;

use crate::config::TranscriptionConfig;
use crate::media;
use crate::transcript::Transcript;

const ENGINE: &str = "mlx-whisper";
const OUTPUT_NAME: &str = "transcript";

pub fn transcribe(config: &TranscriptionConfig, audio: &Path) -> anyhow::Result<Transcript> {
    let scratch = tempfile::tempdir().context("making a scratch folder for mlx_whisper")?;
    media::run(
        "mlx_whisper",
        [
            OsStr::new("--model"),
            OsStr::new(&config.model),
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
    )?;
    let written = scratch.path().join(format!("{OUTPUT_NAME}.json"));
    let json = std::fs::read_to_string(&written)
        .with_context(|| format!("reading the transcript mlx_whisper wrote to {}", written.display()))?;
    parse(&json, config)
}

/// Turns mlx-whisper's JSON into the shared transcript, keeping only words with text.
pub fn parse(json: &str, config: &TranscriptionConfig) -> anyhow::Result<Transcript> {
    #[derive(Deserialize)]
    struct Report {
        segments: Vec<RawSegment>,
    }
    #[derive(Deserialize)]
    struct RawSegment {
        #[serde(default)]
        words: Vec<RawWord>,
    }
    #[derive(Deserialize)]
    struct RawWord {
        word: String,
        start: f64,
        end: f64,
        probability: Option<f32>,
    }

    let report: Report = serde_json::from_str(json).context("parsing mlx_whisper JSON")?;
    let segments = report
        .segments
        .into_iter()
        .filter_map(|segment| {
            let words = segment
                .words
                .into_iter()
                .filter_map(|word| super::word_of(&word.word, word.start, word.end, word.probability))
                .collect();
            super::segment_of(words)
        })
        .collect();
    Ok(Transcript {
        engine: ENGINE.to_owned(),
        model: config.model.clone(),
        language: config.language.clone(),
        unreliable: Vec::new(),
        segments,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::{TranscriptionConfig, TranscriptionEngine};

    fn config() -> TranscriptionConfig {
        TranscriptionConfig {
            engine: TranscriptionEngine::MlxWhisper,
            model: "mlx-community/whisper-large-v3-turbo".into(),
            language: "en".into(),
        }
    }

    const REPORT: &str = r#"{
      "text": " Hi, I'm Paul F. Tompkins.",
      "language": "en",
      "segments": [
        {
          "id": 0, "start": 0.0, "end": 1.36, "text": " Hi, I'm Paul F. Tompkins.",
          "words": [
            {"word": " Hi,", "start": 0.0, "end": 0.32, "probability": 0.93896484375},
            {"word": " I'm", "start": 0.46, "end": 0.56, "probability": 1.0},
            {"word": " ", "start": 0.56, "end": 0.56, "probability": 0.5},
            {"word": " Tompkins.", "start": 0.98, "end": 1.36, "probability": 0.99560546875}
          ]
        },
        {"id": 1, "start": 1.36, "end": 1.36, "text": "", "words": []}
      ]
    }"#;

    #[test]
    fn keeps_word_timings_and_drops_empty_words_and_segments() {
        let transcript = super::parse(REPORT, &config()).unwrap();
        assert_eq!(transcript.segments.len(), 1);
        let words: Vec<&str> = transcript.words().map(|word| word.text.as_str()).collect();
        assert_eq!(words, ["Hi,", "I'm", "Tompkins."]);
        let first = transcript.segments[0].words.first().unwrap();
        assert_eq!((first.start, first.end), (0.0, 0.32));
        assert_eq!(transcript.segments[0].text, "Hi, I'm Tompkins.");
        assert_eq!((transcript.segments[0].start, transcript.segments[0].end), (0.0, 1.36));
    }
}
