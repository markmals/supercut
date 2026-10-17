use crate::commands::Context;

/// Label every stretch of episodes with the show's segments using the Jev decision model (paid API).
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`labels create` is not built yet")
}
