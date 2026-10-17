use std::path::PathBuf;

use serde::Serialize;

use crate::commands::{Context, Selection};
use crate::error::CliError;
use crate::history;
use crate::timecode::Time;

/// Recover the cuts, fades and interstitials of a hand-made supercut by matching it against the source episodes.
///
/// Fingerprints the export and the candidate episodes (cached), finds every stretch of each
/// episode the export plays, then times each stretch to the sample and measures the fades at
/// its edges. Writes `history/<name>.json` (the recovery) and `history/<name>.plan.json`, a cut
/// plan that reproduces the export. Candidates default to every downloaded episode; narrowing
/// them (e.g. `--season 10`) is faster.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug.
    show: String,
    /// The exported supercut (MP3).
    export: PathBuf,
    /// Candidate source episodes (default: every downloaded episode).
    #[command(flatten)]
    candidates: Selection,
    /// Replace an earlier recovery of this export, including its plan (which you may have edited).
    #[arg(long)]
    force: bool,
}

#[derive(Serialize)]
struct Receipt {
    name: String,
    duration: Time,
    episodes: Vec<String>,
    pieces: usize,
    interstitials: Vec<InterstitialSummary>,
    unmatched_seconds: f64,
    warnings: Vec<String>,
    recovery: PathBuf,
    plan: PathBuf,
}

#[derive(Serialize)]
struct InterstitialSummary {
    at: Time,
    seconds: f64,
    sources: Vec<String>,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, show) = context.show(&args.show)?;
    if !args.export.is_file() {
        return Err(CliError::not_found(format!("no export at {}", args.export.display())).into());
    }
    let name = args.export.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    let earlier = [history::recovery_path(&show, &name), history::plan_path(&show, &name)];
    if let Some(existing) = earlier.iter().find(|path| path.exists())
        && !args.force
    {
        let conflict = CliError::conflict(format!("{name} was already recovered: {} exists", existing.display()))
            .hint("pass --force to recover it again and replace the recovery and its plan");
        return Err(conflict.into());
    }
    let index = show.load_synced_index()?;
    let candidates = if args.candidates.ids.is_empty() && args.candidates.season.is_none() {
        index.episodes.iter().filter(|episode| episode.audio_file.is_some()).collect()
    } else {
        args.candidates.resolve(&index)?
    };
    let saved = history::recover(&show, &index, &args.export, &candidates)?;
    crate::output::print_json(&receipt(saved))
}

fn receipt(saved: history::Saved) -> Receipt {
    let recovery = saved.recovery;
    let mut episodes: Vec<String> = Vec::new();
    for piece in &recovery.pieces {
        if !episodes.contains(&piece.episode) {
            episodes.push(piece.episode.clone());
        }
    }
    let interstitials = recovery
        .interstitials
        .iter()
        .map(|interstitial| InterstitialSummary {
            at: Time(interstitial.export_start),
            seconds: round(interstitial.export_end - interstitial.export_start),
            sources: interstitial
                .sources
                .iter()
                .map(|source| format!("{} {}-{}", source.episode, Time(source.source_start), Time(source.source_end)))
                .collect(),
        })
        .collect();
    Receipt {
        name: recovery.name,
        duration: Time(recovery.duration),
        episodes,
        pieces: recovery.pieces.len(),
        interstitials,
        unmatched_seconds: round(recovery.unmatched.iter().map(|span| span.export_end - span.export_start).sum()),
        warnings: recovery.warnings,
        recovery: saved.recovery_path,
        plan: saved.plan_path,
    }
}

fn round(seconds: f64) -> f64 {
    (seconds * 1000.0).round() / 1000.0
}
