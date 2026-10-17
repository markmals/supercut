use crate::commands::Context;

/// Refresh a show's episode index from its feeds.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`sync` is not built yet")
}
