//! A close look at a few seconds of an episode: levels, words, and the stretches without words,
//! told apart into silence and sound (music, laughter, effects). For placing cut edges.

use serde::Serialize;

use super::Envelope;
use crate::timecode::Time;
use crate::transcript::Transcript;

/// Pauses between words shorter than this are ordinary speech rhythm.
const MIN_NON_SPEECH_SECONDS: f64 = 0.3;
/// Quiet stretches shorter than this are too short to hide a cut in.
const MIN_QUIET_SECONDS: f64 = 0.15;

#[derive(Serialize)]
pub struct Moment {
    pub from: Time,
    pub to: Time,
    pub quiet_threshold_db: f32,
    pub transcribed: bool,
    pub levels: Vec<Level>,
    pub words: Vec<TimedWord>,
    pub non_speech: Vec<NonSpeech>,
    pub quiet_spans: Vec<Span>,
}

#[derive(Serialize)]
pub struct Level {
    pub at: Time,
    pub db: f32,
}

#[derive(Serialize)]
pub struct TimedWord {
    pub start: Time,
    pub end: Time,
    pub text: String,
}

#[derive(Serialize)]
pub struct NonSpeech {
    pub start: Time,
    pub end: Time,
    pub seconds: f64,
    /// `silence` or `sound` (music, laughter, effects) by mean level, or `unreliable-transcript`
    /// where the transcriber garbled the audio and words may be missing.
    pub kind: &'static str,
    pub mean_db: f32,
}

#[derive(Serialize)]
pub struct Span {
    pub start: Time,
    pub end: Time,
}

pub fn inspect(envelope: &Envelope, transcript: Option<&Transcript>, from: f64, to: f64, step: f64, quiet_db: f32) -> Moment {
    let levels = (0..((to - from) / step).ceil() as usize)
        .map(|index| from + index as f64 * step)
        .map(|at| Level { at: Time(at), db: round_db(envelope.mean_db(at, at + step)) })
        .collect();
    let words: Vec<TimedWord> = transcript
        .map(|transcript| {
            transcript
                .words_between(from, to)
                .map(|word| TimedWord { start: Time(word.start), end: Time(word.end), text: word.text.clone() })
                .collect()
        })
        .unwrap_or_default();
    let non_speech = transcript.map(|transcript| non_speech(envelope, transcript, &words, from, to, quiet_db)).unwrap_or_default();
    let quiet_spans = envelope
        .quiet_spans(from, to, quiet_db, MIN_QUIET_SECONDS)
        .into_iter()
        .map(|(start, end)| Span { start: Time(start), end: Time(end) })
        .collect();
    Moment { from: Time(from), to: Time(to), quiet_threshold_db: quiet_db, transcribed: transcript.is_some(), levels, words, non_speech, quiet_spans }
}

/// The gaps between words (and before the first / after the last) inside the window. A gap
/// the transcript marks unreliable may hold speech it lost, so it is reported as such.
fn non_speech(envelope: &Envelope, transcript: &Transcript, words: &[TimedWord], from: f64, to: f64, quiet_db: f32) -> Vec<NonSpeech> {
    let mut edges = vec![from];
    for word in words {
        edges.push(word.start.seconds());
        edges.push(word.end.seconds());
    }
    edges.push(to);
    edges
        .chunks_exact(2)
        .filter(|pair| pair[1] - pair[0] >= MIN_NON_SPEECH_SECONDS)
        .map(|pair| {
            let mean_db = round_db(envelope.mean_db(pair[0], pair[1]));
            let kind = if transcript.unreliable_between(pair[0], pair[1]).next().is_some() {
                "unreliable-transcript"
            } else if mean_db < quiet_db {
                "silence"
            } else {
                "sound"
            };
            NonSpeech { start: Time(pair[0]), end: Time(pair[1]), seconds: ((pair[1] - pair[0]) * 1000.0).round() / 1000.0, kind, mean_db }
        })
        .collect()
}

fn round_db(db: f32) -> f32 {
    (db * 10.0).round() / 10.0
}

/// One line per level step: time, level, a bar, and the words starting in that step.
pub fn text(moment: &Moment) -> String {
    let mut lines = Vec::with_capacity(moment.levels.len() + moment.non_speech.len() + 2);
    lines.push(format!("{} to {} (quiet below {} dBFS)", moment.from, moment.to, moment.quiet_threshold_db));
    let step = moment.levels.get(1).map_or(0.1, |level| level.at.seconds() - moment.levels[0].at.seconds());
    for level in &moment.levels {
        let at = level.at.seconds();
        let spoken: Vec<&str> = moment
            .words
            .iter()
            .filter(|word| word.start.seconds() >= at && word.start.seconds() < at + step)
            .map(|word| word.text.as_str())
            .collect();
        let bar = "#".repeat(((level.db + 60.0).max(0.0) / 2.0) as usize);
        lines.push(format!("{:>11} {:>6.1} {bar:<30} {}", level.at.to_string(), level.db, spoken.join(" ")));
    }
    for gap in &moment.non_speech {
        lines.push(format!("no words {}-{} ({:.2} s): {} at {:.1} dBFS", gap.start, gap.end, gap.seconds, gap.kind, gap.mean_db));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::{Segment, Word};

    #[test]
    fn word_gaps_are_split_into_silence_and_sound_by_level() {
        // 1 s speech, 1 s silence, 1 s speech, 1 s loud music, 1 s speech (10 ms frames).
        let pattern = [(100, 0.05), (100, 1e-8), (100, 0.05), (100, 0.05), (100, 0.05)];
        let envelope = Envelope::from_mean_squares(pattern.iter().flat_map(|&(frames, square)| std::iter::repeat_n(square, frames)));
        let word = |start: f64, end: f64| Word { start, end, text: "word".into(), probability: None };
        let transcript = Transcript {
            engine: "test".into(),
            model: "test".into(),
            language: "en".into(),
            unreliable: Vec::new(),
            segments: vec![Segment { start: 0.0, end: 5.0, text: String::new(), words: vec![word(0.0, 1.0), word(2.0, 3.0), word(4.0, 5.0)] }],
        };
        let moment = inspect(&envelope, Some(&transcript), 0.0, 5.0, 0.5, -45.0);
        let kinds: Vec<(&str, f64)> = moment.non_speech.iter().map(|gap| (gap.kind, gap.start.seconds())).collect();
        assert_eq!(kinds, vec![("silence", 1.0), ("sound", 3.0)]);
        assert_eq!(moment.levels.len(), 10);
    }
}
