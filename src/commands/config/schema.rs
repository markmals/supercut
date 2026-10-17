use crate::commands::Context;
use crate::output::print_json;

/// Print the JSON Schema of the config file.
///
/// Describes every setting `supercut.config.jsonc` takes, with its type, default and meaning, so
/// an editor or agent can check a config before supercut reads it. `config init` writes the same
/// schema beside a new config as `supercut.config.schema.json`; print it again after upgrading
/// supercut to refresh that copy.
#[derive(clap::Args)]
pub struct Args {}

pub fn run(_context: &Context, _args: Args) -> anyhow::Result<()> {
    print_json(&crate::config::schema())
}
