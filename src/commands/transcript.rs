use anyhow::ensure;

use crate::commands::Context;
use crate::library::read_json;
use crate::timecode::Time;
use crate::transcript::Transcript;
use crate::{output, transcribe};

/// Print an episode's transcript with timestamps.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as `supercut config` lists it.
    pub show: String,
    /// Episode id, as `supercut episodes <show>` lists it.
    pub id: String,
    /// Start of the excerpt, as `h:mm:ss.sss`, `m:ss` or seconds.
    #[arg(long)]
    pub from: Option<Time>,
    /// End of the excerpt.
    #[arg(long)]
    pub to: Option<Time>,
    /// Print the selected part of the transcript as JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let path = library.transcript_path(&args.id);
    ensure!(
        path.exists(),
        "no transcript for `{1}`; run `supercut transcribe {0} {1}`",
        library.slug,
        args.id
    );
    let transcript: Transcript = read_json(&path)?;

    let from = args.from.unwrap_or_default().seconds();
    let to = args.to.map_or(f64::INFINITY, Time::seconds);
    ensure!(to > from, "--to {to} is not after --from {from}");

    if args.json {
        return output::print_json(&transcribe::slice(&transcript, from, to));
    }
    println!("{}", transcribe::render(&transcript, from, to));
    Ok(())
}
