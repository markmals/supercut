use crate::commands::Context;

/// List configured shows with episode, download, transcript and label counts.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`shows list` is not built yet")
}
