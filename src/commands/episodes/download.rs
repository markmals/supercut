use crate::commands::Context;

/// Download episodes' audio into the library.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`episodes download` is not built yet")
}
