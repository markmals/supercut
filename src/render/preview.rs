use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Serialize;

use crate::commands::Context;
use crate::error::CliError;
use crate::library::partial_path;
use crate::plan::timeline::{Input, InputEpisode, Kept, kept_ranges};
use crate::plan::{Cut, EDGE_FADE_SECONDS, Plan, resolve};
use crate::render::piece::{Piece, Source};
use crate::render::{encode, graph};
use crate::timecode::Time;

/// Less context than this is not long enough to judge a splice by.
const MINIMUM_CONTEXT_SECONDS: f64 = 1.0;

/// Audio shorter than this is not worth a transition of its own.
const MINIMUM_PIECE_SECONDS: f64 = 0.05;
/// Times closer than this are the same position (a cut edge and the range it bounds).
const EPSILON: f64 = 0.001;

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
    if request.context < MINIMUM_CONTEXT_SECONDS {
        let short = CliError::invalid(format!(
            "--context of {} s is too short to judge a splice by; give at least {MINIMUM_CONTEXT_SECONDS} s",
            request.context
        ));
        return Err(short.into());
    }
    let resolved = resolve::resolve(context, request.plan)?;
    crate::plan::check::require_valid_cuts(&resolved)?;
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
            let kept = kept_ranges(&episode.cuts, episode.duration);
            for cut in &episode.cuts {
                self.cut_transition(episode, cut, &kept, gain, context);
            }
            let Some(next) = input.episodes.get(index + 1) else { continue };
            self.boundary_transition(input, episode, next, context);
        }
    }

    /// What the listener hears across a cut: the run-up, the splice, and what follows, each
    /// taken only from audio the render keeps.
    fn cut_transition(&mut self, episode: &InputEpisode, cut: &Cut, kept: &[Kept], gain: f64, context: f64) {
        let start = cut.start.seconds();
        let end = cut.end.map_or(episode.duration, Time::seconds);
        self.start_transition();
        let path = &episode.audio;
        if let Some(before) = kept.iter().find(|range| (range.end - start).abs() < EPSILON) {
            let run_up = Source::file(path, before.start.max(start - context), start);
            self.push(run_up, gain, EDGE_FADE_SECONDS, before.fade_out);
        }
        if let Some(after) = kept.iter().find(|range| (range.start - end).abs() < EPSILON) {
            self.push(Source::file(path, end, after.end.min(end + context)), gain, after.fade_in, EDGE_FADE_SECONDS);
        }
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

    /// The last kept audio of one episode, the interstitial, and the first kept audio of the
    /// next.
    fn boundary_transition(&mut self, input: &Input, episode: &InputEpisode, next: &InputEpisode, context: f64) {
        self.start_transition();
        if let Some(last) = kept_ranges(&episode.cuts, episode.duration).pop() {
            let tail = Source::file(&episode.audio, last.start.max(last.end - context), last.end);
            self.push(tail, input.target_lufs - episode.integrated_lufs, EDGE_FADE_SECONDS, last.fade_out);
        }
        if let Some(clip) = &input.interstitial {
            let clip_gain = input.target_lufs - clip.integrated_lufs;
            self.push(Source::file(&clip.audio, 0.0, clip.duration), clip_gain, EDGE_FADE_SECONDS, EDGE_FADE_SECONDS);
        }
        if let Some(first) = kept_ranges(&next.cuts, next.duration).into_iter().next() {
            let head = Source::file(&next.audio, first.start, first.end.min(first.start + context));
            self.push(head, input.target_lufs - next.integrated_lufs, first.fade_in, EDGE_FADE_SECONDS);
        }
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

#[cfg(test)]
mod tests {
    use super::Reel;
    use crate::plan::Cut;
    use crate::plan::timeline::{Input, InputEpisode};
    use crate::render::piece::Source;
    use crate::timecode::Time;

    fn episode(id: &str, cuts: Vec<Cut>) -> InputEpisode {
        InputEpisode {
            id: id.into(),
            title: String::new(),
            description: String::new(),
            audio: format!("/audio/{id}.mp3").into(),
            duration: 100.0,
            integrated_lufs: -16.0,
            cuts,
            chapters: Vec::new(),
        }
    }

    fn cut(start: f64, end: Option<f64>) -> Cut {
        Cut { start: Time(start), end: end.map(Time), reason: String::new(), fade_out: None, fade_in: None }
    }

    #[test]
    fn the_reel_plays_only_audio_the_render_keeps() {
        // a's outro is cut to the end and b's intro from the top: no transition may play them.
        let input = Input {
            target_lufs: -16.0,
            episodes: vec![episode("a", vec![cut(90.0, None)]), episode("b", vec![cut(0.0, Some(10.0))])],
            interstitial: None,
        };
        let mut reel = Reel::default();
        reel.fill(&input, 6.0);
        let played: Vec<(String, f64, f64)> = reel
            .pieces
            .iter()
            .filter_map(|piece| match &piece.source {
                Source::File { path, start, end } => Some((path.display().to_string(), *start, *end)),
                Source::Beep => None,
            })
            .collect();
        let boundary = played.windows(2).find(|pair| pair[0].0.contains("/a.") && pair[1].0.contains("/b."));
        assert!(boundary.is_some(), "the a-to-b boundary is in the reel: {played:?}");
        for (path, start, end) in &played {
            let removed = if path.contains("/a.") { *end > 90.0 } else { *start < 10.0 };
            assert!(!removed, "the reel plays cut audio {path} {start}-{end}");
        }
    }
}
