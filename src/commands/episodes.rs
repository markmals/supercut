use crate::commands::Context;
use crate::feed;
use crate::output::print_json;

/// List a show's episodes from the library index.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as it appears in the config file.
    show: String,
    /// Only episodes the feed files under this season, its bonus episodes included.
    #[arg(long)]
    season: Option<u32>,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let index = library.load_synced_index()?;
    let chosen = index.select(&[], args.season, args.season.is_none())?;
    print_json(&feed::statuses(&library, &chosen))
}
