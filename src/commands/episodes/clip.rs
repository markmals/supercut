use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::analysis::Envelope;
use crate::commands::Context;
use crate::error::{CliError, ErrorCode};
use crate::library::partial_path;
use crate::timecode::Time;

/// Cut a stretch of an episode out into its own audio file, e.g. a show's interstitial or theme recording.
///
/// Writes `[--from, --to)` of the episode, untouched apart from the format, which follows the
/// output's extension (`.flac` keeps it lossless). Reports the level at both edges and warns when
/// one lands on sound, naming where the sound starts or dies away so the clip can be redone to
/// hold all of it: find the stretch first with `episodes inspect`. Refuses to overwrite a file
/// without `--force`.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug.
    show: String,
    /// Episode id.
    id: String,
    /// Where the clip starts in the episode (`h:mm:ss.sss`, `m:ss` or seconds).
    #[arg(long)]
    from: Time,
    /// Where it ends.
    #[arg(long)]
    to: Time,
    /// The file to write; its extension picks the format (`.flac`, `.wav`, `.mp3`).
    #[arg(long)]
    out: PathBuf,
    /// Replace an existing file at `--out`.
    #[arg(long)]
    force: bool,
    /// Report the clip and its edges without writing it.
    #[arg(long)]
    dry_run: bool,
}

/// Below this the music has died away; above it an edge cuts something off.
const EDGE_QUIET_DB: f32 = -60.0;
/// Audio either side of an edge measured for its level.
const EDGE_SECONDS: f64 = 0.02;
/// How far past an edge to look for where the sound starts or dies away.
const SEARCH_SECONDS: f64 = 10.0;
/// Quiet this long is the sound stopping, not a beat inside it.
const SILENCE_SECONDS: f64 = 0.2;

#[derive(Serialize)]
struct Receipt {
    episode: String,
    from: Time,
    to: Time,
    seconds: f64,
    output: PathBuf,
    /// Level just inside each edge, in dBFS.
    start_db: f32,
    end_db: f32,
    warnings: Vec<String>,
    dry_run: bool,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (from, to) = (args.from.seconds(), args.to.seconds());
    if from < 0.0 || to <= from {
        return Err(CliError::invalid("`--to` must come after `--from`, and both inside the episode").into());
    }
    let (_, show) = context.show(&args.show)?;
    let index = show.load_synced_index()?;
    let episode = index.get(&args.id)?;
    let envelope = crate::analysis::episode_envelope(&show, episode)?;
    if to > envelope.duration() {
        return Err(CliError::invalid(format!("the episode ends at {}", Time(envelope.duration()))).into());
    }
    if args.out.exists() && !args.force {
        let conflict = CliError::conflict(format!("{} already exists", args.out.display())).hint("pass --force to replace it");
        return Err(conflict.into());
    }
    if !args.dry_run {
        extract(&show.audio_path(episode)?, from, to, &args.out)?;
    }
    crate::output::print_json(&Receipt {
        episode: episode.id.clone(),
        from: args.from,
        to: args.to,
        seconds: ((to - from) * 1000.0).round() / 1000.0,
        output: args.out,
        start_db: envelope.mean_db(from, from + EDGE_SECONDS),
        end_db: envelope.mean_db(to - EDGE_SECONDS, to),
        warnings: edge_warnings(&envelope, from, to),
        dry_run: args.dry_run,
    })
}

/// Warns about an edge that cuts sound off, unless the edge is where that sound starts or dies
/// away, give or take a frame.
fn edge_warnings(envelope: &Envelope, from: f64, to: f64) -> Vec<String> {
    let mut warnings = Vec::new();
    if envelope.mean_db(from, from + EDGE_SECONDS) > EDGE_QUIET_DB {
        let quiet = envelope.quiet_spans((from - SEARCH_SECONDS).max(0.0), from + EDGE_SECONDS, EDGE_QUIET_DB, SILENCE_SECONDS).pop();
        match quiet {
            Some((_, starts)) if (starts - from).abs() <= EDGE_SECONDS => {}
            Some((_, starts)) => warnings.push(format!("the clip starts on sound, which begins at {}", Time(starts))),
            None => warnings.push(format!("the clip starts on sound that began more than {SEARCH_SECONDS} s earlier")),
        }
    }
    if envelope.mean_db(to - EDGE_SECONDS, to) > EDGE_QUIET_DB {
        let quiet = envelope.quiet_spans(to - EDGE_SECONDS, to + SEARCH_SECONDS, EDGE_QUIET_DB, SILENCE_SECONDS).into_iter().next();
        match quiet {
            Some((ends, _)) if (ends - to).abs() <= EDGE_SECONDS => {}
            Some((ends, _)) => warnings.push(format!("the clip ends on sound, which dies away at {}", Time(ends))),
            None => warnings.push(format!("the clip ends on sound that goes on for more than {SEARCH_SECONDS} s")),
        }
    }
    warnings
}

/// Writes `[from, to)` of `audio` to `out` through a temporary file beside it.
fn extract(audio: &Path, from: f64, to: f64, out: &Path) -> anyhow::Result<()> {
    if let Some(folder) = out.parent().filter(|folder| !folder.as_os_str().is_empty()) {
        std::fs::create_dir_all(folder)?;
    }
    let partial = partial_path(out);
    let format = out.extension().and_then(|extension| extension.to_str()).unwrap_or("flac").to_lowercase();
    let output = std::process::Command::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-y", "-ss", &format!("{from:.6}"), "-t", &format!("{:.6}", to - from), "-i"])
        .arg(audio)
        .args(["-map", "0:a:0", "-map_metadata", "-1", "-f", &format])
        .arg(&partial)
        .output()?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&partial);
        let message = format!("ffmpeg could not write the clip: {}", String::from_utf8_lossy(&output.stderr).trim());
        return Err(CliError::new(ErrorCode::ExternalTool, message).into());
    }
    std::fs::rename(&partial, out)?;
    Ok(())
}
