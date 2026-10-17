use crate::commands::Context;

/// List the hand-made supercuts recovered for a show.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`history list` is not built yet")
}
