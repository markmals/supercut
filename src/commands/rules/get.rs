use crate::commands::Context;

/// Print a show's editorial rules and the path of the rules file.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`rules get` is not built yet")
}
