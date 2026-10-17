use crate::commands::Context;

/// Render every transition of a plan into one short MP3 for listening.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`plans preview` is not built yet")
}
