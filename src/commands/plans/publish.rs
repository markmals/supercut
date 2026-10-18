use std::path::PathBuf;

use crate::commands::Context;
use crate::output::print_json;
use crate::plan::Plan;
use crate::render::supercut::default_output;

/// Copy a rendered supercut into every configured publish folder, named after its title.
///
/// Reads the file `plans render` wrote for the plan (`<output_dir>/<show>/<the plan's output
/// name>`, or `--rendered` when `render --out` put it elsewhere) and copies it into each folder
/// in the config's `publish_folders`, e.g. a podcast app's sideload folder, as `<title>.mp3`
/// ("Show: Part 2" becomes "Show - Part 2.mp3"). Each copy is written beside its destination and
/// renamed into place. Refuses to replace an existing copy without `--force`.
#[derive(clap::Args)]
pub struct Args {
    /// Path to the plan's JSON file.
    plan: PathBuf,
    /// The rendered supercut, when `plans render --out` wrote it somewhere else.
    #[arg(long)]
    rendered: Option<PathBuf>,
    /// Replace copies already in the publish folders.
    #[arg(long)]
    force: bool,
    /// Report the copies that would be made, copy nothing.
    #[arg(long)]
    dry_run: bool,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let plan = Plan::load(&args.plan)?;
    context.config.show(&plan.show)?;
    let source = match args.rendered {
        Some(path) => path,
        None => default_output(context, &plan)?,
    };
    let published = crate::publish::publish(context, &plan, source, args.force, args.dry_run)?;
    print_json(&published)
}
