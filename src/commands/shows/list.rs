use serde::Serialize;

use crate::commands::Context;
use crate::feed;
use crate::library::ShowLibrary;
use crate::output::print_json;

/// List configured shows with episode, download, transcript and label counts.
///
/// The counts come from the library, so a show that has never been synced reads zero. Use the
/// `show` slug with every other command.
#[derive(clap::Args)]
pub struct Args {}

#[derive(Serialize)]
struct Row<'a> {
    show: &'a str,
    title: &'a str,
    #[serde(flatten)]
    counts: feed::Counts,
}

pub fn run(context: &Context, _args: Args) -> anyhow::Result<()> {
    let mut rows = Vec::with_capacity(context.config.shows.len());
    for (slug, show) in &context.config.shows {
        let library = ShowLibrary::new(&context.library_dir, slug);
        let index = library.load_index()?;
        rows.push(Row { show: slug, title: &show.title, counts: feed::counts(&library, &index) });
    }
    print_json(&rows)
}
