//! Where the second, finer labeling pass runs, and how its answers join the coarse ones.
//!
//! Coarse windows find the shape of an episode but place its boundaries no better than ~30 s.
//! The fine pass re-asks the same question utterance by utterance where that matters: the
//! opening, the closing, and around every boundary the coarse pass found.

use crate::labels::record::{LabeledSegment, LabeledUnit};

/// The fine pass covers the opening of every episode,
const OPENING_SECONDS: f64 = 120.0;
/// its closing minutes,
const CLOSING_SECONDS: f64 = 240.0;
/// and this far either side of every coarse boundary.
const AROUND_BOUNDARY_SECONDS: f64 = 40.0;

/// The stretches to label utterance by utterance, each widened to whole coarse windows so no
/// word is covered by both passes.
pub fn regions(coarse: &[LabeledUnit], segments: &[LabeledSegment], duration: f64) -> Vec<Region> {
    let mut spans = opening_and_closing(duration);
    for boundary in segments.iter().skip(1).map(|segment| segment.start.seconds()) {
        spans.push((boundary - AROUND_BOUNDARY_SECONDS, boundary + AROUND_BOUNDARY_SECONDS));
    }
    let mut widened: Vec<Region> = spans.into_iter().map(|span| widen(coarse, span)).collect();
    widened.sort_by(|a, b| a.0.total_cmp(&b.0));
    merge(widened)
}

/// What the fine pass covers whatever the coarse pass finds: the opening and the closing.
pub fn opening_and_closing(duration: f64) -> Vec<Region> {
    merge(vec![
        (0.0, OPENING_SECONDS.min(duration)),
        ((duration - CLOSING_SECONDS).max(0.0), duration),
    ])
}

/// How many utterances one boundary's region holds, at this episode's rate of talking.
pub fn utterances_per_boundary(utterances: usize, duration: f64) -> usize {
    let a_second = utterances as f64 / duration.max(1.0);
    (a_second * 2.0 * AROUND_BOUNDARY_SECONDS).round() as usize
}

/// A stretch of the episode the fine pass covers.
pub type Region = (f64, f64);

/// Fine units inside the regions, coarse units everywhere else, in order.
pub fn mixed(
    coarse: &[LabeledUnit],
    fine: Vec<LabeledUnit>,
    regions: &[Region],
) -> Vec<LabeledUnit> {
    let kept = coarse.iter().filter(|unit| !covered(regions, unit)).cloned();
    let mut units: Vec<LabeledUnit> = kept.chain(fine).collect();
    units.sort_by(|a, b| a.start.seconds().total_cmp(&b.start.seconds()));
    units
}

/// Does the fine pass take this window over?
fn covered(regions: &[Region], unit: &LabeledUnit) -> bool {
    regions.iter().any(|(start, end)| unit.end.seconds() > *start && unit.start.seconds() < *end)
}

/// Grows `span` to the ends of the coarse windows it touches.
fn widen(coarse: &[LabeledUnit], span: Region) -> Region {
    let mut touched = coarse
        .iter()
        .filter(|unit| unit.end.seconds() > span.0 && unit.start.seconds() < span.1)
        .map(|unit| (unit.start.seconds(), unit.end.seconds()));
    let Some(first) = touched.next() else {
        return span;
    };
    let last = touched.next_back().unwrap_or(first);
    (first.0.min(span.0), last.1.max(span.1))
}

fn merge(sorted: Vec<Region>) -> Vec<Region> {
    let mut merged: Vec<Region> = Vec::with_capacity(sorted.len());
    for region in sorted {
        match merged.last_mut() {
            Some(last) if region.0 <= last.1 => last.1 = last.1.max(region.1),
            _ => merged.push(region),
        }
    }
    merged
}
