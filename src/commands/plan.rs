use crate::commands::Context;

/// Check or review a cut plan.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`plan` is not built yet")
}
