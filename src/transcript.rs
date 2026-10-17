use serde::{Deserialize, Serialize};

/// A word-timed transcript, independent of the engine that produced it. Times are seconds from
/// the start of the episode's audio file.
#[derive(Debug, Deserialize, Serialize)]
pub struct Transcript {
    pub engine: String,
    pub model: String,
    pub language: String,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub words: Vec<Word>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Word {
    pub start: f64,
    pub end: f64,
    /// The word with its punctuation, without surrounding whitespace.
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probability: Option<f32>,
}

impl Transcript {
    pub fn words(&self) -> impl Iterator<Item = &Word> {
        self.segments.iter().flat_map(|segment| segment.words.iter())
    }

    /// Words that overlap `[start, end)`.
    pub fn words_between(&self, start: f64, end: f64) -> impl Iterator<Item = &Word> {
        self.words().filter(move |word| word.end > start && word.start < end)
    }

    /// The text spoken in `[start, end)`, joined with spaces.
    pub fn text_between(&self, start: f64, end: f64) -> String {
        self.words_between(start, end).map(|word| word.text.as_str()).collect::<Vec<_>>().join(" ")
    }
}
