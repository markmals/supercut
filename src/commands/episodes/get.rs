use crate::commands::Context;

/// Show one episode's metadata and local files.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`episodes get` is not built yet")
}
