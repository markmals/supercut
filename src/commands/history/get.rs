use crate::commands::Context;

/// Print a recovered supercut: its pieces, cuts and what was said in each cut.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`history get` is not built yet")
}
