use crate::commands::Context;

/// Recover the cuts, fades and interstitials of a hand-made supercut by matching it against the source episodes.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`history recover` is not built yet")
}
