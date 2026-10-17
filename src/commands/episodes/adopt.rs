use crate::commands::Context;

/// Adopt audio files you already have into the library instead of downloading them again.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`episodes adopt` is not built yet")
}
