//! How two plans for the same episodes differ: which cuts they share (and how far apart their
//! edges are), and which cuts only one of them makes. Used to hold a drafted plan against the
//! plan recovered from a supercut made by hand.

use std::collections::BTreeMap;

use serde::Serialize;

use super::{Cut, Plan};
use crate::timecode::Time;

#[derive(Serialize)]
pub struct Comparison {
    pub episodes: Vec<EpisodeComparison>,
    /// Episodes only one plan includes.
    pub only_in_a: Vec<String>,
    pub only_in_b: Vec<String>,
    pub summary: Summary,
}

#[derive(Serialize)]
pub struct EpisodeComparison {
    pub id: String,
    pub matched: Vec<MatchedCut>,
    pub only_in_a: Vec<CutSpan>,
    pub only_in_b: Vec<CutSpan>,
}

#[derive(Serialize)]
pub struct MatchedCut {
    pub a: CutSpan,
    pub b: CutSpan,
    /// `b.start - a.start` in seconds.
    pub start_delta: f64,
    /// `b.end - a.end` in seconds; absent when either cut runs to the episode end.
    pub end_delta: Option<f64>,
    /// Seconds both cuts remove.
    pub shared_seconds: f64,
}

#[derive(Serialize)]
pub struct CutSpan {
    pub start: Time,
    pub end: Option<Time>,
    pub seconds: f64,
    pub reason: String,
}

#[derive(Serialize)]
pub struct Summary {
    pub matched: usize,
    pub only_in_a: usize,
    pub only_in_b: usize,
    /// Seconds one plan cuts and the other keeps, over the shared episodes.
    pub seconds_cut_only_by_a: f64,
    pub seconds_cut_only_by_b: f64,
    /// Median distance between corresponding edges of matched cuts.
    pub median_edge_delta: Option<f64>,
}

/// `durations` holds each shared episode's length, which closes open-ended cuts.
pub fn compare(a: &Plan, b: &Plan, durations: &BTreeMap<String, f64>) -> Comparison {
    let ids_a: Vec<&str> = a.episodes.iter().map(|episode| episode.id.as_str()).collect();
    let ids_b: Vec<&str> = b.episodes.iter().map(|episode| episode.id.as_str()).collect();
    let episodes: Vec<EpisodeComparison> = a
        .episodes
        .iter()
        .filter_map(|episode_a| {
            let episode_b = b.episodes.iter().find(|episode| episode.id == episode_a.id)?;
            let duration = durations.get(&episode_a.id).copied().unwrap_or(f64::INFINITY);
            Some(compare_episode(&episode_a.id, &episode_a.cuts, &episode_b.cuts, duration))
        })
        .collect();
    Comparison {
        only_in_a: ids_a.iter().filter(|id| !ids_b.contains(id)).map(|id| (*id).to_owned()).collect(),
        only_in_b: ids_b.iter().filter(|id| !ids_a.contains(id)).map(|id| (*id).to_owned()).collect(),
        summary: summarize(&episodes),
        episodes,
    }
}

/// Pairs each cut of `a` with the not-yet-paired cut of `b` it overlaps most.
fn compare_episode(id: &str, cuts_a: &[Cut], cuts_b: &[Cut], duration: f64) -> EpisodeComparison {
    let mut unmatched_b: Vec<&Cut> = cuts_b.iter().collect();
    let mut matched = Vec::new();
    let mut only_in_a = Vec::new();
    for cut_a in cuts_a {
        let best = unmatched_b
            .iter()
            .enumerate()
            .map(|(index, cut_b)| (index, overlap(cut_a, cut_b, duration)))
            .filter(|(_, shared)| *shared > 0.0)
            .max_by(|x, y| x.1.total_cmp(&y.1));
        let Some((index, shared)) = best else {
            only_in_a.push(span(cut_a, duration));
            continue;
        };
        let cut_b = unmatched_b.remove(index);
        matched.push(MatchedCut {
            start_delta: round(cut_b.start.seconds() - cut_a.start.seconds()),
            end_delta: cut_a.end.zip(cut_b.end).map(|(end_a, end_b)| round(end_b.seconds() - end_a.seconds())),
            shared_seconds: round(shared),
            a: span(cut_a, duration),
            b: span(cut_b, duration),
        });
    }
    EpisodeComparison { id: id.to_owned(), matched, only_in_a, only_in_b: unmatched_b.into_iter().map(|cut| span(cut, duration)).collect() }
}

fn bounds(cut: &Cut, duration: f64) -> (f64, f64) {
    (cut.start.seconds(), cut.end.map_or(duration, Time::seconds))
}

fn overlap(a: &Cut, b: &Cut, duration: f64) -> f64 {
    let ((start_a, end_a), (start_b, end_b)) = (bounds(a, duration), bounds(b, duration));
    (end_a.min(end_b) - start_a.max(start_b)).max(0.0)
}

fn span(cut: &Cut, duration: f64) -> CutSpan {
    let (start, end) = bounds(cut, duration);
    CutSpan { start: cut.start, end: cut.end, seconds: round(end - start), reason: cut.reason.clone() }
}

fn summarize(episodes: &[EpisodeComparison]) -> Summary {
    let matched = || episodes.iter().flat_map(|episode| &episode.matched);
    let mut deltas: Vec<f64> =
        matched().flat_map(|cut| std::iter::once(cut.start_delta.abs()).chain(cut.end_delta.map(f64::abs))).collect();
    deltas.sort_by(f64::total_cmp);
    let unmatched_a: f64 = episodes.iter().flat_map(|episode| &episode.only_in_a).map(|cut| cut.seconds).sum();
    let unmatched_b: f64 = episodes.iter().flat_map(|episode| &episode.only_in_b).map(|cut| cut.seconds).sum();
    Summary {
        matched: matched().count(),
        only_in_a: episodes.iter().map(|episode| episode.only_in_a.len()).sum(),
        only_in_b: episodes.iter().map(|episode| episode.only_in_b.len()).sum(),
        seconds_cut_only_by_a: round(unmatched_a + matched().map(|cut| cut.a.seconds - cut.shared_seconds).sum::<f64>()),
        seconds_cut_only_by_b: round(unmatched_b + matched().map(|cut| cut.b.seconds - cut.shared_seconds).sum::<f64>()),
        median_edge_delta: deltas.get(deltas.len() / 2).copied(),
    }
}

fn round(seconds: f64) -> f64 {
    (seconds * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::PlanEpisode;

    fn cut(start: f64, end: Option<f64>) -> Cut {
        Cut { start: Time(start), end: end.map(Time), reason: String::new(), fade_out: None, fade_in: None }
    }

    fn plan(cuts: Vec<Cut>) -> Plan {
        let episode = PlanEpisode { id: "e".into(), cuts, chapters: Vec::new(), interstitial_after: true, note: None };
        Plan { show: "s".into(), title: "t".into(), output: "o.mp3".into(), interstitial: None, episodes: vec![episode] }
    }

    #[test]
    fn pairs_overlapping_cuts_and_counts_what_only_one_side_removes() {
        let drafted = plan(vec![cut(0.0, Some(53.0)), cut(1763.0, Some(1832.0)), cut(4966.0, Some(5144.0)), cut(5494.0, None)]);
        let by_hand = plan(vec![cut(0.0, Some(50.5)), cut(1762.5, Some(1830.8)), cut(5502.0, None)]);
        let durations = BTreeMap::from([("e".to_owned(), 5560.0)]);
        let comparison = compare(&drafted, &by_hand, &durations);
        let episode = &comparison.episodes[0];
        assert_eq!(episode.matched.len(), 3);
        assert_eq!(episode.only_in_a.len(), 1);
        assert_eq!(episode.only_in_a[0].start, Time(4966.0));
        assert_eq!(episode.matched[0].end_delta, Some(-2.5));
        assert_eq!(episode.matched[2].end_delta, None, "open-ended cuts have no end delta");
        // The draft alone removes 4966-5144 plus 2.5 s, 1.2 s and 8 s at matched edges.
        assert_eq!(comparison.summary.seconds_cut_only_by_a, 189.7);
        assert_eq!(comparison.summary.seconds_cut_only_by_b, 0.5);
    }
}
