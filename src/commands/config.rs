use crate::commands::Context;

/// Show where supercut keeps its config and library, or write a starter config.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`config` is not built yet")
}
