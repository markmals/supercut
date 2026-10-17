//! What `labels/<id>.json` holds: the windows Jev answered and the segments they smooth into.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::config::SegmentKind;
use crate::timecode::Time;

/// One episode's labels.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Labels {
    pub episode: String,
    /// The Jev model that answered; probabilities shift between releases.
    pub model: String,
    /// The segments the show was configured with when these labels were made. Labels whose kinds
    /// differ from the config are stale.
    pub kinds: Vec<LabeledKind>,
    /// Every ~30 s window of the coarse pass, with what Jev answered about it.
    pub windows: Vec<LabeledUnit>,
    /// The sequence the segments were smoothed from: utterances where the fine pass ran,
    /// windows elsewhere.
    #[serde(default)]
    pub units: Vec<LabeledUnit>,
    pub segments: Vec<LabeledSegment>,
}

/// A copy of one configured segment, as it was when the episode was labeled.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LabeledKind {
    pub label: String,
    pub title: String,
    pub describe: String,
    #[serde(default)]
    pub cut: bool,
}

/// What Jev answered about one unit, coarse or fine.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LabeledUnit {
    /// Start of the unit's first word.
    pub start: Time,
    /// End of its last word.
    pub end: Time,
    pub selected: String,
    pub confidence: f64,
    pub probabilities: BTreeMap<String, f64>,
}

impl LabeledUnit {
    pub fn seconds(&self) -> f64 {
        self.end.seconds() - self.start.seconds()
    }
}

/// A run of units that smoothed into one segment of the episode.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LabeledSegment {
    pub label: String,
    pub start: Time,
    pub end: Time,
    /// Mean probability of this label across the segment's units.
    pub probability: f64,
    /// The least confident unit in the segment.
    pub min_confidence: f64,
}

impl Labels {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        crate::library::read_json(path).with_context(|| format!("loading labels {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        crate::library::write_json_atomically(path, self)
    }

    /// Were these labels made from the show's current segment labels and descriptions (all Jev
    /// sees), by a labeler that ran the fine pass? Titles, cut flags and edge policies only
    /// shape the draft, so changing them needs no relabeling.
    pub fn matches(&self, kinds: &[SegmentKind]) -> bool {
        !self.units.is_empty()
            && self.kinds.len() == kinds.len()
            && self.kinds.iter().zip(kinds).all(|(stored, current)| stored.label == current.label && stored.describe == current.describe)
    }
}

pub fn kind_of(kind: &SegmentKind) -> LabeledKind {
    LabeledKind {
        label: kind.label.clone(),
        title: kind.title.clone(),
        describe: kind.describe.clone(),
        cut: kind.cut,
    }
}
