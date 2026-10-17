use crate::commands::Context;

/// Print a show's editorial rules file (and its path).
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`rules` is not built yet")
}
