pub mod get;

use clap::Subcommand;

use crate::commands::Context;

/// A show's editorial rules: plain Markdown an agent reads before planning cuts.
#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    Get(get::Args),
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    match args.operation {
        Operation::Get(args) => get::run(context, args),
    }
}
