//! Cutting a transcript into the ~30 s windows Jev labels, each quoted with its surroundings.

use crate::analysis::Envelope;
use crate::labels::QUIET_DB;
use crate::transcript::{Transcript, UnreliableSpan, Word};

/// A window ends at the best pause this far into it.
const MIN_SECONDS: f64 = 25.0;
const MAX_SECONDS: f64 = 35.0;
/// Where a window ends when no pause falls inside that range.
const TARGET_SECONDS: f64 = 30.0;
/// Spoken context quoted on each side of the window.
const CONTEXT_SECONDS: f64 = 90.0;
/// A stretch without words at least this long is described in the text.
const GAP_SECONDS: f64 = 1.5;

/// One labeling request's worth of transcript.
#[derive(Debug)]
pub struct Window {
    /// Start of the window's first word.
    pub start: f64,
    /// End of the window's last word.
    pub end: f64,
    /// The window itself, long gaps described in place.
    pub text: String,
    /// What was said before the window.
    pub before: String,
    /// What was said after it.
    pub after: String,
}

/// Windows covering every word of the transcript, in order.
pub fn windows(transcript: &Transcript, envelope: &Envelope) -> Vec<Window> {
    let words: Vec<&Word> = transcript.words().collect();
    let mut windows = Vec::new();
    let mut first = 0;
    while first < words.len() {
        let last = window_end(&words, first);
        windows.push(window(&words, first, last, envelope, &transcript.unreliable));
        first = last + 1;
    }
    windows
}

/// The last word of the window that starts at `first`.
fn window_end(words: &[&Word], first: usize) -> usize {
    let start = words[first].start;
    let last = words.len() - 1;
    if words[last].end <= start + MAX_SECONDS {
        return last;
    }
    let candidates = (first..last)
        .filter(|&index| (MIN_SECONDS..=MAX_SECONDS).contains(&(words[index].end - start)));
    candidates
        .max_by(|&a, &b| pause_after(words, a).total_cmp(&pause_after(words, b)))
        .unwrap_or_else(|| nearest_to_target(words, first))
}

fn pause_after(words: &[&Word], index: usize) -> f64 {
    words[index + 1].start - words[index].end
}

/// The word boundary closest to the target length, for windows with no pause to cut at.
fn nearest_to_target(words: &[&Word], first: usize) -> usize {
    let target = words[first].start + TARGET_SECONDS;
    let distance = |index: &usize| (words[*index].end - target).abs();
    (first..words.len() - 1)
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
        .unwrap_or(first)
}

fn window(
    words: &[&Word],
    first: usize,
    last: usize,
    envelope: &Envelope,
    unreliable: &[UnreliableSpan],
) -> Window {
    let lead_in = if first == 0 { 0.0 } else { words[first - 1].end };
    let mut text = String::new();
    push_gap(&mut text, lead_in, words[first].start, envelope, unreliable);
    push_words(&mut text, &words[first..=last], envelope, unreliable);
    if last + 1 == words.len() {
        push_gap(&mut text, words[last].end, envelope.duration(), envelope, unreliable);
    }
    Window {
        start: words[first].start,
        end: words[last].end,
        text,
        before: render(&words[context_start(words, first)..first], envelope, unreliable),
        after: render(&words[last + 1..context_end(words, last)], envelope, unreliable),
    }
}

/// First word of the context quoted before the window.
fn context_start(words: &[&Word], first: usize) -> usize {
    let earliest = words[first].start - CONTEXT_SECONDS;
    words[..first].iter().position(|word| word.end >= earliest).unwrap_or(first)
}

/// One past the last word of the context quoted after the window.
fn context_end(words: &[&Word], last: usize) -> usize {
    let latest = words[last].end + CONTEXT_SECONDS;
    let after = &words[last + 1..];
    last + 1 + after.iter().position(|word| word.start > latest).unwrap_or(after.len())
}

fn render(words: &[&Word], envelope: &Envelope, unreliable: &[UnreliableSpan]) -> String {
    let mut text = String::new();
    push_words(&mut text, words, envelope, unreliable);
    text
}

fn push_words(
    text: &mut String,
    words: &[&Word],
    envelope: &Envelope,
    unreliable: &[UnreliableSpan],
) {
    for (index, word) in words.iter().enumerate() {
        if index > 0 {
            push_gap(text, words[index - 1].end, word.start, envelope, unreliable);
        }
        push_part(text, &word.text);
    }
}

/// Describes `start`..`end` when it is long enough to matter: the engine drops out there, or
/// music plays, or nobody is talking.
fn push_gap(
    text: &mut String,
    start: f64,
    end: f64,
    envelope: &Envelope,
    unreliable: &[UnreliableSpan],
) {
    let seconds = end - start;
    if seconds < GAP_SECONDS {
        return;
    }
    let rounded = seconds.round() as i64;
    match unreliable.iter().find(|span| span.end > start && span.start < end) {
        Some(span) => push_part(text, &format!("[{rounded} s dropped: {}]", span.reason)),
        None if envelope.mean_db(start, end) > QUIET_DB => {
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

#[cfg(test)]
mod tests {
    use super::{MAX_SECONDS, MIN_SECONDS, windows};
    use crate::analysis::{Envelope, FRAME_SECONDS};
    use crate::transcript::{Segment, Transcript, UnreliableSpan, Word};

    /// One word per `step` seconds, each lasting half a step, with `pauses` inserted as extra
    /// silence before the given word index.
    fn transcript(count: usize, step: f64, pauses: &[(usize, f64)]) -> Transcript {
        let mut words = Vec::new();
        let mut clock = 0.0;
        for index in 0..count {
            if let Some((_, pause)) = pauses.iter().find(|(at, _)| *at == index) {
                clock += pause;
            }
            words.push(Word {
                start: clock,
                end: clock + step / 2.0,
                text: format!("w{index}"),
                probability: None,
            });
            clock += step;
        }
        Transcript {
            engine: "test".into(),
            model: "test".into(),
            language: "en".into(),
            unreliable: Vec::new(),
            segments: vec![Segment { start: 0.0, end: clock, text: String::new(), words }],
        }
    }

    fn loud(seconds: f64) -> Envelope {
        Envelope::from_mean_squares(vec![0.25; (seconds / FRAME_SECONDS) as usize])
    }

    fn quiet(seconds: f64) -> Envelope {
        Envelope::from_mean_squares(vec![1e-9; (seconds / FRAME_SECONDS) as usize])
    }

    #[test]
    fn windows_run_about_thirty_seconds_and_cover_every_word() {
        let transcript = transcript(600, 0.5, &[]);
        let windows = windows(&transcript, &quiet(320.0));
        let (last, rest) = windows.split_last().unwrap();
        for window in rest {
            let length = window.end - window.start;
            assert!((MIN_SECONDS..=MAX_SECONDS).contains(&length), "window of {length} s");
        }
        assert!(last.end - last.start <= MAX_SECONDS);
        assert_eq!(windows[0].start, 0.0);
        assert_eq!(last.end, 299.75, "the last word is in the last window");
        for pair in windows.windows(2) {
            assert!(pair[1].start >= pair[0].end, "windows overlap");
        }
    }

    #[test]
    fn windows_break_at_the_longest_pause_in_range() {
        // A 1 s pause 25.75 s in, a 3.25 s pause 31.55 s in: the longer one wins.
        let transcript = transcript(300, 0.5, &[(52, 0.8), (62, 3.0)]);
        let windows = windows(&transcript, &quiet(200.0));
        assert!((windows[0].end - 31.55).abs() < 1e-6, "cut at {}", windows[0].end);
    }

    #[test]
    fn long_gaps_are_described_as_music_or_silence() {
        let transcript = transcript(200, 0.5, &[(10, 4.0)]);
        let musical = windows(&transcript, &loud(200.0));
        assert!(musical[0].text.contains("[4 s of music/sound]"), "{}", musical[0].text);
        let silent = windows(&transcript, &quiet(200.0));
        assert!(silent[0].text.contains("[4 s of silence]"), "{}", silent[0].text);
    }

    #[test]
    fn the_run_up_to_the_first_word_is_described() {
        let transcript = transcript(200, 0.5, &[(0, 67.0)]);
        let windows = windows(&transcript, &loud(200.0));
        assert!(windows[0].text.starts_with("[67 s of music/sound]"), "{}", windows[0].text);
    }

    #[test]
    fn unreliable_spans_are_called_out_instead_of_the_sound() {
        let mut transcript = transcript(200, 0.5, &[(10, 4.0)]);
        transcript.unreliable =
            vec![UnreliableSpan { start: 5.0, end: 9.0, reason: "repeated \"no\" 90 times".into() }];
        let windows = windows(&transcript, &loud(200.0));
        assert!(windows[0].text.contains("[4 s dropped: repeated \"no\" 90 times]"));
    }

    #[test]
    fn context_quotes_the_neighbouring_minute_and_a_half() {
        let transcript = transcript(600, 0.5, &[]);
        let windows = windows(&transcript, &quiet(320.0));
        let middle = &windows[4];
        // 90 s of words at two a second, give or take the word the boundary lands on.
        assert!((179..=181).contains(&middle.before.split_whitespace().count()));
        assert!((179..=181).contains(&middle.after.split_whitespace().count()));
    }
}
