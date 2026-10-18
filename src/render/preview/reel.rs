//! The preview reel: the rendered file's own audio around every splice. Windows that overlap
//! play as one, so no moment of the render is heard twice.

use super::{Kind, Transition};
use crate::plan::EDGE_FADE_SECONDS;
use crate::plan::timeline::{Input, InputEpisode, Timeline};
use crate::render::piece::{Piece, Source};
use crate::timecode::Time;

/// Times closer than this are the same position.
const EPSILON: f64 = 0.001;

/// A stretch of the rendered file and the splices it lets the reviewer hear.
struct Window {
    from: f64,
    to: f64,
    splices: Vec<Splice>,
}

/// A place where the render joins audio, in the rendered file's time.
struct Splice {
    at: f64,
    kind: Kind,
    episode: String,
    next: Option<String>,
    /// For a cut: where it starts and ends in the episode, and why.
    cut: Option<(f64, f64, Option<String>)>,
}

impl Window {
    fn around(from: f64, to: f64, splice: Splice) -> Self {
        Self { from: from.max(0.0), to, splices: vec![splice] }
    }
}

#[derive(Default)]
pub struct Reel {
    pub pieces: Vec<Piece>,
    pub transitions: Vec<Transition>,
}

impl Reel {
    /// Every splice in `timeline` with `context` seconds of the render either side.
    pub fn of(input: &Input, timeline: &Timeline, context: f64) -> Self {
        let mut reel = Self::default();
        for window in merged(windows(input, timeline, context)) {
            reel.play(timeline, window);
        }
        reel
    }

    fn play(&mut self, timeline: &Timeline, window: Window) {
        if !self.pieces.is_empty() {
            self.push(Piece::new(Source::Beep, 0.0, 0.0));
        }
        let start = self.duration();
        for piece in &timeline.pieces {
            if let Some(slice) = slice(piece, window.from, window.to) {
                self.push(slice);
            }
        }
        for splice in window.splices {
            let preview_time = Time(start + splice.at - window.from);
            let (cut_start, cut_end, reason) = match splice.cut {
                Some((cut_start, cut_end, reason)) => {
                    (Some(Time(cut_start)), Some(Time(cut_end)), reason)
                }
                None => (None, None, None),
            };
            self.transitions.push(Transition {
                index: self.transitions.len() + 1,
                preview_time,
                kind: splice.kind,
                episode: splice.episode,
                next: splice.next,
                cut_start,
                cut_end,
                reason,
            });
        }
    }

    fn push(&mut self, mut piece: Piece) {
        piece.output_start = self.duration();
        self.pieces.push(piece);
    }

    fn duration(&self) -> f64 {
        self.pieces.last().map_or(0.0, Piece::output_end)
    }
}

/// The opening, every cut inside an episode, every boundary between episodes (through the
/// interstitial) and the ending, each with `context` seconds either side.
fn windows(input: &Input, timeline: &Timeline, context: f64) -> Vec<Window> {
    let mut windows = Vec::new();
    if let Some(first) = timeline.episodes.first() {
        let opening = Splice {
            at: 0.0,
            kind: Kind::Opening,
            episode: first.id.clone(),
            next: None,
            cut: None,
        };
        windows.push(Window::around(0.0, context, opening));
    }
    for (index, (span, episode)) in timeline.episodes.iter().zip(&input.episodes).enumerate() {
        let pieces = &timeline.pieces[span.pieces.clone()];
        windows.extend(cut_windows(episode, pieces, context));
        let Some(last) = pieces.last() else { continue };
        let at = last.output_end();
        let (kind, next, until) = match timeline.episodes.get(index + 1) {
            Some(next) => (Kind::EpisodeBoundary, Some(next.id.clone()), next.start + context),
            None => (Kind::Ending, None, timeline.duration()),
        };
        let splice = Splice { at, kind, episode: span.id.clone(), next, cut: None };
        windows.push(Window::around(at - context, until, splice));
    }
    windows
}

/// Where one kept stretch of an episode gives way to the next: a cut with audio on both sides.
fn cut_windows<'a>(
    episode: &'a InputEpisode,
    pieces: &'a [Piece],
    context: f64,
) -> impl Iterator<Item = Window> + 'a {
    pieces.windows(2).map(move |pair| {
        let (cut_start, cut_end) = (source(&pair[0]).1, source(&pair[1]).0);
        let reason = episode
            .cuts
            .iter()
            .find(|cut| (cut.start.seconds() - cut_start).abs() < EPSILON)
            .map(|cut| cut.reason.clone());
        let at = pair[0].output_end();
        let splice = Splice {
            at,
            kind: Kind::Cut,
            episode: episode.id.clone(),
            next: None,
            cut: Some((cut_start, cut_end, reason)),
        };
        Window::around(at - context, at + context, splice)
    })
}

/// Overlapping windows joined, in the order the render plays them.
fn merged(mut windows: Vec<Window>) -> Vec<Window> {
    windows.sort_by(|a, b| a.from.total_cmp(&b.from));
    let mut merged: Vec<Window> = Vec::with_capacity(windows.len());
    for window in windows {
        match merged.last_mut() {
            Some(last) if window.from <= last.to + EPSILON => {
                last.to = last.to.max(window.to);
                last.splices.extend(window.splices);
            }
            _ => merged.push(window),
        }
    }
    for window in &mut merged {
        window.splices.sort_by(|a, b| a.at.total_cmp(&b.at));
    }
    merged
}

/// The part of `piece` inside `[from, to)` of the rendered file, keeping the render's fades
/// where the slice keeps the piece's own edges.
fn slice(piece: &Piece, from: f64, to: f64) -> Option<Piece> {
    let (start, end) = (from.max(piece.output_start), to.min(piece.output_end()));
    if end - start < EPSILON {
        return None;
    }
    let Source::File { path, start: source_start, .. } = &piece.source else {
        unreachable!("a rendered timeline holds no preview markers")
    };
    let offset = start - piece.output_start;
    let source =
        Source::file(path.clone(), source_start + offset, source_start + offset + (end - start));
    let fade_in = if offset < EPSILON { piece.fade_in } else { EDGE_FADE_SECONDS };
    let fade_out =
        if piece.output_end() - end < EPSILON { piece.fade_out } else { EDGE_FADE_SECONDS };
    Some(Piece::new(source, piece.gain_db, 0.0).faded(fade_in, fade_out))
}

fn source(piece: &Piece) -> (f64, f64) {
    match piece.source {
        Source::File { start, end, .. } => (start, end),
        Source::Beep => unreachable!("a rendered timeline holds no preview markers"),
    }
}

#[cfg(test)]
mod tests;
