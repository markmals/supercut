pub mod list;

use clap::Subcommand;

use crate::commands::Context;

/// Shows configured for supercut.
#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    List(list::Args),
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    match args.operation {
        Operation::List(args) => list::run(context, args),
    }
}
