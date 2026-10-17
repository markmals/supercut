use crate::commands::Context;

/// Measure loudness and the level envelope of episodes (cached; other commands run it on demand).
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`analyze` is not built yet")
}
