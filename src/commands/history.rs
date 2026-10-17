use crate::commands::Context;

/// Recover the cuts of a supercut you made by hand by matching it against the source episodes.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`history` is not built yet")
}
