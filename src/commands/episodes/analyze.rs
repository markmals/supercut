use crate::commands::Context;

/// Measure episodes' loudness and level envelope (cached; other commands run it on demand).
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`episodes analyze` is not built yet")
}
