pub mod get;
pub mod init;
pub mod schema;
pub mod validate;

use clap::Subcommand;

use crate::commands::Context;

/// The config file (`supercut.config.jsonc`): where supercut keeps things, feeds, Jev and loudness settings.
#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    Get(get::Args),
    Init(init::Args),
    Schema(schema::Args),
    Validate(validate::Args),
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    match args.operation {
        Operation::Get(args) => get::run(context, args),
        Operation::Init(args) => init::run(context, args),
        Operation::Schema(args) => schema::run(context, args),
        Operation::Validate(args) => validate::run(context, args),
    }
}
