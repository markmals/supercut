//! mlx-whisper: Whisper on Apple silicon, driven through its command line.

use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;

use crate::config::TranscriptionConfig;
use crate::error::{CliError, ErrorCode};
use crate::media;
use crate::transcript::Transcript;

const ENGINE: &str = "mlx-whisper";
const OUTPUT_NAME: &str = "transcript";

pub fn transcribe(config: &TranscriptionConfig, audio: &Path) -> anyhow::Result<Transcript> {
    let model = config.model()?;
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

    let report: Report = serde_json::from_str(&as_json(json)).context("parsing mlx_whisper JSON")?;
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
        model: config.model()?.to_owned(),
        language: config.language.clone(),
        unreliable: Vec::new(),
        segments,
    })
}

/// mlx-whisper writes Python's `NaN`, `Infinity` and `-Infinity` for segment scores it could
/// not compute, which JSON has no words for. Outside strings they can only stand where a
/// number is allowed, so `null` says the same thing to a parser.
fn as_json(raw: &str) -> Cow<'_, str> {
    const NON_NUMBERS: [&str; 3] = ["NaN", "-Infinity", "Infinity"];
    if !NON_NUMBERS.iter().any(|word| raw.contains(word)) {
        return Cow::Borrowed(raw);
    }
    let mut cleaned = String::with_capacity(raw.len());
    let mut inside_string = false;
    let mut escaped = false;
    let mut skip = 0;
    for (position, character) in raw.char_indices() {
        if skip > 0 {
            skip -= 1;
            continue;
        }
        if character == '"' && !escaped {
            inside_string = !inside_string;
        }
        escaped = inside_string && character == '\\' && !escaped;
        match NON_NUMBERS.iter().find(|word| !inside_string && raw[position..].starts_with(**word)) {
            Some(word) => {
                cleaned.push_str("null");
                skip = word.len() - 1;
            }
            None => cleaned.push(character),
        }
    }
    Cow::Owned(cleaned)
}

#[cfg(test)]
mod tests {
    use crate::config::{TranscriptionConfig, TranscriptionEngine};

    fn config() -> TranscriptionConfig {
        TranscriptionConfig {
            engine: TranscriptionEngine::MlxWhisper,
            model: Some("mlx-community/whisper-large-v3-turbo".into()),
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

    #[test]
    fn reads_segments_whose_score_mlx_wrote_as_nan() {
        let json = r#"{"segments": [
          {"start": 0.0, "end": 0.4, "avg_logprob": NaN, "text": " NaN.",
           "words": [{"word": " NaN.", "start": 0.0, "end": 0.4, "probability": 0.5}]}
        ]}"#;
        let transcript = super::parse(json, &config()).unwrap();
        assert_eq!(transcript.segments[0].text, "NaN.");
    }
}
