pub mod create;
pub mod get;

use clap::Subcommand;

use crate::commands::Context;

/// Segment labels: which recurring segment (intro, guest, ad...) each stretch of an episode is.
#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    Create(create::Args),
    Get(get::Args),
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    match args.operation {
        Operation::Create(args) => create::run(context, args),
        Operation::Get(args) => get::run(context, args),
    }
}
