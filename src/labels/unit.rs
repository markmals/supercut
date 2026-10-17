//! A stretch of transcript put to Jev as one question, and how its text is written.

use crate::analysis::Envelope;
use crate::labels::QUIET_DB;
use crate::transcript::{UnreliableSpan, Word};

/// A stretch without words at least this long is described in the text.
const GAP_SECONDS: f64 = 1.5;

/// One labeling request's worth of transcript: a coarse window or a single utterance.
#[derive(Debug)]
pub struct Unit {
    /// Start of the unit's first word.
    pub start: f64,
    /// End of its last word.
    pub end: f64,
    /// The unit itself, long gaps described in place.
    pub text: String,
    /// What was said before it.
    pub before: String,
    /// What was said after it.
    pub after: String,
}

/// What the audio does where the words stop.
#[derive(Clone, Copy)]
pub struct Sound<'a> {
    pub envelope: &'a Envelope,
    pub unreliable: &'a [UnreliableSpan],
}

/// The unit covering `words[first..=last]`, quoting `context` seconds of talk either side.
pub fn unit(words: &[&Word], first: usize, last: usize, context: f64, sound: Sound<'_>) -> Unit {
    let lead_in = if first == 0 { 0.0 } else { words[first - 1].end };
    let mut text = String::new();
    push_gap(&mut text, lead_in, words[first].start, sound);
    push_words(&mut text, &words[first..=last], sound);
    if last + 1 == words.len() {
        push_gap(&mut text, words[last].end, sound.envelope.duration(), sound);
    }
    Unit {
        start: words[first].start,
        end: words[last].end,
        text,
        before: render(&words[context_start(words, first, context)..first], sound),
        after: render(&words[last + 1..context_end(words, last, context)], sound),
    }
}

/// First word of the context quoted before the unit.
fn context_start(words: &[&Word], first: usize, context: f64) -> usize {
    let earliest = words[first].start - context;
    words[..first].iter().position(|word| word.end >= earliest).unwrap_or(first)
}

/// One past the last word of the context quoted after the unit.
fn context_end(words: &[&Word], last: usize, context: f64) -> usize {
    let latest = words[last].end + context;
    let after = &words[last + 1..];
    last + 1 + after.iter().position(|word| word.start > latest).unwrap_or(after.len())
}

fn render(words: &[&Word], sound: Sound<'_>) -> String {
    let mut text = String::new();
    push_words(&mut text, words, sound);
    text
}

fn push_words(text: &mut String, words: &[&Word], sound: Sound<'_>) {
    for (index, word) in words.iter().enumerate() {
        if index > 0 {
            push_gap(text, words[index - 1].end, word.start, sound);
        }
        push_part(text, &word.text);
    }
}

/// Describes `start`..`end` when it is long enough to matter: the engine drops out there, or
/// music plays, or nobody is talking.
fn push_gap(text: &mut String, start: f64, end: f64, sound: Sound<'_>) {
    let seconds = end - start;
    if seconds < GAP_SECONDS {
        return;
    }
    let rounded = seconds.round() as i64;
    match sound.unreliable.iter().find(|span| span.end > start && span.start < end) {
        Some(span) => push_part(text, &format!("[{rounded} s dropped: {}]", span.reason)),
        None if sound.envelope.mean_db(start, end) > QUIET_DB => {
            push_part(text, &format!("[{rounded} s of music/sound]"));
        }
        None => push_part(text, &format!("[{rounded} s of silence]")),
    }
}

fn push_part(text: &mut String, part: &str) {
    if !text.is_empty() {
        text.push(' ');
    }
    text.push_str(part);
}
