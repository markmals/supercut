use std::path::PathBuf;

use serde::Serialize;

use crate::commands::{Context, Selection};
use crate::error::CliError;
use crate::labels::{self, AlsoCut, Uncertain};
use crate::library::write_json_atomically;
use crate::output::print_json;

/// Draft a cut plan from labeled segments: cut the show's always-cut segments and add segment chapters.
///
/// Needs labels (`supercut labels create`). Every labeled segment whose kind has `cut = true`
/// becomes a cut, its edges moved onto the pauses that fence the segment; one the labels are
/// unsure of only when the kind's phrases fence it or it closes the episode before the outro.
/// Every other segment becomes a chapter. When the show configures `themes`, each episode's
/// opening is cut through the end of its theme song instead; the first episode is cut on to the
/// hosts' first words.
/// Writes the plan to `<library>/<show>/plans/<file name stem>.json` unless `--plan` names a
/// path, and refuses to overwrite an existing plan without `--force`.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as listed by `supercut shows list`.
    show: String,

    #[command(flatten)]
    selection: Selection,

    /// Title tag of the rendered supercut.
    #[arg(long)]
    title: String,

    /// File name of the rendered supercut (the plan's `output`), e.g. `nhl-s10-pt-1.mp3`.
    #[arg(long)]
    file_name: String,

    /// Also cut every segment with this label from this episode, e.g. `s10e04=guest`. Repeatable.
    #[arg(long = "also-cut", value_name = "EPISODE=LABEL")]
    also_cut: Vec<String>,

    /// Write the plan here instead of `<library>/<show>/plans/<file name stem>.json`.
    #[arg(long)]
    plan: Option<PathBuf>,

    /// Overwrite an existing plan file.
    #[arg(long)]
    force: bool,
}

#[derive(Serialize)]
struct Receipt {
    plan: String,
    episodes: usize,
    cuts: usize,
    low_confidence: Vec<Uncertain>,
    /// Episodes whose opening no theme recording placed; check those openings by hand.
    warnings: Vec<String>,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let index = library.load_synced_index()?;
    let chosen = args.selection.resolve(&index)?;
    let episodes: Vec<_> = index
        .episodes
        .iter()
        .filter(|episode| chosen.iter().any(|one| one.id == episode.id))
        .collect();

    let path = labels::plan_path(&library, &args.file_name, args.plan);
    if path.exists() && !args.force {
        return Err(CliError::conflict(format!("{} already exists", path.display()))
            .hint("pass --force to overwrite it, or --plan to write elsewhere")
            .into());
    }
    let show = context.config.show(&args.show)?;
    let also_cut = AlsoCut::parse(&args.also_cut, &episodes, &show.segments)?;
    let draft =
        labels::draft(context, &args.show, &episodes, args.title, args.file_name, &also_cut)?;
    write_json_atomically(&path, &draft.plan)?;
    print_json(&Receipt {
        plan: path.display().to_string(),
        episodes: draft.plan.episodes.len(),
        cuts: draft.plan.episodes.iter().map(|episode| episode.cuts.len()).sum(),
        low_confidence: draft.low_confidence,
        warnings: draft.warnings,
    })
}
