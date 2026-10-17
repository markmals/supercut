use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::commands::Context;
use crate::plan::{Plan, compare};

/// Compare two cut plans for the same episodes: shared cuts with their edge differences, and cuts only one plan makes.
///
/// Typical use: hold a drafted plan (`a`) against the plan recovered from a supercut you made by
/// hand (`b`, `history/<name>.plan.json`) to see where the draft disagrees with past choices.
#[derive(clap::Args)]
pub struct Args {
    /// First plan (e.g. a draft).
    a: PathBuf,
    /// Second plan (e.g. a recovered `history/<name>.plan.json`).
    b: PathBuf,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (a, b) = (Plan::load(&args.a)?, Plan::load(&args.b)?);
    let (_, show) = context.show(&a.show)?;
    let index = show.load_synced_index()?;
    let mut durations = BTreeMap::new();
    for planned in a.episodes.iter().filter(|planned| b.episodes.iter().any(|other| other.id == planned.id)) {
        let analysis = crate::analysis::episode_analysis(&show, index.get(&planned.id)?)?;
        durations.insert(planned.id.clone(), analysis.duration);
    }
    crate::output::print_json(&compare::compare(&a, &b, &durations))
}
