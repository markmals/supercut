use crate::commands::Context;

/// List a show's episodes with what exists locally for each.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`episodes list` is not built yet")
}
