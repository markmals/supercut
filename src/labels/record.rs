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
    pub windows: Vec<LabeledWindow>,
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

/// What Jev answered about one window.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LabeledWindow {
    /// Start of the window's first word.
    pub start: Time,
    /// End of its last word.
    pub end: Time,
    pub selected: String,
    pub confidence: f64,
    pub probabilities: BTreeMap<String, f64>,
}

/// A run of windows that smoothed into one segment of the episode.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LabeledSegment {
    pub label: String,
    pub start: Time,
    pub end: Time,
    /// Mean probability of this label across the segment's windows.
    pub probability: f64,
    /// The least confident window in the segment.
    pub min_confidence: f64,
}

impl Labels {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        crate::library::read_json(path).with_context(|| format!("loading labels {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        crate::library::write_json_atomically(path, self)
    }

    /// Were these labels made with the show's current segments?
    pub fn matches(&self, kinds: &[SegmentKind]) -> bool {
        self.kinds.len() == kinds.len()
            && self.kinds.iter().zip(kinds).all(|(stored, current)| *stored == kind_of(current))
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
