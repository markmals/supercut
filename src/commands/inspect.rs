use crate::commands::Context;

/// Show the audio around a moment of an episode: levels, words and quiet spans, for placing cuts.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`inspect` is not built yet")
}
