use crate::commands::Context;

/// Summarize a cut plan for a human reviewer, with transcript context around each cut.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`plans review` is not built yet")
}
