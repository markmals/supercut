use crate::commands::Context;

/// List a show's episodes from the library index.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`episodes` is not built yet")
}
