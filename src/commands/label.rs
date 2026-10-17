use crate::commands::Context;

/// Label every stretch of episodes with the show's segments using Jev.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`label` is not built yet")
}
