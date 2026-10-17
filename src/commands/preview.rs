use crate::commands::Context;

/// Render the transitions of a plan into one short file for listening.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`preview` is not built yet")
}
