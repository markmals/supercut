use crate::commands::Context;

/// Draft a cut plan from labeled segments: cut the show's always-cut segments, add segment chapters.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`draft` is not built yet")
}
