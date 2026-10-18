//! The transcript as an agent reads it: one line per utterance, holes called out.

use serde::Serialize;

use crate::transcript::{Transcript, Word};

/// A gap this long means someone else started talking, or the same person moved on.
const PAUSE_SECONDS: f64 = 1.0;
/// Longest line, so that a monologue still gives the reader timestamps to aim at.
const UTTERANCE_SECONDS: f64 = 25.0;

/// Words said without a noticeable pause: one line of the transcript.
#[derive(Serialize)]
pub struct Utterance {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// The utterances of `transcript` that overlap `[from, to)`.
pub fn utterances(transcript: &Transcript, from: f64, to: f64) -> Vec<Utterance> {
    let mut utterances: Vec<Utterance> = Vec::new();
    let mut spoken_until = 0.0;
    for word in transcript.words_between(from, to) {
        match utterances.last_mut() {
            Some(current)
                if word.start - spoken_until <= PAUSE_SECONDS
                    && word.end - current.start <= UTTERANCE_SECONDS =>
            {
                extend(current, word);
            }
            _ => utterances.push(Utterance {
                start: word.start,
                end: word.end,
                text: word.text.clone(),
            }),
        }
        spoken_until = word.end;
    }
    utterances
}

fn extend(utterance: &mut Utterance, word: &Word) {
    utterance.text.push(' ');
    utterance.text.push_str(&word.text);
    utterance.end = word.end;
}

/// `[m:ss.s] what was said`, one line per utterance, with unreliable spans in their place.
pub fn render(transcript: &Transcript, from: f64, to: f64) -> String {
    let mut lines: Vec<(f64, String)> = utterances(transcript, from, to)
        .into_iter()
        .map(|utterance| {
            (utterance.start, format!("[{}] {}", stamp(utterance.start), utterance.text))
        })
        .collect();
    lines.extend(transcript.unreliable_between(from, to).map(|span| {
        (
            span.start,
            format!("[{}] [unreliable transcript until {}]", stamp(span.start), stamp(span.end)),
        )
    }));
    lines.sort_by(|left, right| left.0.total_cmp(&right.0));
    lines.into_iter().map(|(_, line)| line).collect::<Vec<_>>().join("\n")
}

/// `m:ss.s`: minutes keep counting past sixty, which reads better than hours for one episode.
fn stamp(seconds: f64) -> String {
    let minutes = (seconds / 60.0).floor();
    format!("{minutes}:{:04.1}", seconds - minutes * 60.0)
}

#[cfg(test)]
mod tests {
    use crate::transcript::{Transcript, UnreliableSpan, Word};

    fn transcript(words: &[(f64, f64, &str)], unreliable: Vec<UnreliableSpan>) -> Transcript {
        let words: Vec<Word> = words
            .iter()
            .map(|(start, end, text)| Word {
                start: *start,
                end: *end,
                text: (*text).to_owned(),
                probability: None,
            })
            .collect();
        let segments =
            words.chunks(2).filter_map(|chunk| super::super::segment_of(chunk.to_vec())).collect();
        Transcript {
            engine: "mlx-whisper".into(),
            model: "test".into(),
            language: "en".into(),
            unreliable,
            segments,
        }
    }

    #[test]
    fn breaks_lines_on_pauses() {
        let transcript = transcript(
            &[
                (59.0, 59.4, "Knock,"),
                (59.5, 60.0, "knock."),
                (61.5, 61.9, "Who's"),
                (61.9, 62.2, "there?"),
            ],
            Vec::new(),
        );
        assert_eq!(
            super::render(&transcript, 0.0, f64::INFINITY),
            "[0:59.0] Knock, knock.\n[1:01.5] Who's there?"
        );
    }

    #[test]
    fn splits_an_utterance_that_runs_too_long() {
        let words: Vec<(f64, f64, &str)> =
            (0..60).map(|index| (index as f64, index as f64 + 0.9, "and")).collect();
        let text = super::render(&transcript(&words, Vec::new()), 0.0, f64::INFINITY);
        let starts: Vec<&str> = text.lines().map(|line| line.split(']').next().unwrap()).collect();
        assert_eq!(starts, ["[0:00.0", "[0:25.0", "[0:50.0"]);
    }

    #[test]
    fn marks_unreliable_spans_in_place() {
        let span = UnreliableSpan {
            start: 70.0,
            end: 95.5,
            reason: "engine repeated \"no\" 40 times".into(),
        };
        let transcript = transcript(
            &[(60.0, 60.5, "Oh,"), (60.5, 61.0, "no."), (96.0, 96.4, "Anyway.")],
            vec![span],
        );
        assert_eq!(
            super::render(&transcript, 0.0, f64::INFINITY),
            "[1:00.0] Oh, no.\n[1:10.0] [unreliable transcript until 1:35.5]\n[1:36.0] Anyway."
        );
    }

    #[test]
    fn keeps_only_what_overlaps_the_window() {
        let span = UnreliableSpan {
            start: 70.0,
            end: 95.5,
            reason: "engine repeated \"no\" 40 times".into(),
        };
        let whole = transcript(
            &[(60.0, 60.5, "Oh,"), (60.5, 61.0, "no."), (96.0, 96.4, "Anyway.")],
            vec![span],
        );
        let part = super::utterances(&whole, 61.0, 96.2);
        assert_eq!(part.len(), 1);
        assert_eq!(part[0].text, "Anyway.");
        assert_eq!(whole.unreliable_between(96.5, 200.0).count(), 0);
    }
}
