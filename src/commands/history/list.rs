use serde::Serialize;

use crate::commands::Context;
use crate::history;
use crate::timecode::Time;

/// List the hand-made supercuts recovered for a show.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug.
    show: String,
}

#[derive(Serialize)]
struct Entry {
    name: String,
    export: String,
    duration: Time,
    episodes: Vec<String>,
    cuts: usize,
    interstitials: usize,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, show) = context.show(&args.show)?;
    let mut names: Vec<String> = match std::fs::read_dir(show.history_dir()) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter_map(|file| file.strip_suffix(".plan.json").map(str::to_owned))
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    names.sort();
    let mut entries = Vec::with_capacity(names.len());
    for name in names {
        let (recovery, plan) = history::load(&show, &name)?;
        entries.push(Entry {
            name,
            export: recovery.export,
            duration: Time(recovery.duration),
            episodes: plan.episodes.iter().map(|episode| episode.id.clone()).collect(),
            cuts: plan.episodes.iter().map(|episode| episode.cuts.len()).sum(),
            interstitials: recovery.interstitials.len(),
        });
    }
    crate::output::print_json(&entries)
}
