use crate::commands::Context;

/// Render a plan into the finished supercut MP3 with loudness matching, tags and chapters.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`plans render` is not built yet")
}
