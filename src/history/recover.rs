//! Turns refined pieces into what the editor did: which episodes, which stretches were kept,
//! what was cut with which fades, and what was played between episodes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::pieces::Piece;
use crate::plan::{Cut, InterstitialSetting, Plan, PlanEpisode};
use crate::timecode::Time;

/// Pieces shorter than this can be music stings rather than kept content.
const STING_SECONDS: f64 = 30.0;
/// Unmatched export audio shorter than this is a splice artifact, not inserted material.
const MIN_GAP_SECONDS: f64 = 0.5;
/// Material between episodes longer than this is not an interstitial: its source is missing.
const MAX_INTERSTITIAL_SECONDS: f64 = 60.0;
/// Fades shorter than this are just the edge of the audio, not a deliberate fade.
const MIN_FADE_SECONDS: f64 = 0.05;

#[derive(Debug, Deserialize, Serialize)]
pub struct Recovery {
    pub name: String,
    pub export: String,
    pub duration: f64,
    /// Kept stretches of episodes, in export order.
    pub pieces: Vec<Piece>,
    /// Material played between episodes (matched to a source or not).
    pub interstitials: Vec<Interstitial>,
    /// Export audio inside an episode that matches no source.
    pub unmatched: Vec<Span>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Span {
    pub export_start: f64,
    pub export_end: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Interstitial {
    pub export_start: f64,
    pub export_end: f64,
    /// Where the matched parts of it come from.
    pub sources: Vec<Piece>,
}

/// Splits pieces into episode content and interstitial material, and finds unmatched audio.
pub fn classify(name: &str, export: &str, duration: f64, pieces: Vec<Piece>) -> Recovery {
    let sting: Vec<bool> = (0..pieces.len()).map(|index| is_sting(&pieces, index)).collect();
    let mut recovery = Recovery {
        name: name.to_owned(),
        export: export.to_owned(),
        duration,
        pieces: Vec::new(),
        interstitials: Vec::new(),
        unmatched: Vec::new(),
        warnings: Vec::new(),
    };
    let mut pending: Option<Interstitial> = None;
    let mut cursor = 0.0;
    let mut previous_episode: Option<String> = None;
    for (piece, is_sting) in pieces.into_iter().zip(sting) {
        let gap = Span { export_start: cursor, export_end: piece.export_start };
        cursor = cursor.max(piece.export_end);
        let between_episodes = previous_episode.as_deref() != Some(piece.episode.as_str()) || is_sting;
        if gap.export_end - gap.export_start >= MIN_GAP_SECONDS {
            if between_episodes {
                extend(&mut pending, gap.export_start, gap.export_end, None);
            } else {
                recovery.unmatched.push(gap);
            }
        }
        if is_sting {
            extend(&mut pending, piece.export_start, piece.export_end, Some(piece));
            continue;
        }
        if let Some(interstitial) = pending.take() {
            close(&mut recovery, interstitial);
        }
        previous_episode = Some(piece.episode.clone());
        recovery.pieces.push(piece);
    }
    // Whatever plays after the last matched piece is outro material or a missing episode too.
    if duration - cursor >= MIN_GAP_SECONDS {
        extend(&mut pending, cursor, duration, None);
    }
    if let Some(interstitial) = pending.take() {
        close(&mut recovery, interstitial);
    }
    recovery
}

/// Files material between episodes as an interstitial, or as unmatched audio with a warning
/// when it is too long to be one (an episode the candidates did not include).
fn close(recovery: &mut Recovery, interstitial: Interstitial) {
    let seconds = interstitial.export_end - interstitial.export_start;
    if seconds <= MAX_INTERSTITIAL_SECONDS {
        recovery.interstitials.push(interstitial);
        return;
    }
    recovery.warnings.push(format!(
        "{} of the export from {} matches no candidate episode; its source is probably not downloaded",
        Time(seconds),
        Time(interstitial.export_start)
    ));
    recovery.unmatched.push(Span { export_start: interstitial.export_start, export_end: interstitial.export_end });
}

fn extend(pending: &mut Option<Interstitial>, start: f64, end: f64, source: Option<Piece>) {
    let interstitial = pending.get_or_insert(Interstitial { export_start: start, export_end: end, sources: Vec::new() });
    interstitial.export_start = interstitial.export_start.min(start);
    interstitial.export_end = interstitial.export_end.max(end);
    interstitial.sources.extend(source);
}

/// A short piece is a sting when its audio is also used elsewhere in the export, or when it
/// sits between stretches of other episodes.
fn is_sting(pieces: &[Piece], index: usize) -> bool {
    let piece = &pieces[index];
    if piece.duration() >= STING_SECONDS {
        return false;
    }
    let reused = pieces.iter().enumerate().any(|(other_index, other)| {
        other_index != index
            && other.episode == piece.episode
            && other.duration() > piece.duration()
            && other.source_start < piece.source_end
            && piece.source_start < other.source_end
    });
    let long_episode = |other: &&Piece| other.duration() >= STING_SECONDS;
    let before = pieces[..index].iter().rev().find(long_episode).map(|other| other.episode.as_str());
    let after = pieces[index + 1..].iter().find(long_episode).map(|other| other.episode.as_str());
    let isolated = before != Some(piece.episode.as_str()) && after != Some(piece.episode.as_str());
    reused || isolated
}

/// The plan that reproduces the export: episodes in export order, cuts = everything not kept.
pub fn recovered_plan(show: &str, recovery: &Recovery, durations: &BTreeMap<String, f64>) -> Plan {
    let mut order: Vec<&str> = Vec::new();
    for piece in &recovery.pieces {
        if !order.contains(&piece.episode.as_str()) {
            order.push(&piece.episode);
        }
    }
    let episodes = order
        .into_iter()
        .map(|id| {
            let mut kept: Vec<&Piece> = recovery.pieces.iter().filter(|piece| piece.episode == id).collect();
            kept.sort_by(|a, b| a.source_start.total_cmp(&b.source_start));
            let duration = durations.get(id).copied().unwrap_or(f64::INFINITY);
            PlanEpisode { id: id.to_owned(), cuts: cuts_between(&kept, duration, &recovery.name), chapters: Vec::new(), note: None }
        })
        .collect();
    Plan {
        show: show.to_owned(),
        title: recovery.name.clone(),
        output: format!("{}.mp3", recovery.name),
        interstitial: Some(InterstitialSetting::Enabled(!recovery.interstitials.is_empty())),
        episodes,
    }
}

fn cuts_between(kept: &[&Piece], duration: f64, name: &str) -> Vec<Cut> {
    let reason = format!("recovered from {name}");
    let mut cuts = Vec::new();
    let mut previous: Option<&Piece> = None;
    for piece in kept {
        let start = previous.map_or(0.0, |previous| previous.source_end);
        if piece.source_start - start > MIN_GAP_SECONDS {
            cuts.push(Cut {
                start: Time(start),
                end: Some(Time(piece.source_start)),
                reason: reason.clone(),
                fade_out: previous.and_then(|previous| deliberate(previous.fade_out)),
                fade_in: deliberate(piece.fade_in),
            });
        }
        previous = Some(piece);
    }
    if let Some(last) = previous
        && duration - last.source_end > MIN_GAP_SECONDS
    {
        cuts.push(Cut { start: Time(last.source_end), end: None, reason, fade_out: deliberate(last.fade_out), fade_in: None });
    }
    cuts
}

fn deliberate(fade: f64) -> Option<f64> {
    (fade >= MIN_FADE_SECONDS).then_some((fade * 1000.0).round() / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(episode: &str, export_start: f64, source_start: f64, length: f64) -> Piece {
        Piece {
            episode: episode.into(),
            export_start,
            export_end: export_start + length,
            source_start,
            source_end: source_start + length,
            gain_db: 0.0,
            fade_in: 0.0,
            fade_out: 0.0,
            correlation: 1.0,
        }
    }

    #[test]
    fn reused_and_isolated_stings_become_interstitials_between_episodes() {
        let pieces = vec![
            piece("a", 0.0, 50.0, 600.0),
            piece("a", 600.0, 700.0, 500.0),
            // The sting from a's own audio, reused between episodes.
            piece("a", 1104.0, 640.0, 4.0),
            piece("b", 1110.0, 40.0, 900.0),
            // A sting from an episode that is not in the supercut.
            piece("c", 2012.0, 10.0, 5.0),
            piece("d", 2020.0, 30.0, 800.0),
        ];
        let recovery = classify("x", "x.mp3", 2820.0, pieces);
        assert_eq!(recovery.pieces.len(), 4);
        assert_eq!(recovery.interstitials.len(), 2);
        let first = &recovery.interstitials[0];
        assert_eq!((first.export_start, first.export_end), (1100.0, 1110.0), "gap + sting + gap form one interstitial");
        assert!(recovery.unmatched.is_empty(), "{:?}", recovery.unmatched);
    }

    #[test]
    fn audio_after_the_last_match_is_accounted_for() {
        let missing = classify("x", "x.mp3", 5000.0, vec![piece("a", 0.0, 50.0, 600.0)]);
        assert_eq!(missing.unmatched.len(), 1, "an unmatched hour at the end is reported");
        assert_eq!((missing.unmatched[0].export_start, missing.unmatched[0].export_end), (600.0, 5000.0));
        assert_eq!(missing.warnings.len(), 1);

        let outro = classify("x", "x.mp3", 603.0, vec![piece("a", 0.0, 50.0, 600.0)]);
        assert!(outro.unmatched.is_empty());
        assert_eq!(outro.interstitials.len(), 1, "a closing sting is material, not a missing episode");
    }

    #[test]
    fn an_hour_between_episodes_is_a_missing_source_not_an_interstitial() {
        // Between a and b the export plays an episode that was not a candidate; only its theme
        // song matched (a short sting from c).
        let pieces = vec![piece("a", 0.0, 50.0, 600.0), piece("c", 1200.0, 30.0, 10.0), piece("b", 4200.0, 40.0, 900.0)];
        let recovery = classify("x", "x.mp3", 5100.0, pieces);
        assert!(recovery.interstitials.is_empty(), "{:?}", recovery.interstitials);
        assert_eq!(recovery.unmatched.len(), 1);
        assert_eq!((recovery.unmatched[0].export_start, recovery.unmatched[0].export_end), (600.0, 4200.0));
        assert_eq!(recovery.warnings.len(), 1);
    }

    #[test]
    fn recovered_cuts_are_the_unkept_source_with_the_measured_fades() {
        let mut first = piece("a", 0.0, 50.0, 600.0);
        first.fade_out = 1.5;
        let mut second = piece("a", 600.0, 700.0, 500.0);
        second.fade_in = 0.02;
        let recovery = classify("x", "x.mp3", 1100.0, vec![first, second]);
        let durations = BTreeMap::from([("a".to_owned(), 1300.0)]);
        let plan = recovered_plan("show", &recovery, &durations);
        let cuts = &plan.episodes[0].cuts;
        assert_eq!(cuts.len(), 3);
        assert_eq!((cuts[0].start, cuts[0].end), (Time(0.0), Some(Time(50.0))));
        assert_eq!((cuts[1].start, cuts[1].end, cuts[1].fade_out, cuts[1].fade_in), (Time(650.0), Some(Time(700.0)), Some(1.5), None));
        assert_eq!((cuts[2].start, cuts[2].end), (Time(1200.0), None));
    }
}
