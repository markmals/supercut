use std::path::PathBuf;

use crate::commands::Context;
use crate::output::print_json;
use crate::plan::Plan;
use crate::render::preview::{self, Request};

/// Render every transition of a plan into one short MP3 for listening.
///
/// Plays a few seconds either side of every cut, and the end of each episode with the
/// interstitial and the start of the next, each separated by a quiet marker tone. The gains and
/// fades are the ones the finished supercut will use, so a splice that clicks here will click
/// there. Writes into the show's `previews/` folder unless `--out` says otherwise, and prints an
/// index that maps each position in the reel back to the cut it came from.
#[derive(clap::Args)]
pub struct Args {
    /// Path to the plan's JSON file.
    plan: PathBuf,
    /// Seconds of audio played either side of each splice.
    #[arg(long, default_value_t = 6.0)]
    context: f64,
    /// Write the reel here instead of `<library>/<show>/previews/<plan output>-transitions.mp3`.
    #[arg(long)]
    out: Option<PathBuf>,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let plan = Plan::load(&args.plan)?;
    let request = Request { plan: &plan, context: args.context, out: args.out };
    let preview = preview::preview(context, request)?;
    print_json(&preview)
}
