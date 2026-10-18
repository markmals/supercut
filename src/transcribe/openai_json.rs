//! The JSON OpenAI's Whisper writes, which mlx-whisper and faster-whisper (through
//! whisper-ctranslate2) both copy: segments of words, each with its own start, end and
//! probability.

use std::borrow::Cow;

use anyhow::Context as _;
use serde::Deserialize;

use crate::transcript::Transcript;

/// Who made a transcript, recorded in it.
pub struct Made<'a> {
    pub engine: &'a str,
    pub model: &'a str,
    pub language: &'a str,
}

/// Turns Whisper's JSON into the shared transcript, keeping only words with text.
pub fn parse(json: &str, made: &Made) -> anyhow::Result<Transcript> {
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

    let report: Report = serde_json::from_str(&as_json(json))
        .with_context(|| format!("parsing {}'s JSON", made.engine))?;
    let segments = report
        .segments
        .into_iter()
        .filter_map(|segment| {
            let words = segment
                .words
                .into_iter()
                .filter_map(|word| {
                    super::word_of(&word.word, word.start, word.end, word.probability)
                })
                .collect();
            super::segment_of(words)
        })
        .collect();
    Ok(Transcript {
        engine: made.engine.to_owned(),
        model: made.model.to_owned(),
        language: made.language.to_owned(),
        unreliable: Vec::new(),
        segments,
    })
}

/// Python writes `NaN`, `Infinity` and `-Infinity` for segment scores it could not compute,
/// which JSON has no words for. Outside strings they can only stand where a number is allowed,
/// so `null` says the same thing to a parser.
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
        match NON_NUMBERS.iter().find(|word| !inside_string && raw[position..].starts_with(**word))
        {
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
    use super::{Made, parse};

    const MADE: Made = Made {
        engine: "mlx-whisper",
        model: "mlx-community/whisper-large-v3-turbo",
        language: "en",
    };

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
        let transcript = parse(REPORT, &MADE).unwrap();
        assert_eq!(transcript.segments.len(), 1);
        let words: Vec<&str> = transcript.words().map(|word| word.text.as_str()).collect();
        assert_eq!(words, ["Hi,", "I'm", "Tompkins."]);
        let first = transcript.segments[0].words.first().unwrap();
        assert_eq!((first.start, first.end), (0.0, 0.32));
        assert_eq!(transcript.segments[0].text, "Hi, I'm Tompkins.");
        assert_eq!((transcript.segments[0].start, transcript.segments[0].end), (0.0, 1.36));
    }

    #[test]
    fn reads_segments_whose_score_python_wrote_as_nan() {
        let json = r#"{"segments": [
          {"start": 0.0, "end": 0.4, "avg_logprob": NaN, "text": " NaN.",
           "words": [{"word": " NaN.", "start": 0.0, "end": 0.4, "probability": 0.5}]}
        ]}"#;
        let transcript = parse(json, &MADE).unwrap();
        assert_eq!(transcript.segments[0].text, "NaN.");
    }
}
