//! Spoken phrases that pin a segment's edges, e.g. the hosts' "welcome back" right after a fake
//! ad. A phrase in the transcript is exact where labels are only as precise as a window.

use crate::transcript::Transcript;

/// How far from a labeled edge an anchor phrase is looked for.
const REACH_SECONDS: f64 = 45.0;

/// A transcript word reduced to what phrase matching compares.
pub struct Spoken {
    start: f64,
    end: f64,
    text: String,
}

pub fn spoken(transcript: &Transcript) -> Vec<Spoken> {
    transcript
        .words()
        .map(|word| Spoken { start: word.start, end: word.end, text: normalize(&word.text) })
        .collect()
}

/// Lowercase letters, digits and apostrophes only, so "Back," matches "back" and "we'll" stays
/// distinct from "well".
fn normalize(text: &str) -> String {
    text.chars()
        .map(|character| if character == '’' { '\'' } else { character })
        .filter(|character| character.is_alphanumeric() || *character == '\'')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Every place `phrase` is said, as (start of its first word, end of its last word).
fn occurrences(words: &[Spoken], phrase: &str) -> Vec<(f64, f64)> {
    let wanted: Vec<String> =
        phrase.split_whitespace().map(normalize).filter(|word| !word.is_empty()).collect();
    if wanted.is_empty() || words.len() < wanted.len() {
        return Vec::new();
    }
    words
        .windows(wanted.len())
        .filter(|run| run.iter().zip(&wanted).all(|(word, want)| word.text == *want))
        .map(|run| (run[0].start, run[run.len() - 1].end))
        .collect()
}

/// Where a segment labeled to start near `labeled_start` really begins: right after the
/// closest of `phrases` said around then.
pub fn start_after(words: &[Spoken], phrases: &[String], labeled_start: f64) -> Option<f64> {
    phrases
        .iter()
        .flat_map(|phrase| occurrences(words, phrase))
        .map(|(_, end)| end)
        .filter(|end| (end - labeled_start).abs() <= REACH_SECONDS)
        .min_by(|a, b| (a - labeled_start).abs().total_cmp(&(b - labeled_start).abs()))
}

/// Where a segment that began at `segment_start` and was labeled to end near `labeled_end`
/// really ends: at the first of `phrases` said after it began.
pub fn end_before(
    words: &[Spoken],
    phrases: &[String],
    segment_start: f64,
    labeled_end: f64,
) -> Option<f64> {
    phrases
        .iter()
        .flat_map(|phrase| occurrences(words, phrase))
        .map(|(start, _)| start)
        .filter(|start| *start > segment_start && *start <= labeled_end + REACH_SECONDS)
        .min_by(f64::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::{Segment, Word};

    fn said(words: &[(&str, f64)]) -> Vec<Spoken> {
        let words: Vec<Word> = words
            .iter()
            .map(|(text, start)| Word {
                start: *start,
                end: start + 0.3,
                text: (*text).into(),
                probability: None,
            })
            .collect();
        let transcript = Transcript {
            engine: String::new(),
            model: String::new(),
            language: String::new(),
            unreliable: Vec::new(),
            segments: vec![Segment { start: 0.0, end: 0.0, text: String::new(), words }],
        };
        spoken(&transcript)
    }

    #[test]
    fn the_hosts_welcome_back_ends_the_ad_wherever_the_labels_put_the_edge() {
        let words = said(&[
            ("pigs.", 4738.8),
            ("And", 4743.6),
            ("Welcome", 4744.8),
            ("back", 4745.2),
            ("to", 4745.6),
        ]);
        let phrases = vec!["welcome back".to_owned()];
        // Labels ended the ad 20 s late, or 10 s early: the phrase decides either way.
        assert_eq!(end_before(&words, &phrases, 4675.0, 4765.0), Some(4744.8));
        assert_eq!(end_before(&words, &phrases, 4675.0, 4735.0), Some(4744.8));
        assert_eq!(
            end_before(&words, &phrases, 4746.0, 4765.0),
            None,
            "only phrases after the segment began count"
        );
    }

    #[test]
    fn the_break_announcement_nearest_the_labeled_start_begins_the_ad() {
        let words = said(&[
            ("I'll", 1600.0),
            ("be", 1600.3),
            ("back", 1600.6),
            ("we'll", 1748.0),
            ("be", 1748.3),
            ("back", 1748.6),
            ("Hi,", 1764.6),
        ]);
        let phrases = vec!["be back".to_owned()];
        let start = start_after(&words, &phrases, 1760.0).unwrap();
        assert!((start - 1748.9).abs() < 1e-9, "the nearer announcement wins: {start}");
        assert_eq!(start_after(&words, &phrases, 1900.0), None, "too far from the labeled start");
    }
}
