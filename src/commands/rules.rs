use anyhow::{Context as _, ensure};

use crate::commands::Context;

/// Print a show's editorial rules file (and its path).
///
/// The rules are plain Markdown you write: what the show's segments are and what to cut.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as it appears in the config file.
    show: String,
    /// Print where the file lives instead of what it says.
    #[arg(long)]
    path: bool,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    context.config.show(&args.show)?;
    let path = context.rules_path(&args.show);
    if args.path {
        println!("{}", path.display());
        return Ok(());
    }
    ensure!(path.exists(), "no rules for `{}`; write them in {}", args.show, path.display());
    let rules =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    println!("{}", rules.trim_end());
    Ok(())
}
