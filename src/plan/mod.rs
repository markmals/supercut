pub mod check;
pub mod lint;
pub mod resolve;
pub mod review;
pub mod timeline;

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::timecode::Time;

/// Fade, in seconds, applied at a cut edge that names no fade of its own, so splices never click.
pub const EDGE_FADE_SECONDS: f64 = 0.01;

/// A supercut: which episodes, in what order, and what to cut from each. Times are positions in
/// the episode's own audio file.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub show: String,
    /// Title tag of the rendered file.
    pub title: String,
    /// File name of the rendered supercut inside `<output_dir>/<show>/`.
    pub output: String,
    /// Clip between episodes: absent or `true` uses the show's configured clip, `false` plays
    /// nothing, a path plays that file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interstitial: Option<InterstitialSetting>,
    pub episodes: Vec<PlanEpisode>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum InterstitialSetting {
    Enabled(bool),
    File(PathBuf),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlanEpisode {
    pub id: String,
    /// Ranges removed from the episode, in order and not overlapping.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cuts: Vec<Cut>,
    /// Finer chapters inside the episode (segments), by position in the episode's audio. The
    /// episode itself always gets a chapter carrying its title and show notes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chapters: Vec<Chapter>,
    /// Free-form note for the reviewer; not rendered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cut {
    pub start: Time,
    /// Absent: cut to the end of the episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<Time>,
    pub reason: String,
    /// Seconds of fade-out on the audio that ends at `start`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_out: Option<f64>,
    /// Seconds of fade-in on the audio that resumes at `end`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_in: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Chapter {
    pub start: Time,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Plan {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        crate::library::read_json(path).with_context(|| format!("loading plan {}", path.display()))
    }
}
