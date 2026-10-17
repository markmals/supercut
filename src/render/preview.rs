mod reel;

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Serialize;

use crate::commands::Context;
use crate::error::CliError;
use crate::library::partial_path;
use crate::plan::{Plan, resolve, timeline};
use crate::render::piece::Piece;
use crate::render::{encode, graph};
use crate::timecode::Time;

use reel::Reel;

/// Less context than this is not long enough to judge a splice by.
const MINIMUM_CONTEXT_SECONDS: f64 = 1.0;

/// Every splice a plan makes, back to back, so the reviewer can hear them in a few minutes.
#[derive(Debug, Serialize)]
pub struct Preview {
    pub preview: PathBuf,
    pub duration: Time,
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Serialize)]
pub struct Transition {
    pub index: usize,
    /// Where the splice itself falls in the preview file.
    pub preview_time: Time,
    pub kind: Kind,
    pub episode: String,
    /// At an episode boundary, the episode that follows.
    pub next: Option<String>,
    pub cut_start: Option<Time>,
    pub cut_end: Option<Time>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// How the supercut starts.
    Opening,
    /// A cut inside an episode, with its audio kept on both sides.
    Cut,
    /// One episode's last audio, the interstitial if one plays, and the next episode's first.
    EpisodeBoundary,
    /// How the supercut ends.
    Ending,
}

pub struct Request<'a> {
    pub plan: &'a Plan,
    /// Seconds of audio played either side of a splice.
    pub context: f64,
    pub out: Option<PathBuf>,
}

pub fn preview(context: &Context, request: Request) -> anyhow::Result<Preview> {
    if request.context < MINIMUM_CONTEXT_SECONDS {
        let short = CliError::invalid(format!(
            "--context of {} s is too short to judge a splice by; give at least {MINIMUM_CONTEXT_SECONDS} s",
            request.context
        ));
        return Err(short.into());
    }
    let resolved = resolve::resolve(context, request.plan)?;
    crate::plan::check::require_valid_cuts(&resolved)?;
    let timeline = timeline::build(&resolved.input);
    let reel = Reel::of(&resolved.input, &timeline, request.context);

    let output = match request.out {
        Some(path) => path,
        None => default_path(&resolved.library.previews_dir(), &request.plan.output),
    };
    let parent = output.parent().expect("a preview file has a folder");
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    let duration = reel.pieces.last().map_or(0.0, Piece::output_end);
    let limit_db = context.config.loudness.true_peak_db;
    let partial = partial_path(&output);
    let args = graph::encode_args(&reel.pieces, &resolved.format, limit_db, &partial);
    encode::run(&args, duration, "previewing")?;
    std::fs::rename(&partial, &output).with_context(|| format!("moving {} into place", output.display()))?;
    Ok(Preview { preview: output, duration: Time(duration), transitions: reel.transitions })
}

fn default_path(previews_dir: &Path, plan_output: &str) -> PathBuf {
    let stem = Path::new(plan_output).file_stem().unwrap_or_default().to_string_lossy();
    previews_dir.join(format!("{stem}-transitions.mp3"))
}
