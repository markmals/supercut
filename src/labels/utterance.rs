//! Splitting a transcript into single utterances: the fine pass that puts segment boundaries on
//! words rather than on window edges.

use crate::labels::unit::{Sound, Unit, unit};
use crate::transcript::{Transcript, Word};

/// A pause this long ends an utterance.
const PAUSE_SECONDS: f64 = 0.35;
/// No utterance runs longer than this.
const MAX_SECONDS: f64 = 12.0;
/// Spoken context quoted either side of an utterance, as much as a coarse window gets: one line
/// of a read-out ad looks much like one line of chat, and only the run-up tells them apart.
const CONTEXT_SECONDS: f64 = 180.0;

/// Utterances covering every word inside `regions`, in order.
pub fn utterances(transcript: &Transcript, sound: Sound<'_>, regions: &[(f64, f64)]) -> Vec<Unit> {
    let words: Vec<&Word> = transcript.words().collect();
    let mut utterances = Vec::new();
    let mut first = 0;
    while first < words.len() {
        let last = utterance_end(&words, first);
        if inside(regions, words[first].start) {
            utterances.push(unit(&words, first, last, CONTEXT_SECONDS, sound));
        }
        first = last + 1;
    }
    utterances
}

/// The last word before the next pause, or before the utterance outstays its welcome.
fn utterance_end(words: &[&Word], first: usize) -> usize {
    let start = words[first].start;
    let mut last = first;
    while last + 1 < words.len() {
        let pause = words[last + 1].start - words[last].end;
        if pause >= PAUSE_SECONDS || words[last + 1].end - start > MAX_SECONDS {
            break;
        }
        last += 1;
    }
    last
}

fn inside(regions: &[(f64, f64)], time: f64) -> bool {
    regions.iter().any(|(start, end)| time >= *start && time < *end)
}

#[cfg(test)]
mod tests {
    use super::{MAX_SECONDS, utterances};
    use crate::labels::unit::Sound;
    use crate::labels::window::tests::{quiet, transcript};

    #[test]
    fn utterances_break_at_pauses_and_stay_short() {
        // Words every 0.5 s, a quarter second apart, with a 1 s pause before word 10.
        let transcript = transcript(60, 0.5, &[(10, 1.0)]);
        let quiet = quiet(60.0);
        let sound = Sound { envelope: &quiet, unreliable: &[] };
        let found = utterances(&transcript, sound, &[(0.0, 60.0)]);
        assert_eq!(found[0].end, 4.75, "the first utterance ends at the pause");
        assert_eq!(found[1].start, 6.0, "the next one starts after it");
        assert!(found.iter().all(|unit| unit.end - unit.start <= MAX_SECONDS));
    }

    #[test]
    fn only_the_regions_asked_for_are_split() {
        let transcript = transcript(200, 0.5, &[]);
        let quiet = quiet(120.0);
        let sound = Sound { envelope: &quiet, unreliable: &[] };
        let found = utterances(&transcript, sound, &[(0.0, 20.0), (60.0, 80.0)]);
        assert!(found.iter().all(|unit| unit.start < 20.0 || (60.0..80.0).contains(&unit.start)));
        assert!(found.iter().any(|unit| unit.start >= 60.0), "the second region is covered");
    }
}
