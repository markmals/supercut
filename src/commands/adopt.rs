use crate::commands::Context;

/// Adopt audio files you already downloaded into the library instead of downloading them again.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`adopt` is not built yet")
}
