use serde::Serialize;

use crate::commands::{Context, Selection};
use crate::feed;
use crate::output::print_json;

/// Download episodes' audio into the library.
///
/// Fetches every selected episode that has no audio on disk into
/// `<library>/<show>/audio/<id>.<ext>`, saving the index after each one so an interrupted run
/// keeps what it fetched. Needs `episodes sync` first. Progress goes to stderr.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as it appears in the config file.
    show: String,
    #[command(flatten)]
    selection: Selection,
    /// Report what would be fetched, download nothing.
    #[arg(long)]
    dry_run: bool,
}

/// What became of one episode. In a dry run `file` is the name the URL suggests; the served
/// content type can still choose another extension.
#[derive(Serialize)]
struct Outcome {
    id: String,
    title: String,
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes: Option<u64>,
    status: Status,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum Status {
    Downloaded,
    /// Already in the library; nothing to do.
    Present,
    WouldDownload,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let mut index = library.load_synced_index()?;
    let ids: Vec<String> =
        args.selection.resolve(&index)?.iter().map(|episode| episode.id.clone()).collect();

    let mut outcomes = Vec::with_capacity(ids.len());
    for id in ids {
        let episode = index.get(&id)?;
        let title = episode.title.clone();
        if let Some(file) = feed::existing_audio(&library, episode) {
            let file = file.to_owned();
            outcomes.push(Outcome { id, title, file, bytes: None, status: Status::Present });
            continue;
        }
        if args.dry_run {
            let file = feed::planned_file(episode);
            outcomes.push(Outcome { id, title, file, bytes: None, status: Status::WouldDownload });
            continue;
        }
        let download = feed::download(&library, episode)?;
        index.get_mut(&id)?.audio_file = Some(download.file.clone());
        library.save_index(&index)?;
        outcomes.push(Outcome {
            id,
            title,
            file: download.file,
            bytes: Some(download.bytes),
            status: Status::Downloaded,
        });
    }
    print_json(&outcomes)
}
