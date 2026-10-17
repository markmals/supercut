pub mod compare;
pub mod draft;
pub mod validate;
pub mod review;
pub mod preview;
pub mod render;

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
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    match args.operation {
        Operation::Draft(args) => draft::run(context, args),
        Operation::Validate(args) => validate::run(context, args),
        Operation::Review(args) => review::run(context, args),
        Operation::Compare(args) => compare::run(context, args),
        Operation::Preview(args) => preview::run(context, args),
        Operation::Render(args) => render::run(context, args),
    }
}
