use std::path::PathBuf;

use crate::commands::Context;
use crate::feed;
use crate::output::print_json;

/// Adopt audio files you already have into the library instead of downloading them again.
///
/// Every audio file under the folder is matched to an episode that has no audio yet, by the
/// title in the file's own tags; a file whose length disagrees with the feed by more than 2% is
/// refused. Matches are hard-linked into `<library>/<show>/audio/`, so they cost no extra disk;
/// if the folder is on another volume the file is copied instead. Needs `episodes sync` first.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as it appears in the config file.
    show: String,
    /// Folder to search, recursively.
    dir: PathBuf,
    /// Report the matches, link nothing.
    #[arg(long)]
    dry_run: bool,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let mut index = library.load_synced_index()?;
    let report = feed::adopt(&library, &mut index, &args.dir, args.dry_run)?;
    if !args.dry_run && !report.adopted.is_empty() {
        library.save_index(&index)?;
    }
    print_json(&report)
}
