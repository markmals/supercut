use crate::commands::Context;

/// Print an episode's transcript, optionally only a time range.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`transcripts get` is not built yet")
}
