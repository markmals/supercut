use std::path::Path;
use std::time::Instant;

use serde::Serialize;

use crate::commands::{Context, Selection};
use crate::library::{Episode, ShowLibrary, read_json, write_json_atomically};
use crate::transcript::Transcript;
use crate::{media, output, transcribe};

/// Transcribe episodes with word timings, one at a time.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as `supercut config` lists it.
    pub show: String,
    #[command(flatten)]
    pub selection: Selection,
    /// Transcribe again even where a transcript already exists.
    #[arg(long)]
    pub force: bool,
}

#[derive(Serialize)]
struct Summary {
    id: String,
    /// Length of the audio that was transcribed.
    seconds: f64,
    words: usize,
    unreliable_spans: usize,
    /// True when a transcript was already on disk and `--force` was not given.
    skipped: bool,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let index = library.load_synced_index()?;
    let episodes = args.selection.resolve(&index)?;
    let total = episodes.len();
    let mut summaries = Vec::with_capacity(total);
    for (position, episode) in episodes.into_iter().enumerate() {
        let label = format!("[{}/{total}] {}", position + 1, episode.id);
        summaries.push(one(context, &library, episode, args.force, &label)?);
    }
    output::print_json(&summaries)
}

fn one(
    context: &Context,
    library: &ShowLibrary,
    episode: &Episode,
    force: bool,
    label: &str,
) -> anyhow::Result<Summary> {
    let audio = library.audio_path(episode)?;
    let seconds = media::probe(&audio)?.duration;
    let path = library.transcript_path(&episode.id);
    if path.exists() && !force {
        let transcript: Transcript = read_json(&path)?;
        eprintln!("{label}: transcript already written, skipping");
        return Ok(summary(episode, seconds, &transcript, true));
    }

    eprintln!("{label}: transcribing {:.1} min of audio", seconds / 60.0);
    let started = Instant::now();
    let transcript = transcribe::transcribe_file(&context.config.transcription, &audio)?;
    let elapsed = started.elapsed().as_secs_f64();
    write_json_atomically(&path, &transcript)?;
    report(label, seconds, elapsed, &transcript, &path);
    Ok(summary(episode, seconds, &transcript, false))
}

fn report(label: &str, seconds: f64, elapsed: f64, transcript: &Transcript, path: &Path) {
    let holes = match transcript.unreliable.len() {
        0 => String::new(),
        count => format!(", {count} unreliable span(s)"),
    };
    eprintln!(
        "{label}: done in {:.1} min ({:.1}x realtime, {} words{holes}) -> {}",
        elapsed / 60.0,
        seconds / elapsed,
        transcript.words().count(),
        path.display()
    );
}

fn summary(episode: &Episode, seconds: f64, transcript: &Transcript, skipped: bool) -> Summary {
    Summary {
        id: episode.id.clone(),
        seconds,
        words: transcript.words().count(),
        unreliable_spans: transcript.unreliable.len(),
        skipped,
    }
}
