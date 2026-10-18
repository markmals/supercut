pub mod get;
pub mod list;
pub mod recover;

use clap::Subcommand;

use crate::commands::Context;

/// Supercuts you made by hand, matched back to their source episodes.
#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    Recover(recover::Args),
    Get(get::Args),
    List(list::Args),
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    match args.operation {
        Operation::Recover(args) => recover::run(context, args),
        Operation::Get(args) => get::run(context, args),
        Operation::List(args) => list::run(context, args),
    }
}
