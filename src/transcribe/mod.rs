//! Speech-to-text engines.

mod mlx;
mod repetition;
mod view;
mod whisper_cpp;

use std::path::Path;

use crate::config::{TranscriptionConfig, TranscriptionEngine};
use crate::transcript::{Segment, Transcript, Word};

pub use view::{Utterance, render, utterances};

/// Transcribes one audio file with the configured engine and strips the loops the engine fell
/// into, which are recorded as unreliable spans.
pub fn transcribe_file(config: &TranscriptionConfig, audio: &Path) -> anyhow::Result<Transcript> {
    let mut transcript = match config.engine {
        TranscriptionEngine::MlxWhisper => mlx::transcribe(config, audio)?,
        TranscriptionEngine::WhisperCpp => whisper_cpp::transcribe(config, audio)?,
    };
    repetition::strip_loops(&mut transcript);
    Ok(transcript)
}

/// The segment `words` make up, or `None` when no word is left to cover.
fn segment_of(words: Vec<Word>) -> Option<Segment> {
    let start = words.first()?.start;
    let end = words.iter().map(|word| word.end).fold(start, f64::max);
    let text = words.iter().map(|word| word.text.as_str()).collect::<Vec<_>>().join(" ");
    Some(Segment { start, end, text, words })
}

/// A word with its text trimmed, or `None` when the engine emitted whitespace.
fn word_of(text: &str, start: f64, end: f64, probability: Option<f32>) -> Option<Word> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(Word { start, end: end.max(start), text: text.to_owned(), probability })
}
