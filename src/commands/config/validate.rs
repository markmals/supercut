use crate::commands::Context;

/// Check the config file, the paths it names and the external tools supercut needs.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    anyhow::bail!("`config validate` is not built yet")
}
