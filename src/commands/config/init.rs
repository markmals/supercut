use crate::commands::Context;

/// Write a commented starter config file at the config path.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`config init` is not built yet")
}
