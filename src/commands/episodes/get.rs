use crate::commands::Context;
use crate::feed;
use crate::output::print_json;

/// Show one episode's metadata and local files.
///
/// Everything the feed said about the episode, plus where its audio, transcript, labels and
/// analysis live and whether each exists. Feed URLs are printed without their query string.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as it appears in the config file.
    show: String,
    /// Episode id, as listed by `supercut episodes list <show>`.
    id: String,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let index = library.load_synced_index()?;
    print_json(&feed::detail(&library, index.get(&args.id)?))
}
