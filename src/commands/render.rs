use crate::commands::Context;

/// Render a plan into the finished supercut MP3.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`render` is not built yet")
}
