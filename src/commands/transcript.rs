use crate::commands::Context;

/// Print an episode's transcript with timestamps.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`transcript` is not built yet")
}
