use crate::commands::Context;

/// Transcribe episodes with word timings using the configured local Whisper engine.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`transcripts create` is not built yet")
}
