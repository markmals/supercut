use std::path::PathBuf;

use crate::commands::Context;
use crate::output::print_json;
use crate::plan::Plan;
use crate::render::supercut::{Request, render};

/// Render a plan into the finished supercut MP3 with loudness matching, tags and chapters.
///
/// One ffmpeg pass: every kept stretch is decoded, brought to the configured target loudness,
/// faded at its cut edges, joined with the interstitial between episodes and limited at the
/// configured true peak, then encoded at the sources' own bit rate and sample rate. The file is
/// written beside its destination and renamed into place, then tagged with the plan's title, the
/// show's cover art and a chapter per episode (segment chapters nested inside it). Needs every
/// episode downloaded; run `plans validate` first. Minutes of CPU for a ten-episode supercut.
#[derive(clap::Args)]
pub struct Args {
    /// Path to the plan's JSON file.
    plan: PathBuf,
    /// Write here instead of `<output_dir>/<show>/<the plan's output name>`.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Overwrite an existing output file.
    #[arg(long)]
    force: bool,
    /// Report what would be written — path, duration, bit rate, chapters — and encode nothing.
    #[arg(long)]
    dry_run: bool,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let plan = Plan::load(&args.plan)?;
    let request = Request { plan: &plan, out: args.out, force: args.force, dry_run: args.dry_run };
    let rendered = render(context, request)?;
    print_json(&rendered)
}
