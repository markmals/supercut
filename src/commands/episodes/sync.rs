use serde::Serialize;

use crate::commands::Context;
use crate::error::CliError;
use crate::feed;
use crate::output::print_json;

/// Refresh a show's episode index from its RSS feeds.
///
/// Reads every feed the show configures, in order, and folds the items into
/// `<library>/<show>/episodes.json`. Episodes already there keep their id and their downloaded
/// audio; only what the feed says about them is refreshed. New episodes are numbered oldest
/// first, so the ids of a feed never shift. Run this before anything else about a show.
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
    /// Ids of the episodes this sync added, oldest first.
    added: Vec<String>,
    /// How many known episodes the feeds changed.
    updated: usize,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (show, library) = context.show(&args.show)?;
    if show.feeds.is_empty() {
        let error =
            CliError::precondition(format!("show `{}` has no feeds in the config file", args.show));
        return Err(error.hint("supercut config get").into());
    }

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
