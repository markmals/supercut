pub mod search;

use clap::Subcommand;

use crate::commands::Context;

/// Discovering supercut's commands.
#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    Search(search::Args),
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    match args.operation {
        Operation::Search(args) => search::run(context, args),
    }
}
