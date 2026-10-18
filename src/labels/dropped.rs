//! Which labeled segments an episode loses, and what pins each cut's edges. A segment of a kind
//! the show cuts goes when the labels are sure of it. One they are unsure of stays unless the
//! episode vouches for it: the show's phrases fence it, or it is a closing spoken before the
//! outro plays. A segment the command line names goes whatever its probability.
//!
//! The kind's phrases pin a cut's edges: the announcement and the closing phrase that fence it,
//! or without a fence, the announcement nearest its start and the first closing phrase after.

use crate::config::SegmentKind;
use crate::labels::also_cut::AlsoCut;
use crate::labels::closing;
use crate::labels::phrases::{self, Spoken};
use crate::labels::record::LabeledSegment;
use crate::labels::refine::{Anchors, Policy};

pub struct Dropped<'a> {
    pub kinds: &'a [SegmentKind],
    pub episode: &'a str,
    pub also: &'a AlsoCut,
    pub spoken: &'a [Spoken],
    /// Where the episode's outro starts, when one plays.
    pub outro: Option<f64>,
}

impl Dropped<'_> {
    /// What pins the edges of the segment's cut, or nothing when it stays.
    pub fn cut(&self, segment: &LabeledSegment) -> Option<Anchors> {
        let named = self.also.contains(self.episode, &segment.label);
        let kind = self.kind(&segment.label).filter(|kind| kind.cut || named)?;
        let bounds = (segment.start.seconds(), segment.end.seconds());
        let fenced = phrases::fence(self.spoken, &kind.after_phrases, &kind.before_phrases, bounds);
        let sure = named || !crate::labels::uncertain(segment);
        if !(sure || fenced.is_some() || self.closes(segment)) {
            return None;
        }
        Some(match fenced {
            Some((start_after, end_before)) => {
                Anchors { start_after: Some(start_after), end_before: Some(end_before) }
            }
            None => self.anchors(kind, bounds),
        })
    }

    /// Whether the segment is a closing spoken before the outro, labeled from well before the
    /// outro into it.
    pub fn closes(&self, segment: &LabeledSegment) -> bool {
        let bounds = (segment.start.seconds(), segment.end.seconds());
        self.outro.is_some_and(|outro| closing::spoken_before(bounds.0, bounds.1, outro))
    }

    /// Whether the draft has to decide the segment's fate: it is of a kind the show cuts, or one
    /// the command line names.
    pub fn at_stake(&self, segment: &LabeledSegment) -> bool {
        self.kind(&segment.label).is_some_and(|kind| kind.cut)
            || self.also.contains(self.episode, &segment.label)
    }

    pub fn policy(&self, label: &str) -> Policy {
        self.kind(label)
            .map(|kind| Policy { start: kind.cut_start, end: kind.cut_end })
            .unwrap_or_default()
    }

    pub fn title(&self, label: &str) -> Option<&str> {
        self.kind(label).map(|kind| kind.title.as_str())
    }

    fn kind(&self, label: &str) -> Option<&SegmentKind> {
        self.kinds.iter().find(|kind| kind.label == label)
    }

    /// The edges the segment kind's phrases pin down around `bounds`, if it names any.
    fn anchors(&self, kind: &SegmentKind, bounds: (f64, f64)) -> Anchors {
        let start_after = phrases::start_after(self.spoken, &kind.after_phrases, bounds.0);
        let end_before = phrases::end_before(
            self.spoken,
            &kind.before_phrases,
            start_after.unwrap_or(bounds.0),
            bounds.1,
        );
        Anchors { start_after, end_before }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CutEnd, CutStart};
    use crate::labels::phrases::said;
    use crate::timecode::Time;

    fn kind(label: &str, after: &[&str], before: &[&str]) -> SegmentKind {
        let phrases = |phrases: &[&str]| phrases.iter().map(|phrase| (*phrase).into()).collect();
        SegmentKind {
            label: label.into(),
            title: label.into(),
            describe: String::new(),
            cut: true,
            cut_start: CutStart::default(),
            cut_end: CutEnd::default(),
            after_phrases: phrases(after),
            before_phrases: phrases(before),
        }
    }

    fn segment(label: &str, start: f64, end: f64, probability: f64) -> LabeledSegment {
        LabeledSegment {
            label: label.into(),
            start: Time(start),
            end: Time(end),
            probability,
            min_confidence: probability,
        }
    }

    #[test]
    fn an_unsure_ad_goes_when_the_hosts_announce_it_and_come_back_from_it() {
        let kinds = [kind("ads", &["be back"], &["welcome back"])];
        let words = said(&[
            ("be", 1748.0),
            ("back.", 1748.3),
            ("Hi,", 1764.6),
            ("Welcome", 1810.0),
            ("back.", 1810.3),
            ("Hi,", 2400.0),
        ]);
        let also = AlsoCut::default();
        let dropped =
            Dropped { kinds: &kinds, episode: "e1", also: &also, spoken: &words, outro: None };
        let fenced = dropped.cut(&segment("ads", 1745.0, 1812.0, 0.55)).unwrap();
        assert_eq!(fenced.end_before, Some(1810.0));
        assert!(dropped.cut(&segment("ads", 2390.0, 2450.0, 0.55)).is_none(), "nothing fences it");
        assert!(dropped.cut(&segment("ads", 2390.0, 2450.0, 0.9)).is_some(), "the labels are sure");
    }

    #[test]
    fn an_unsure_closing_goes_when_it_was_spoken_well_before_the_outro() {
        let kinds = [kind("credits", &[], &[])];
        let also = AlsoCut::default();
        let dropped =
            Dropped { kinds: &kinds, episode: "e1", also: &also, spoken: &[], outro: Some(4078.3) };
        // Credits read aloud for 29 s before the closing music, labeled to the end of the file.
        assert!(dropped.cut(&segment("credits", 4049.4, 4106.0, 0.56)).is_some());
        // The labels' reading of the goodbye as credits a few seconds before the outro.
        assert!(dropped.cut(&segment("credits", 4070.0, 4106.0, 0.56)).is_none());
    }
}
