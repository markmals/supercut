//! Finding the loops Whisper falls into, where it repeats one phrase instead of transcribing.

use std::ops::Range;

use crate::transcript::{Transcript, UnreliableSpan, Word};

/// A loop repeats a short phrase: longer repeats are callbacks people make on purpose.
const MAX_PHRASE_WORDS: usize = 4;
/// People repeat themselves a handful of times; an engine repeats itself until the audio ends.
const MIN_REPEATS: usize = 10;
/// Even a fast chant is over in a few seconds, so only long stretches count as a loop.
const MIN_SPAN_SECONDS: f64 = 15.0;

/// Removes looped words from `transcript` and records each loop as an unreliable span.
pub fn strip_loops(transcript: &mut Transcript) {
    let words: Vec<Word> = transcript.words().cloned().collect();
    let loops = find(&words);
    if loops.is_empty() {
        return;
    }
    let mut looped = vec![false; words.len()];
    for found in &loops {
        looped[found.words.clone()].fill(true);
    }
    let mut position = 0;
    let mut segments = Vec::with_capacity(transcript.segments.len());
    for segment in std::mem::take(&mut transcript.segments) {
        let mut kept = Vec::with_capacity(segment.words.len());
        for word in segment.words {
            if !looped[position] {
                kept.push(word);
            }
            position += 1;
        }
        segments.extend(super::segment_of(kept));
    }
    transcript.segments = segments;
    transcript.unreliable.extend(loops.into_iter().map(|found| found.span));
}

struct Loop {
    words: Range<usize>,
    span: UnreliableSpan,
}

/// Every loop in `words`, in order and without overlap.
fn find(words: &[Word]) -> Vec<Loop> {
    let keys: Vec<String> = words.iter().map(|word| key(&word.text)).collect();
    let mut found = Vec::new();
    let mut start = 0;
    while start < keys.len() {
        match loop_at(words, &keys, start) {
            Some(repetition) => {
                start = repetition.words.end;
                found.push(repetition);
            }
            None => start += 1,
        }
    }
    found
}

/// The longest loop starting at `start`, if the words there repeat long and often enough.
fn loop_at(words: &[Word], keys: &[String], start: usize) -> Option<Loop> {
    (1..=MAX_PHRASE_WORDS)
        .filter_map(|phrase| {
            let repeats = repeats_of(keys, start, phrase);
            let end = start + repeats * phrase;
            let span = words[start].start..words[end - 1].end;
            let long_enough = span.end - span.start >= MIN_SPAN_SECONDS;
            (repeats >= MIN_REPEATS && long_enough).then(|| Loop {
                words: start..end,
                span: UnreliableSpan {
                    start: span.start,
                    end: span.end,
                    reason: format!("engine repeated \"{}\" {repeats} times", keys[start..start + phrase].join(" ")),
                },
            })
        })
        // Longest loop wins; between phrasings of the same stretch, the shortest phrase.
        .reduce(|best, candidate| if candidate.words.end > best.words.end { candidate } else { best })
}

/// How many times the `phrase` words at `start` repeat back to back, itself included.
fn repeats_of(keys: &[String], start: usize, phrase: usize) -> usize {
    let mut repeats = 0;
    let mut next = start;
    while next + phrase <= keys.len() && keys[next..next + phrase] == keys[start..start + phrase] {
        repeats += 1;
        next += phrase;
    }
    repeats
}

/// What makes two spoken words the same word: letters and digits, case and punctuation aside.
fn key(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric() || *c == '\'').flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use crate::transcript::{Transcript, Word};

    /// Words cut from a real mlx-whisper run of The Neighborhood Listen S10E01, one
    /// `start end text` line each, split into segments the way an engine emits them.
    fn transcript(fixture: &str) -> Transcript {
        let words: Vec<Word> = fixture
            .lines()
            .map(|line| {
                let mut parts = line.splitn(3, ' ');
                let mut number = || parts.next().unwrap().parse().unwrap();
                Word { start: number(), end: number(), text: parts.next().unwrap().to_owned(), probability: None }
            })
            .collect();
        let segments = words.chunks(4).filter_map(|chunk| super::super::segment_of(chunk.to_vec())).collect();
        Transcript {
            engine: "mlx-whisper".into(),
            model: "test".into(),
            language: "en".into(),
            unreliable: Vec::new(),
            segments,
        }
    }

    #[test]
    fn keeps_repeats_people_actually_say() {
        // Eleven "Royale." in eight seconds, then "It doesn't matter" three times over.
        let mut transcript = transcript(include_str!("testdata/genuine-repeats.txt"));
        let before = transcript.words().count();
        super::strip_loops(&mut transcript);
        assert!(transcript.unreliable.is_empty());
        assert_eq!(transcript.words().count(), before);
    }

    #[test]
    fn removes_a_one_word_loop() {
        let mut transcript = transcript(include_str!("testdata/loop-no.txt"));
        super::strip_loops(&mut transcript);
        let span = transcript.unreliable.first().expect("the `No.` loop");
        assert_eq!(transcript.unreliable.len(), 1);
        assert_eq!((span.start, span.end), (4216.74, 4261.58));
        assert_eq!(span.reason, "engine repeated \"no\" 52 times");
        assert_eq!(transcript.words().count(), 0);
        assert!(transcript.segments.is_empty());
    }

    #[test]
    fn removes_a_two_word_loop() {
        let mut transcript = transcript(include_str!("testdata/loop-i-mean.txt"));
        super::strip_loops(&mut transcript);
        let span = transcript.unreliable.first().expect("the `I mean,` loop");
        assert_eq!(transcript.unreliable.len(), 1);
        assert!(span.reason.starts_with("engine repeated \"i mean\""), "{}", span.reason);
        assert!(span.end - span.start > 20.0);
        assert_eq!(transcript.words().count(), 0);
    }
}
