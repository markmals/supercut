use crate::commands::Context;

/// Transcribe episodes with word timings.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`transcribe` is not built yet")
}
