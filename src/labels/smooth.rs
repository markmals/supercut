//! Smoothing one answer per unit into contiguous segments: a Viterbi pass that prices every
//! change of segment, so a single odd unit does not become a segment of its own.
//!
//! Units come at two scales — ~30 s windows and single utterances — so each one's answer counts
//! for what it covers: a three-second line cannot move a boundary on its own, a run of them can.

use crate::labels::record::{LabeledSegment, LabeledUnit};
use crate::timecode::Time;

/// Probability floor under an emission, so one certain-looking unit cannot dominate the path.
const PROBABILITY_FLOOR: f64 = 0.001;
/// What changing segment between neighbouring units costs, in log probability. A segment one
/// window wide has to beat two of these, a classified ad two windows long only one.
const SWITCH_PENALTY: f64 = 1.0;
/// The unit length a full vote is worth: one coarse window.
const FULL_VOTE_SECONDS: f64 = 30.0;
/// The smallest vote any unit gets, however short it is. Four strongly-labeled utterances then
/// carry a boundary, which is what a 15 s bonus-room preamble needs.
const LEAST_VOTE: f64 = 0.25;

/// The segments `units` smooth into, covering `0..duration` without gaps.
pub fn segments(units: &[LabeledUnit], labels: &[String], duration: f64) -> Vec<LabeledSegment> {
    if units.is_empty() || labels.is_empty() {
        return Vec::new();
    }
    let emissions: Vec<Vec<f64>> = units.iter().map(|unit| emission(unit, labels)).collect();
    let path = best_path(&emissions);
    runs(&path, units, labels, duration)
}

/// One unit's log-probabilities, scaled by how much of the episode it speaks for.
fn emission(unit: &LabeledUnit, labels: &[String]) -> Vec<f64> {
    let vote = (unit.seconds() / FULL_VOTE_SECONDS).clamp(LEAST_VOTE, 1.0);
    labels
        .iter()
        .map(|label| {
            let probability = unit.probabilities.get(label).copied().unwrap_or(0.0);
            vote * probability.max(PROBABILITY_FLOOR).ln()
        })
        .collect()
}

/// The highest-scoring label for every unit, given the switch penalty.
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
        .expect("a unit carries at least one label")
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
        .expect("a unit carries at least one label")
}

fn runs(
    path: &[usize],
    units: &[LabeledUnit],
    labels: &[String],
    duration: f64,
) -> Vec<LabeledSegment> {
    let mut segments = Vec::new();
    let mut begin = 0;
    for index in 1..=path.len() {
        if index < path.len() && path[index] == path[begin] {
            continue;
        }
        let members = &units[begin..index];
        let label = &labels[path[begin]];
        segments.push(LabeledSegment {
            label: label.clone(),
            start: Time(if begin == 0 { 0.0 } else { gap_middle(&units[begin - 1], &units[begin]) }),
            end: Time(match units.get(index) {
                Some(next) => gap_middle(&units[index - 1], next),
                None => duration.max(members[members.len() - 1].end.seconds()),
            }),
            probability: mean_probability(members, label),
            min_confidence: members.iter().map(|unit| unit.confidence).fold(1.0, f64::min),
        });
        begin = index;
    }
    segments
}

/// The middle of the wordless gap between two units: where a segment changes.
fn gap_middle(before: &LabeledUnit, after: &LabeledUnit) -> f64 {
    (before.end.seconds() + after.start.seconds()) / 2.0
}

fn mean_probability(units: &[LabeledUnit], label: &str) -> f64 {
    let total: f64 =
        units.iter().map(|unit| unit.probabilities.get(label).copied().unwrap_or(0.0)).sum();
    total / units.len() as f64
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::segments;
    use crate::labels::record::LabeledUnit;
    use crate::timecode::Time;

    fn labels() -> Vec<String> {
        vec!["hosts".to_owned(), "classified".to_owned()]
    }

    /// Windows 30 s apart with a 1 s gap between them, `classified` carrying `odds` in the
    /// windows named by `odd_ones` and 0.1 everywhere else.
    fn windows(count: usize, odd_ones: &[usize], odds: f64) -> Vec<LabeledUnit> {
        (0..count)
            .map(|index| {
                let classified = if odd_ones.contains(&index) { odds } else { 0.1 };
                let probabilities = BTreeMap::from([
                    ("hosts".to_owned(), 1.0 - classified),
                    ("classified".to_owned(), classified),
                ]);
                LabeledUnit {
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

    fn names() -> Vec<String> {
        ["preamble", "hosts", "classified"].map(ToOwned::to_owned).to_vec()
    }

    /// A unit of `seconds`, `label` carrying `probability` and the other labels sharing the rest.
    fn unit(start: f64, seconds: f64, label: &str, probability: f64) -> LabeledUnit {
        let names = names();
        let rest = (1.0 - probability) / (names.len() - 1) as f64;
        let probabilities = names
            .iter()
            .map(|name| (name.clone(), if name == label { probability } else { rest }))
            .collect();
        LabeledUnit {
            start: Time(start),
            end: Time(start + seconds),
            selected: label.to_owned(),
            confidence: probability,
            probabilities,
        }
    }

    fn hosts_from(start: f64, count: usize) -> Vec<LabeledUnit> {
        (0..count).map(|index| unit(start + index as f64 * 31.0, 30.0, "hosts", 0.9)).collect()
    }

    #[test]
    fn a_run_of_short_utterances_can_hold_a_segment_of_its_own() {
        let mut units: Vec<LabeledUnit> =
            (0..5).map(|index| unit(index as f64 * 3.5, 3.0, "preamble", 0.9)).collect();
        units.extend(hosts_from(18.0, 6));
        let smoothed = segments(&units, &names(), 220.0);
        assert_eq!(smoothed.len(), 2, "a 15 s preamble is a segment");
        assert_eq!(smoothed[0].label, "preamble");
        assert!((smoothed[0].end.seconds() - 17.5).abs() < 0.01, "ends at {}", smoothed[0].end);
    }

    #[test]
    fn one_short_utterance_cannot_flip_a_segment() {
        let mut units = hosts_from(0.0, 3);
        units.push(unit(93.0, 3.0, "classified", 0.9));
        units.extend(hosts_from(97.0, 3));
        let smoothed = segments(&units, &names(), 220.0);
        assert_eq!(smoothed.len(), 1, "three seconds do not outvote two switches");
    }

    #[test]
    fn one_whole_window_still_can() {
        let mut units = hosts_from(0.0, 3);
        units.push(unit(93.0, 30.0, "classified", 0.9));
        units.extend(hosts_from(124.0, 3));
        let smoothed = segments(&units, &names(), 320.0);
        assert_eq!(smoothed.len(), 3, "thirty seconds do");
    }
}
