pub mod sync;
pub mod list;
pub mod get;
pub mod download;
pub mod adopt;
pub mod analyze;
pub mod inspect;
pub mod clip;

use clap::Subcommand;

use crate::commands::Context;

/// A show's episodes: the feed index, downloaded audio and its analysis.
#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    Sync(sync::Args),
    List(list::Args),
    Get(get::Args),
    Download(download::Args),
    Adopt(adopt::Args),
    Analyze(analyze::Args),
    Inspect(inspect::Args),
    Clip(clip::Args),
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    match args.operation {
        Operation::Sync(args) => sync::run(context, args),
        Operation::List(args) => list::run(context, args),
        Operation::Get(args) => get::run(context, args),
        Operation::Download(args) => download::run(context, args),
        Operation::Adopt(args) => adopt::run(context, args),
        Operation::Analyze(args) => analyze::run(context, args),
        Operation::Inspect(args) => inspect::run(context, args),
        Operation::Clip(args) => clip::run(context, args),
    }
}
