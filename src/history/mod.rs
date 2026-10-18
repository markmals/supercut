//! Recovering the cuts of supercuts made by hand, by finding where each stretch of an exported
//! supercut came from in the source episodes.

pub mod fingerprint;
mod matcher;
mod pieces;
mod recover;
mod refine;
pub mod report;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context as _;

pub use pieces::Piece;
pub use recover::Recovery;

use crate::library::{Episode, EpisodeIndex, ShowLibrary};
use crate::plan::Plan;

/// Pieces of one episode this close in offset and position are one stretch the fingerprints split.
const MERGE_OFFSET_SECONDS: f64 = 0.02;
const MERGE_GAP_SECONDS: f64 = 1.0;

pub struct Saved {
    pub recovery: Recovery,
    pub recovery_path: PathBuf,
    pub plan_path: PathBuf,
}

pub fn recovery_path(show: &ShowLibrary, name: &str) -> PathBuf {
    show.history_dir().join(format!("{name}.json"))
}

pub fn plan_path(show: &ShowLibrary, name: &str) -> PathBuf {
    show.history_dir().join(format!("{name}.plan.json"))
}

/// Matches `export` against `candidates`, refines every match to the sample, and saves the
/// recovery and the plan that reproduces the export.
pub fn recover(
    show: &ShowLibrary,
    index: &EpisodeIndex,
    export: &Path,
    candidates: &[&Episode],
) -> anyhow::Result<Saved> {
    let name =
        export.file_stem().context("export has no file name")?.to_string_lossy().into_owned();
    let export_fingerprint = show.history_dir().join(format!("{name}.fp"));
    let coarse = coarse_alignment(show, export, &export_fingerprint, candidates)?;
    let item = fingerprint::item_seconds();
    let mut refined = Vec::with_capacity(coarse.len());
    for (position, piece) in coarse.iter().enumerate() {
        eprintln!("refining piece {}/{} ({})", position + 1, coarse.len(), piece.episode);
        let source = show.audio_path(index.get(&piece.episode)?)?;
        let estimate = pieces::Estimate {
            episode: &piece.episode,
            export_start: piece.run.export_start as f64 * item,
            export_end: piece.run.export_end() as f64 * item,
            source_start: piece.run.source_start as f64 * item,
        };
        refined.push(pieces::refine_piece(export, &source, &estimate)?);
    }
    let duration = crate::media::probe(export)?.duration;
    let recovery = recover::classify(
        &name,
        &export.display().to_string(),
        duration,
        merge_split_pieces(refined),
    );
    let mut durations = BTreeMap::new();
    for piece in &recovery.pieces {
        if !durations.contains_key(&piece.episode) {
            let analysis = crate::analysis::episode_analysis(show, index.get(&piece.episode)?)?;
            durations.insert(piece.episode.clone(), analysis.duration);
        }
    }
    let plan = recover::recovered_plan(&show.slug, &recovery, &durations);
    let saved = Saved {
        recovery_path: recovery_path(show, &name),
        plan_path: plan_path(show, &name),
        recovery,
    };
    crate::library::write_json_atomically(&saved.recovery_path, &saved.recovery)?;
    crate::library::write_json_atomically(&saved.plan_path, &plan)?;
    Ok(saved)
}

pub fn load(show: &ShowLibrary, name: &str) -> anyhow::Result<(Recovery, Plan)> {
    Ok((
        crate::library::read_json(&recovery_path(show, name))?,
        Plan::load(&plan_path(show, name))?,
    ))
}

/// A stretch of the export that plays a stretch of one source episode, in fingerprint items.
struct CoarsePiece {
    episode: String,
    run: matcher::Run,
}

/// Matches the export against every candidate episode and keeps, for each export stretch, the
/// best-matching source.
fn coarse_alignment(
    show: &ShowLibrary,
    export: &Path,
    export_cache: &Path,
    episodes: &[&Episode],
) -> anyhow::Result<Vec<CoarsePiece>> {
    let export_fp = fingerprint::cached(export, export_cache)?;
    let mut runs = Vec::new();
    for episode in episodes {
        let source_fp =
            fingerprint::cached(&show.audio_path(episode)?, &show.fingerprint_path(&episode.id))?;
        runs.extend(
            matcher::find_runs(&source_fp, &export_fp)
                .into_iter()
                .map(|run| (episode.id.clone(), run)),
        );
    }
    let pieces = matcher::best_non_overlapping(runs)
        .into_iter()
        .map(|(episode, run)| CoarsePiece { episode, run });
    Ok(pieces.collect())
}

fn merge_split_pieces(pieces: Vec<Piece>) -> Vec<Piece> {
    let mut merged: Vec<Piece> = Vec::with_capacity(pieces.len());
    for piece in pieces {
        if let Some(last) = merged.last_mut()
            && last.episode == piece.episode
            && (last.offset() - piece.offset()).abs() < MERGE_OFFSET_SECONDS
            && piece.export_start - last.export_end < MERGE_GAP_SECONDS
        {
            last.export_end = piece.export_end;
            last.source_end = piece.source_end;
            last.fade_out = piece.fade_out;
            last.correlation = last.correlation.min(piece.correlation);
            continue;
        }
        merged.push(piece);
    }
    merged
}
