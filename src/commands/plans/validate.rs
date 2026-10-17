use std::path::PathBuf;

use crate::commands::Context;
use crate::error::CliError;
use crate::output::print_json;
use crate::plan::{Plan, check};

/// Check a cut plan against the library and lint every cut edge.
///
/// Reads the plan and measures the episodes it names, analyzing any that have not been measured
/// yet (cached in the library). Errors stop a render: an unknown show, an episode that is missing
/// or not downloaded, a missing interstitial, cuts that overlap, run past the end of the episode,
/// remove a whole episode or fade for longer than the audio beside them. Warnings are judgement
/// calls: an edge that cuts through a word (transcript permitting) or lands on audio still
/// sounding, where it suggests a fade or the nearest quiet span. Changes nothing but that cache
/// and exits non-zero when there are errors.
#[derive(clap::Args)]
pub struct Args {
    /// Path to the plan's JSON file.
    plan: PathBuf,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let plan = Plan::load(&args.plan)?;
    let report = check::check(context, &plan)?;
    print_json(&report)?;
    if report.ok {
        return Ok(());
    }
    let count = report.errors.len();
    let plural = if count == 1 { "error" } else { "errors" };
    let invalid = CliError::invalid(format!("the plan has {count} {plural}"))
        .detail("path", args.plan.display().to_string())
        .hint("fix what the `errors` list names, then validate again");
    Err(invalid.into())
}
