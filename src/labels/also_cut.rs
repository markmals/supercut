//! `--also-cut episode=label`: segments one episode drops that the show's config keeps.

use std::collections::{BTreeMap, BTreeSet};

use crate::config::SegmentKind;
use crate::error::CliError;
use crate::library::Episode;

/// Extra labels to cut, per episode.
#[derive(Debug, Default)]
pub struct AlsoCut(BTreeMap<String, BTreeSet<String>>);

impl AlsoCut {
    /// Reads `episode=label` pairs, checking both halves against the draft.
    pub fn parse(
        values: &[String],
        episodes: &[&Episode],
        kinds: &[SegmentKind],
    ) -> anyhow::Result<Self> {
        let mut chosen: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for value in values {
            let (episode, label) = value.split_once('=').ok_or_else(|| malformed(value))?;
            check_episode(episode, episodes)?;
            check_label(label, kinds)?;
            chosen.entry(episode.to_owned()).or_default().insert(label.to_owned());
        }
        Ok(Self(chosen))
    }

    pub fn contains(&self, episode: &str, label: &str) -> bool {
        self.0.get(episode).is_some_and(|labels| labels.contains(label))
    }
}

fn malformed(value: &str) -> anyhow::Error {
    CliError::invalid(format!("`--also-cut` takes `<episode-id>=<label>`, not `{value}`")).into()
}

fn check_episode(id: &str, episodes: &[&Episode]) -> anyhow::Result<()> {
    if episodes.iter().any(|episode| episode.id == id) {
        return Ok(());
    }
    let drafted: Vec<String> = episodes.iter().map(|episode| episode.id.clone()).collect();
    Err(CliError::invalid(format!("`--also-cut` names `{id}`, which this draft does not cover"))
        .detail("episodes", drafted)
        .into())
}

fn check_label(label: &str, kinds: &[SegmentKind]) -> anyhow::Result<()> {
    if kinds.iter().any(|kind| kind.label == label) {
        return Ok(());
    }
    let known: Vec<String> = kinds.iter().map(|kind| kind.label.clone()).collect();
    Err(CliError::invalid(format!(
        "`--also-cut` names segment `{label}`; the show has {}",
        known.join(", ")
    ))
    .detail("labels", known)
    .into())
}
