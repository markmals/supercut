use std::time::Instant;

use serde::Serialize;

use crate::commands::{Context, Selection};
use crate::library::{Episode, ShowLibrary, read_json, write_json_atomically};
use crate::transcript::Transcript;
use crate::{media, output, transcribe};

/// Transcribe episodes with word timings using the configured local Whisper engine.
///
/// Runs one episode at a time, since a single run already saturates the GPU, and writes
/// `transcripts/<id>.json` in the show's library folder. Episodes must be downloaded first
/// (`supercut episodes download`). Stretches where the engine looped instead of transcribing are
/// removed and listed as unreliable spans. Episodes that already have a transcript are skipped
/// unless `--force` is given. Progress, elapsed time and the realtime factor go to stderr.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as `supercut shows list` prints it.
    pub show: String,
    #[command(flatten)]
    pub selection: Selection,
    /// Transcribe again where a transcript already exists, replacing it.
    #[arg(long)]
    pub force: bool,
    /// Report what would be transcribed, without running the engine or writing anything.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    /// Length of the episode's audio.
    seconds: f64,
    words: usize,
    unreliable_spans: usize,
    status: Status,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Status {
    /// Transcribed now, and written to the library.
    Transcribed,
    /// A transcript was already there and `--force` was not given.
    Skipped,
    /// `--dry-run`: this episode is what a real run would transcribe.
    WouldTranscribe,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let index = library.load_synced_index()?;
    let episodes = args.selection.resolve(&index)?;
    let total = episodes.len();
    let mut receipts = Vec::with_capacity(total);
    for (position, episode) in episodes.into_iter().enumerate() {
        let label = format!("[{}/{total}] {}", position + 1, episode.id);
        receipts.push(one(context, &library, episode, &args, &label)?);
    }
    output::print_json(&receipts)
}

fn one(
    context: &Context,
    library: &ShowLibrary,
    episode: &Episode,
    args: &Args,
    label: &str,
) -> anyhow::Result<Receipt> {
    let audio = library.audio_path(episode)?;
    let seconds = media::probe(&audio)?.duration;
    let path = library.transcript_path(&episode.id);
    if path.exists() && !args.force {
        let transcript: Transcript = read_json(&path)?;
        eprintln!("{label}: already transcribed, skipping");
        return Ok(receipt(episode, seconds, &transcript, Status::Skipped));
    }
    if args.dry_run {
        eprintln!("{label}: would transcribe {:.1} min to {}", seconds / 60.0, path.display());
        return Ok(Receipt {
            id: episode.id.clone(),
            seconds,
            words: 0,
            unreliable_spans: 0,
            status: Status::WouldTranscribe,
        });
    }

    eprintln!("{label}: transcribing {:.1} min of audio", seconds / 60.0);
    let started = Instant::now();
    let transcript = transcribe::transcribe_file(&context.config.transcription, &audio)?;
    let elapsed = started.elapsed().as_secs_f64();
    write_json_atomically(&path, &transcript)?;
    eprintln!(
        "{label}: done in {:.1} min ({:.1}x realtime, {} words, {} unreliable span(s)) -> {}",
        elapsed / 60.0,
        seconds / elapsed,
        transcript.words().count(),
        transcript.unreliable.len(),
        path.display()
    );
    Ok(receipt(episode, seconds, &transcript, Status::Transcribed))
}

fn receipt(episode: &Episode, seconds: f64, transcript: &Transcript, status: Status) -> Receipt {
    Receipt {
        id: episode.id.clone(),
        seconds,
        words: transcript.words().count(),
        unreliable_spans: transcript.unreliable.len(),
        status,
    }
}
