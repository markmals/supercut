//! whisper.cpp: the portable engine, driven through `whisper-cli`.

use std::ffi::OsStr;
use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;

use crate::config::TranscriptionConfig;
use crate::media;
use crate::transcript::{Transcript, Word};

const ENGINE: &str = "whisper-cpp";
const OUTPUT_NAME: &str = "transcript";

pub fn transcribe(config: &TranscriptionConfig, audio: &Path) -> anyhow::Result<Transcript> {
    let scratch = tempfile::tempdir().context("making a scratch folder for whisper-cli")?;
    let output_base = scratch.path().join(OUTPUT_NAME);
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
            // The full JSON is the only output that carries per-token times.
            OsStr::new("-ojf"),
            OsStr::new("-of"),
            output_base.as_os_str(),
            OsStr::new("-np"),
            OsStr::new("-f"),
            audio.as_os_str(),
        ],
    )?;
    let written = output_base.with_extension("json");
    let json = std::fs::read_to_string(&written)
        .with_context(|| format!("reading the transcript whisper-cli wrote to {}", written.display()))?;
    parse(&json, config)
}

/// Turns whisper.cpp's full JSON into the shared transcript. Whisper emits sub-word tokens; a
/// leading space is what marks the start of the next word.
pub fn parse(json: &str, config: &TranscriptionConfig) -> anyhow::Result<Transcript> {
    #[derive(Deserialize)]
    struct Report {
        transcription: Vec<RawSegment>,
    }
    #[derive(Deserialize)]
    struct RawSegment {
        #[serde(default)]
        tokens: Vec<RawToken>,
    }
    #[derive(Deserialize)]
    struct RawToken {
        text: String,
        offsets: Offsets,
        p: Option<f32>,
    }
    #[derive(Deserialize)]
    struct Offsets {
        from: f64,
        to: f64,
    }

    let report: Report = serde_json::from_str(json).context("parsing whisper-cli JSON")?;
    let segments = report
        .transcription
        .into_iter()
        .filter_map(|segment| {
            let mut words = Words::default();
            for token in segment.tokens.iter().filter(|token| !is_special(&token.text)) {
                words.push(&token.text, token.offsets.from / 1000.0, token.offsets.to / 1000.0, token.p);
            }
            super::segment_of(words.finish())
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

/// Timestamps and other control tokens whisper.cpp spells out, such as `[_BEG_]` and `[_TT_465]`.
fn is_special(text: &str) -> bool {
    text.starts_with("[_")
}

/// Words being glued together from sub-word tokens.
#[derive(Default)]
struct Words {
    done: Vec<Word>,
    text: String,
    start: f64,
    end: f64,
    probabilities: Vec<f32>,
}

impl Words {
    fn push(&mut self, token: &str, from: f64, to: f64, probability: Option<f32>) {
        if token.starts_with(' ') || self.text.is_empty() {
            self.close();
            self.start = from;
            self.end = to;
        }
        self.text.push_str(token);
        self.end = self.end.max(to);
        self.probabilities.extend(probability);
    }

    fn close(&mut self) {
        let mean = match self.probabilities.len() {
            0 => None,
            count => Some(self.probabilities.iter().sum::<f32>() / count as f32),
        };
        self.done.extend(super::word_of(&self.text, self.start, self.end, mean));
        self.text.clear();
        self.probabilities.clear();
    }

    fn finish(mut self) -> Vec<Word> {
        self.close();
        self.done
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{TranscriptionConfig, TranscriptionEngine};

    fn config() -> TranscriptionConfig {
        TranscriptionConfig {
            engine: TranscriptionEngine::WhisperCpp,
            model: "/tmp/ggml-base.en.bin".into(),
            language: "en".into(),
        }
    }

    fn token(text: &str, from: u32, to: u32, p: f32) -> String {
        format!(r#"{{"text": "{text}", "offsets": {{"from": {from}, "to": {to}}}, "id": 1, "p": {p}}}"#)
    }

    #[test]
    fn glues_sub_word_tokens_into_words_with_token_times() {
        let json = format!(
            r#"{{"transcription": [{{"text": " It's been longer.", "tokens": [{}]}}]}}"#,
            [
                token("[_BEG_]", 0, 0, 0.58),
                token(" It", 0, 120, 0.4),
                token("'s", 120, 240, 0.6),
                token(" been", 410, 700, 1.0),
                token(" longer", 700, 980, 1.0),
                token(".", 980, 1100, 1.0),
                token("[_TT_110]", 1100, 1100, 0.9),
            ]
            .join(",")
        );
        let transcript = super::parse(&json, &config()).unwrap();
        let words: Vec<&str> = transcript.words().map(|word| word.text.as_str()).collect();
        assert_eq!(words, ["It's", "been", "longer."]);
        let first = transcript.segments[0].words.first().unwrap();
        assert_eq!((first.start, first.end), (0.0, 0.24));
        assert_eq!(first.probability, Some(0.5));
        let last = transcript.words().last().unwrap();
        assert_eq!((last.start, last.end), (0.7, 1.1));
        assert_eq!(transcript.segments[0].text, "It's been longer.");
    }
}
