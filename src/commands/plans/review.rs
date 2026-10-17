use std::path::PathBuf;

use crate::commands::Context;
use crate::output::{Format, print_json};
use crate::plan::{Plan, review};

/// Summarize a cut plan for a human reviewer, with transcript context around each cut.
///
/// For every episode: its title, how much audio survives, each cut with its times, length,
/// reason and fades, and — where the episode has a transcript — the words heard before the
/// splice, the words that disappear and the words that follow. Chapters are listed in the time
/// they play in the finished file. `--output text` prints the same thing as Markdown.
#[derive(clap::Args)]
pub struct Args {
    /// Path to the plan's JSON file.
    plan: PathBuf,
    /// `json` for the structured review, `text` for Markdown to read.
    #[arg(long, value_enum, default_value_t = Format::Json)]
    output: Format,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let plan = Plan::load(&args.plan)?;
    let review = review::review(context, &plan)?;
    match args.output {
        Format::Json => print_json(&review),
        Format::Text => {
            print!("{}", review::markdown(&review));
            Ok(())
        }
    }
}
