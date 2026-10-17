//! Smoothing one answer per window into contiguous segments: a Viterbi pass that prices every
//! change of segment, so a single odd window does not become a segment of its own.

use crate::labels::record::{LabeledSegment, LabeledWindow};
use crate::timecode::Time;

/// Probability floor under an emission, so one certain-looking window cannot dominate the path.
const PROBABILITY_FLOOR: f64 = 0.001;
/// What changing segment between neighbouring windows costs, in log probability. A segment a
/// window wide has to beat two of these, a classified ad two windows long only one.
const SWITCH_PENALTY: f64 = 1.0;

/// The segments `windows` smooth into, covering `0..duration` without gaps.
pub fn segments(windows: &[LabeledWindow], labels: &[String], duration: f64) -> Vec<LabeledSegment> {
    if windows.is_empty() || labels.is_empty() {
        return Vec::new();
    }
    let emissions: Vec<Vec<f64>> = windows.iter().map(|window| emission(window, labels)).collect();
    let path = best_path(&emissions);
    runs(&path, windows, labels, duration)
}

fn emission(window: &LabeledWindow, labels: &[String]) -> Vec<f64> {
    labels
        .iter()
        .map(|label| {
            let probability = window.probabilities.get(label).copied().unwrap_or(0.0);
            probability.max(PROBABILITY_FLOOR).ln()
        })
        .collect()
}

/// The highest-scoring label for every window, given the switch penalty.
fn best_path(emissions: &[Vec<f64>]) -> Vec<usize> {
    let states = emissions[0].len();
    let mut scores = emissions[0].clone();
    let mut previous: Vec<Vec<usize>> = Vec::with_capacity(emissions.len() - 1);
    for emission in &emissions[1..] {
        let mut step = Vec::with_capacity(states);
        let mut from = Vec::with_capacity(states);
        for (state, probability) in emission.iter().enumerate() {
            let (score, source) = best_source(&scores, state);
            step.push(score + probability);
            from.push(source);
        }
        scores = step;
        previous.push(from);
    }
    backtrack(&scores, &previous)
}

fn best_source(scores: &[f64], state: usize) -> (f64, usize) {
    (0..scores.len())
        .map(|source| {
            let penalty = if source == state { 0.0 } else { SWITCH_PENALTY };
            (scores[source] - penalty, source)
        })
        .reduce(|best, candidate| if candidate.0 > best.0 { candidate } else { best })
        .expect("a window carries at least one label")
}

fn backtrack(scores: &[f64], previous: &[Vec<usize>]) -> Vec<usize> {
    let mut state = argmax(scores);
    let mut path = vec![state; previous.len() + 1];
    for (index, from) in previous.iter().enumerate().rev() {
        state = from[state];
        path[index] = state;
    }
    path
}

fn argmax(scores: &[f64]) -> usize {
    (0..scores.len())
        .reduce(|best, candidate| if scores[candidate] > scores[best] { candidate } else { best })
        .expect("a window carries at least one label")
}

fn runs(
    path: &[usize],
    windows: &[LabeledWindow],
    labels: &[String],
    duration: f64,
) -> Vec<LabeledSegment> {
    let mut segments = Vec::new();
    let mut begin = 0;
    for index in 1..=path.len() {
        if index < path.len() && path[index] == path[begin] {
            continue;
        }
        let members = &windows[begin..index];
        let label = &labels[path[begin]];
        segments.push(LabeledSegment {
            label: label.clone(),
            start: Time(if begin == 0 { 0.0 } else { gap_middle(&windows[begin - 1], &windows[begin]) }),
            end: Time(match windows.get(index) {
                Some(next) => gap_middle(&windows[index - 1], next),
                None => duration.max(members[members.len() - 1].end.seconds()),
            }),
            probability: mean_probability(members, label),
            min_confidence: members.iter().map(|window| window.confidence).fold(1.0, f64::min),
        });
        begin = index;
    }
    segments
}

/// The middle of the wordless gap between two windows: where a segment changes.
fn gap_middle(before: &LabeledWindow, after: &LabeledWindow) -> f64 {
    (before.end.seconds() + after.start.seconds()) / 2.0
}

fn mean_probability(windows: &[LabeledWindow], label: &str) -> f64 {
    let total: f64 =
        windows.iter().map(|window| window.probabilities.get(label).copied().unwrap_or(0.0)).sum();
    total / windows.len() as f64
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::segments;
    use crate::labels::record::LabeledWindow;
    use crate::timecode::Time;

    fn labels() -> Vec<String> {
        vec!["hosts".to_owned(), "classified".to_owned()]
    }

    /// Windows 30 s apart with a 1 s gap between them, `classified` carrying `odds` in the
    /// windows named by `odd_ones` and 0.1 everywhere else.
    fn windows(count: usize, odd_ones: &[usize], odds: f64) -> Vec<LabeledWindow> {
        (0..count)
            .map(|index| {
                let classified = if odd_ones.contains(&index) { odds } else { 0.1 };
                let probabilities = BTreeMap::from([
                    ("hosts".to_owned(), 1.0 - classified),
                    ("classified".to_owned(), classified),
                ]);
                LabeledWindow {
                    start: Time(index as f64 * 31.0),
                    end: Time(index as f64 * 31.0 + 30.0),
                    selected: if classified > 0.5 { "classified".into() } else { "hosts".into() },
                    confidence: classified.max(1.0 - classified),
                    probabilities,
                }
            })
            .collect()
    }

    #[test]
    fn one_odd_window_does_not_become_a_segment() {
        let smoothed = segments(&windows(6, &[2], 0.75), &labels(), 190.0);
        assert_eq!(smoothed.len(), 1);
        assert_eq!(smoothed[0].label, "hosts");
    }

    #[test]
    fn a_sustained_change_survives() {
        let smoothed = segments(&windows(6, &[2, 3], 0.8), &labels(), 190.0);
        let found: Vec<&str> = smoothed.iter().map(|segment| segment.label.as_str()).collect();
        assert_eq!(found, ["hosts", "classified", "hosts"]);
        assert!((smoothed[1].probability - 0.8).abs() < 1e-9);
    }

    #[test]
    fn segments_cover_the_episode_and_change_inside_the_gaps() {
        let smoothed = segments(&windows(6, &[2, 3], 0.8), &labels(), 190.0);
        assert_eq!(smoothed[0].start, Time(0.0));
        assert_eq!(smoothed.last().unwrap().end, Time(190.0));
        // Window 1 ends at 61 s, window 2 starts at 62 s.
        assert_eq!(smoothed[1].start, Time(61.5));
        for pair in smoothed.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
    }

    #[test]
    fn a_fairly_confident_single_window_still_loses_to_its_neighbours() {
        let smoothed = segments(&windows(6, &[2], 0.85), &labels(), 190.0);
        assert_eq!(smoothed.len(), 1, "one window has to pay for two switches");
    }
}
