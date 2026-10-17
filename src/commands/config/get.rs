use crate::commands::Context;

/// Show the effective configuration and where supercut keeps its files, secrets redacted.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`config get` is not built yet")
}
