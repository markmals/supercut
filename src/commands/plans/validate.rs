use crate::commands::Context;

/// Check a cut plan against the library and lint every cut edge.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`plans validate` is not built yet")
}
