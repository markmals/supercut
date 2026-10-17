use serde::Serialize;

use crate::commands::{Context, Selection};
use crate::feed;
use crate::output::print_json;

/// Download episodes' audio into the library.
///
/// Episodes whose audio is already there are left alone. The index is saved after every
/// episode, so an interrupted run keeps what it fetched.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as it appears in the config file.
    show: String,
    #[command(flatten)]
    selection: Selection,
}

#[derive(Serialize)]
struct Outcome {
    id: String,
    /// File name inside the show's `audio/` folder.
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes: Option<u64>,
    status: Status,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Downloaded,
    Present,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let mut index = library.load_synced_index()?;
    let ids: Vec<String> =
        args.selection.resolve(&index)?.iter().map(|episode| episode.id.clone()).collect();

    let mut outcomes = Vec::with_capacity(ids.len());
    for id in ids {
        let episode = index.get(&id)?;
        if let Some(file) = feed::existing_audio(&library, episode) {
            outcomes.push(Outcome { id, file: file.to_owned(), bytes: None, status: Status::Present });
            continue;
        }
        let download = feed::download(&library, episode)?;
        index.get_mut(&id)?.audio_file = Some(download.file.clone());
        library.save_index(&index)?;
        outcomes.push(Outcome {
            id,
            file: download.file,
            bytes: Some(download.bytes),
            status: Status::Downloaded,
        });
    }
    print_json(&outcomes)
}
