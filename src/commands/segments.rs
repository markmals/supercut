use crate::commands::Context;

/// Print the labeled segments of an episode.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`segments` is not built yet")
}
