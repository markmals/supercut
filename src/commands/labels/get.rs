use crate::commands::Context;

/// Print an episode's labeled segments.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`labels get` is not built yet")
}
