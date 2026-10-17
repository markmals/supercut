use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Serialize;

use crate::commands::Context;
use crate::error::CliError;
use crate::library::partial_path;
use crate::plan::timeline::{Input, InputEpisode};
use crate::plan::{Cut, EDGE_FADE_SECONDS, Plan, resolve};
use crate::render::piece::{Piece, Source};
use crate::render::{encode, graph};
use crate::timecode::Time;

/// Audio shorter than this is not worth a transition of its own.
const MINIMUM_PIECE_SECONDS: f64 = 0.05;

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
    /// Where the transition starts in the preview file.
    pub preview_time: Time,
    pub episode: String,
    pub kind: Kind,
    pub cut_start: Option<Time>,
    pub cut_end: Option<Time>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Cut,
    EpisodeBoundary,
}

pub struct Request<'a> {
    pub plan: &'a Plan,
    /// Seconds of audio played either side of a splice.
    pub context: f64,
    pub out: Option<PathBuf>,
}

pub fn preview(context: &Context, request: Request) -> anyhow::Result<Preview> {
    let resolved = resolve::resolve(context, request.plan)?;
    let mut reel = Reel::default();
    reel.fill(&resolved.input, request.context);
    if reel.pieces.is_empty() {
        let empty = CliError::invalid("the plan has no cuts and only one episode, so there is nothing to preview");
        return Err(empty.into());
    }

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

/// The reel being assembled: pieces of audio and what each one is there to show.
#[derive(Default)]
struct Reel {
    pieces: Vec<Piece>,
    transitions: Vec<Transition>,
}

impl Reel {
    fn fill(&mut self, input: &Input, context: f64) {
        for (index, episode) in input.episodes.iter().enumerate() {
            let gain = input.target_lufs - episode.integrated_lufs;
            for cut in &episode.cuts {
                self.cut_transition(episode, cut, gain, context);
            }
            let Some(next) = input.episodes.get(index + 1) else { continue };
            self.boundary_transition(input, episode, next, context);
        }
    }

    /// What the listener hears across a cut: the run-up, the splice, and what follows.
    fn cut_transition(&mut self, episode: &InputEpisode, cut: &Cut, gain: f64, context: f64) {
        let start = cut.start.seconds();
        let end = cut.end.map_or(episode.duration, Time::seconds);
        self.start_transition();
        let path = &episode.audio;
        self.push(Source::file(path, (start - context).max(0.0), start), gain, EDGE_FADE_SECONDS, fade(cut.fade_out));
        self.push(
            Source::file(path, end, (end + context).min(episode.duration)),
            gain,
            fade(cut.fade_in),
            EDGE_FADE_SECONDS,
        );
        self.transitions.push(Transition {
            index: self.transitions.len() + 1,
            preview_time: Time(self.transition_start()),
            episode: episode.id.clone(),
            kind: Kind::Cut,
            cut_start: Some(Time(start)),
            cut_end: Some(Time(end)),
            reason: Some(cut.reason.clone()),
        });
    }

    /// The end of one episode, the interstitial, and the start of the next.
    fn boundary_transition(&mut self, input: &Input, episode: &InputEpisode, next: &InputEpisode, context: f64) {
        self.start_transition();
        let gain = input.target_lufs - episode.integrated_lufs;
        let tail = (episode.duration - context).max(0.0);
        self.push(Source::file(&episode.audio, tail, episode.duration), gain, EDGE_FADE_SECONDS, 0.0);
        if let Some(clip) = &input.interstitial {
            let clip_gain = input.target_lufs - clip.integrated_lufs;
            self.push(Source::file(&clip.audio, 0.0, clip.duration), clip_gain, EDGE_FADE_SECONDS, EDGE_FADE_SECONDS);
        }
        let head = Source::file(&next.audio, 0.0, context.min(next.duration));
        self.push(head, input.target_lufs - next.integrated_lufs, 0.0, EDGE_FADE_SECONDS);
        self.transitions.push(Transition {
            index: self.transitions.len() + 1,
            preview_time: Time(self.transition_start()),
            episode: episode.id.clone(),
            kind: Kind::EpisodeBoundary,
            cut_start: None,
            cut_end: None,
            reason: None,
        });
    }

    /// Transitions are separated by a marker so they never blur into each other.
    fn start_transition(&mut self) {
        if self.pieces.is_empty() {
            return;
        }
        self.push(Source::Beep, 0.0, 0.0, 0.0);
    }

    fn push(&mut self, source: Source, gain_db: f64, fade_in: f64, fade_out: f64) {
        if source.seconds() < MINIMUM_PIECE_SECONDS {
            return;
        }
        let output_start = self.pieces.last().map_or(0.0, Piece::output_end);
        self.pieces.push(Piece::new(source, gain_db, output_start).faded(fade_in, fade_out));
    }

    /// Where the pieces pushed since the last marker begin.
    fn transition_start(&self) -> f64 {
        let marker = self.pieces.iter().rposition(|piece| matches!(piece.source, Source::Beep));
        let first = marker.map_or(0, |index| index + 1);
        self.pieces.get(first).map_or(0.0, |piece| piece.output_start)
    }
}

fn fade(seconds: Option<f64>) -> f64 {
    seconds.unwrap_or(EDGE_FADE_SECONDS)
}
