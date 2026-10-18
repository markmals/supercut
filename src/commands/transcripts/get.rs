use serde::Serialize;

use crate::commands::Context;
use crate::error::CliError;
use crate::library::read_json;
use crate::output::{self, Format};
use crate::timecode::Time;
use crate::transcribe::{self, Utterance};
use crate::transcript::{Transcript, UnreliableSpan};

/// Print an episode's transcript, optionally only a time range.
///
/// Reads `transcripts/<id>.json`, so the episode must have been transcribed first
/// (`supercut transcripts create`). Words are gathered into utterances: one per pause in the
/// speech. `--output text` prints `[m:ss.s] what was said` lines, with stretches the engine
/// garbled marked as `[unreliable transcript until m:ss.s]`.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as `supercut shows list` prints it.
    pub show: String,
    /// Episode id, as `supercut episodes list <show>` prints it.
    pub id: String,
    /// First moment to print, as `h:mm:ss.sss`, `m:ss` or seconds. Defaults to the start.
    #[arg(long)]
    pub from: Option<Time>,
    /// Last moment to print. Defaults to the end of the episode.
    #[arg(long)]
    pub to: Option<Time>,
    /// `json` for utterances with times, `text` for timestamped lines to read.
    #[arg(long, value_enum, default_value_t = Format::Json)]
    pub output: Format,
}

#[derive(Serialize)]
struct Excerpt<'a> {
    episode: &'a str,
    from: f64,
    to: f64,
    utterances: Vec<Utterance>,
    /// Stretches the engine garbled; they have no words.
    unreliable: Vec<&'a UnreliableSpan>,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let path = library.transcript_path(&args.id);
    if !path.exists() {
        return Err(CliError::precondition(format!("episode `{}` is not transcribed", args.id))
            .hint(format!("supercut transcripts create {} {}", library.slug, args.id))
            .detail("path", path.display().to_string())
            .into());
    }
    let transcript: Transcript = read_json(&path)?;

    // A loop at the end of an episode leaves an unreliable span and no words behind it.
    let spoken_until = transcript.words().map(|word| word.end).fold(0.0, f64::max);
    let covered = transcript.unreliable.iter().map(|span| span.end).fold(spoken_until, f64::max);
    let from = args.from.unwrap_or_default().seconds();
    let to = args.to.map_or(covered, Time::seconds);
    if to <= from {
        return Err(CliError::invalid(format!("--to {to} is not after --from {from}")).into());
    }

    if args.output == Format::Text {
        println!("{}", transcribe::render(&transcript, from, to));
        return Ok(());
    }
    output::print_json(&Excerpt {
        episode: &args.id,
        from,
        to,
        utterances: transcribe::utterances(&transcript, from, to),
        unreliable: transcript.unreliable_between(from, to).collect(),
    })
}
