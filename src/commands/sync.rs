use anyhow::ensure;
use serde::Serialize;

use crate::commands::Context;
use crate::feed;
use crate::output::print_json;

/// Refresh a show's episode index from its feeds.
///
/// Episodes already in the index keep their id and their downloaded audio; only what the feed
/// says about them is refreshed.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as it appears in the config file.
    show: String,
}

#[derive(Serialize)]
struct Summary<'a> {
    show: &'a str,
    /// Episodes in the index afterwards.
    episodes: usize,
    added: Vec<String>,
    updated: usize,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (show, library) = context.show(&args.show)?;
    ensure!(!show.feeds.is_empty(), "show `{}` has no feeds in the config file", args.show);

    let mut items = Vec::new();
    for url in &show.feeds {
        let fetched = feed::fetch(url)?;
        eprintln!("{}: {} episodes from {}", args.show, fetched.len(), feed::redacted(url));
        items.extend(fetched);
    }

    let mut index = library.load_index()?;
    let report = feed::merge(&mut index, &items);
    library.save_index(&index)?;
    print_json(&Summary {
        show: &args.show,
        episodes: index.episodes.len(),
        added: report.added,
        updated: report.updated,
    })
}
