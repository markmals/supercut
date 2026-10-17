//! Cutting a transcript into the ~30 s windows the coarse labeling pass asks about.

use crate::labels::unit::{Sound, Unit, unit};
use crate::transcript::{Transcript, Word};

/// A window ends at the best pause this far into it.
const MIN_SECONDS: f64 = 25.0;
const MAX_SECONDS: f64 = 35.0;
/// Where a window ends when no pause falls inside that range.
const TARGET_SECONDS: f64 = 30.0;
/// Spoken context quoted on each side of the window.
const CONTEXT_SECONDS: f64 = 90.0;

/// Windows covering every word of the transcript, in order.
pub fn windows(transcript: &Transcript, sound: Sound<'_>) -> Vec<Unit> {
    let words: Vec<&Word> = transcript.words().collect();
    let mut windows = Vec::new();
    let mut first = 0;
    while first < words.len() {
        let last = window_end(&words, first);
        windows.push(unit(&words, first, last, CONTEXT_SECONDS, sound));
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

#[cfg(test)]
pub(crate) mod tests {
    use super::{MAX_SECONDS, MIN_SECONDS, windows};
    use crate::analysis::{Envelope, FRAME_SECONDS};
    use crate::labels::unit::Sound;
    use crate::transcript::{Segment, Transcript, UnreliableSpan, Word};

    /// One word per `step` seconds, each lasting half a step, with `pauses` inserted as extra
    /// silence before the given word index.
    pub fn transcript(count: usize, step: f64, pauses: &[(usize, f64)]) -> Transcript {
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

    pub fn loud(seconds: f64) -> Envelope {
        Envelope::from_mean_squares(vec![0.25; (seconds / FRAME_SECONDS) as usize])
    }

    pub fn quiet(seconds: f64) -> Envelope {
        Envelope::from_mean_squares(vec![1e-9; (seconds / FRAME_SECONDS) as usize])
    }

    fn sound<'a>(envelope: &'a Envelope, unreliable: &'a [UnreliableSpan]) -> Sound<'a> {
        Sound { envelope, unreliable }
    }

    #[test]
    fn windows_run_about_thirty_seconds_and_cover_every_word() {
        let transcript = transcript(600, 0.5, &[]);
        let quiet = quiet(320.0);
        let windows = windows(&transcript, sound(&quiet, &[]));
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
        let quiet = quiet(200.0);
        let windows = windows(&transcript, sound(&quiet, &[]));
        assert!((windows[0].end - 31.55).abs() < 1e-6, "cut at {}", windows[0].end);
    }

    #[test]
    fn long_gaps_are_described_as_music_or_silence() {
        let transcript = transcript(200, 0.5, &[(10, 4.0)]);
        let (loud, quiet) = (loud(200.0), quiet(200.0));
        let musical = windows(&transcript, sound(&loud, &[]));
        assert!(musical[0].text.contains("[4 s of music/sound]"), "{}", musical[0].text);
        let silent = windows(&transcript, sound(&quiet, &[]));
        assert!(silent[0].text.contains("[4 s of silence]"), "{}", silent[0].text);
    }

    #[test]
    fn the_run_up_to_the_first_word_is_described() {
        let transcript = transcript(200, 0.5, &[(0, 67.0)]);
        let loud = loud(200.0);
        let windows = windows(&transcript, sound(&loud, &[]));
        assert!(windows[0].text.starts_with("[67 s of music/sound]"), "{}", windows[0].text);
    }

    #[test]
    fn unreliable_spans_are_called_out_instead_of_the_sound() {
        let transcript = transcript(200, 0.5, &[(10, 4.0)]);
        let unreliable =
            [UnreliableSpan { start: 5.0, end: 9.0, reason: "repeated \"no\" 90 times".into() }];
        let loud = loud(200.0);
        let windows = windows(&transcript, sound(&loud, &unreliable));
        assert!(windows[0].text.contains("[4 s dropped: repeated \"no\" 90 times]"));
    }

    #[test]
    fn context_quotes_the_neighbouring_minute_and_a_half() {
        let transcript = transcript(600, 0.5, &[]);
        let quiet = quiet(320.0);
        let windows = windows(&transcript, sound(&quiet, &[]));
        let middle = &windows[4];
        // 90 s of words at two a second, give or take the word the boundary lands on.
        assert!((179..=181).contains(&middle.before.split_whitespace().count()));
        assert!((179..=181).contains(&middle.after.split_whitespace().count()));
    }
}
