pub mod compare;
pub mod draft;
pub mod preview;
pub mod publish;
pub mod render;
pub mod review;
pub mod validate;

use clap::Subcommand;

use crate::commands::Context;

/// Cut plans: which episodes go into a supercut and what is cut from each.
#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    Draft(draft::Args),
    Validate(validate::Args),
    Review(review::Args),
    Compare(compare::Args),
    Preview(preview::Args),
    Render(render::Args),
    Publish(publish::Args),
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    match args.operation {
        Operation::Draft(args) => draft::run(context, args),
        Operation::Validate(args) => validate::run(context, args),
        Operation::Review(args) => review::run(context, args),
        Operation::Compare(args) => compare::run(context, args),
        Operation::Preview(args) => preview::run(context, args),
        Operation::Render(args) => render::run(context, args),
        Operation::Publish(args) => publish::run(context, args),
    }
}
