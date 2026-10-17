//! Recovering the cuts of supercuts made by hand, by finding where each stretch of an exported
//! supercut came from in the source episodes.

pub mod fingerprint;
pub mod matcher;

use std::path::Path;

use crate::library::{Episode, ShowLibrary};

/// A stretch of the export that plays a stretch of one source episode, in fingerprint items.
#[derive(Clone, Debug)]
pub struct CoarsePiece {
    pub episode: String,
    pub run: matcher::Run,
}

/// Matches the export against every candidate episode and keeps, for each export stretch, the
/// best-matching source.
pub fn coarse_alignment(show: &ShowLibrary, export: &Path, export_cache: &Path, episodes: &[&Episode]) -> anyhow::Result<Vec<CoarsePiece>> {
    let export_fp = fingerprint::cached(export, export_cache)?;
    let mut runs = Vec::new();
    for episode in episodes {
        let source_fp = fingerprint::cached(&show.audio_path(episode)?, &show.fingerprint_path(&episode.id))?;
        runs.extend(matcher::find_runs(&source_fp, &export_fp).into_iter().map(|run| (episode.id.clone(), run)));
    }
    let pieces = matcher::best_non_overlapping(runs).into_iter().map(|(episode, run)| CoarsePiece { episode, run });
    Ok(pieces.collect())
}
