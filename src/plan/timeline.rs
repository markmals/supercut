use std::path::PathBuf;

use crate::plan::{Chapter, Cut, EDGE_FADE_SECONDS};
use crate::render::piece::{Piece, Source};
use crate::timecode::Time;

/// Times closer together than this are the same moment.
const EPSILON: f64 = 0.001;
/// A lead-in shorter than this joins the episode's first chapter instead of getting its own.
const LEAD_IN_MERGE_SECONDS: f64 = 30.0;

/// A plan with the audio facts a timeline needs already measured.
pub struct Input {
    /// Loudness every episode and the interstitial are brought to.
    pub target_lufs: f64,
    pub episodes: Vec<InputEpisode>,
    pub interstitial: Option<Clip>,
}

pub struct InputEpisode {
    pub id: String,
    pub title: String,
    pub description: String,
    pub audio: PathBuf,
    pub duration: f64,
    pub integrated_lufs: f64,
    pub cuts: Vec<Cut>,
    pub chapters: Vec<Chapter>,
}

pub struct Clip {
    pub audio: PathBuf,
    pub duration: f64,
    pub integrated_lufs: f64,
}

/// What the rendered file is made of: the audio to splice and the chapters over it.
#[derive(Debug, Default)]
pub struct Timeline {
    pub pieces: Vec<Piece>,
    pub episodes: Vec<EpisodeSpan>,
    pub warnings: Vec<String>,
}

/// One episode's place in the rendered file. The span runs to the next episode, so the
/// interstitial that follows an episode belongs to it.
#[derive(Debug)]
pub struct EpisodeSpan {
    pub id: String,
    pub title: String,
    pub description: String,
    pub start: f64,
    pub end: f64,
    pub kept_seconds: f64,
    pub cuts: usize,
    pub chapters: Vec<SegmentChapter>,
}

#[derive(Clone, Debug)]
pub struct SegmentChapter {
    pub start: f64,
    pub end: f64,
    pub title: String,
    pub description: Option<String>,
}

impl Timeline {
    pub fn duration(&self) -> f64 {
        self.pieces.last().map_or(0.0, Piece::output_end)
    }
}

pub fn build(input: &Input) -> Timeline {
    let mut timeline = Timeline::default();
    for (index, episode) in input.episodes.iter().enumerate() {
        if index > 0
            && let Some(clip) = &input.interstitial
        {
            push_interstitial(&mut timeline, clip, input.target_lufs);
        }
        push_episode(&mut timeline, episode, input.target_lufs);
    }
    close_spans(&mut timeline);
    timeline
}

fn push_interstitial(timeline: &mut Timeline, clip: &Clip, target_lufs: f64) {
    let start = timeline.duration();
    let source = Source::file(clip.audio.clone(), 0.0, clip.duration);
    let piece = Piece::new(source, target_lufs - clip.integrated_lufs, start);
    timeline.pieces.push(piece.faded(EDGE_FADE_SECONDS, EDGE_FADE_SECONDS));
}

fn push_episode(timeline: &mut Timeline, episode: &InputEpisode, target_lufs: f64) {
    let gain_db = target_lufs - episode.integrated_lufs;
    let start = timeline.duration();
    let first = timeline.pieces.len();
    for kept in kept_ranges(&episode.cuts, episode.duration) {
        let output_start = timeline.duration();
        let source = Source::file(episode.audio.clone(), kept.start, kept.end);
        let piece = Piece::new(source, gain_db, output_start);
        timeline.pieces.push(piece.faded(kept.fade_in, kept.fade_out));
    }
    let pieces = &timeline.pieces[first..];
    let kept_seconds = pieces.iter().map(Piece::seconds).sum();
    let chapters = map_chapters(episode, pieces, &mut timeline.warnings);
    timeline.episodes.push(EpisodeSpan {
        id: episode.id.clone(),
        title: episode.title.clone(),
        description: episode.description.clone(),
        start,
        end: start,
        kept_seconds,
        cuts: episode.cuts.len(),
        chapters,
    });
}

/// One stretch of an episode that survives its cuts.
pub struct Kept {
    pub start: f64,
    pub end: f64,
    pub fade_in: f64,
    pub fade_out: f64,
}

/// The complement of `cuts` in `[0, duration]`. Every cut edge fades; the episode's own start
/// and end do not, because nothing is spliced there.
pub fn kept_ranges(cuts: &[Cut], duration: f64) -> Vec<Kept> {
    let mut ranges = Vec::new();
    let mut position = 0.0;
    let mut fade_in = 0.0;
    for cut in cuts {
        let start = cut.start.seconds().clamp(0.0, duration);
        let end = cut.end.map_or(duration, Time::seconds).clamp(start, duration);
        if start > position + EPSILON {
            let fade_out = cut.fade_out.unwrap_or(EDGE_FADE_SECONDS);
            ranges.push(Kept { start: position, end: start, fade_in, fade_out });
        }
        fade_in = cut.fade_in.unwrap_or(EDGE_FADE_SECONDS);
        position = position.max(end);
    }
    if position + EPSILON < duration {
        ranges.push(Kept { start: position, end: duration, fade_in, fade_out: 0.0 });
    }
    ranges
}

/// The episode's chapters in output time. A chapter inside a cut moves to where the audio
/// resumes; one with no audio left after it is dropped.
fn map_chapters(episode: &InputEpisode, pieces: &[Piece], warnings: &mut Vec<String>) -> Vec<SegmentChapter> {
    let mut ordered: Vec<&Chapter> = episode.chapters.iter().collect();
    ordered.sort_by(|left, right| left.start.seconds().total_cmp(&right.start.seconds()));
    let mut mapped: Vec<SegmentChapter> = Vec::new();
    for chapter in ordered {
        let Some(start) = map_time(pieces, chapter.start.seconds()) else {
            warnings.push(format!(
                "{}: chapter `{}` at {} falls inside a cut with no audio after it and was dropped",
                episode.id, chapter.title, chapter.start
            ));
            continue;
        };
        if mapped.last().is_some_and(|previous| start <= previous.start + EPSILON) {
            warnings.push(format!(
                "{}: chapter `{}` at {} lands on the chapter before it and was dropped",
                episode.id, chapter.title, chapter.start
            ));
            continue;
        }
        let description = chapter.description.clone();
        mapped.push(SegmentChapter { start, end: start, title: chapter.title.clone(), description });
    }
    lead_in(episode, pieces, &mut mapped);
    mapped
}

/// Audio before the first chapter still needs a chapter. A short lead-in (a sting, room tone)
/// joins the first chapter; a long one gets its own, titled after the episode. Either way the
/// episode's first chapter carries its show notes, for players that read chapters flat.
fn lead_in(episode: &InputEpisode, pieces: &[Piece], mapped: &mut Vec<SegmentChapter>) {
    let (Some(piece), Some(first)) = (pieces.first(), mapped.first_mut()) else { return };
    let start = piece.output_start;
    if first.start - start <= LEAD_IN_MERGE_SECONDS {
        first.start = start;
        first.description.get_or_insert_with(|| episode.description.clone());
        return;
    }
    let description = Some(episode.description.clone());
    mapped.insert(0, SegmentChapter { start, end: start, title: episode.title.clone(), description });
}

/// Where a position in the episode's own audio ends up in the rendered file.
fn map_time(pieces: &[Piece], time: f64) -> Option<f64> {
    for piece in pieces {
        let Source::File { start, end, .. } = &piece.source else { continue };
        if time < *end {
            return Some(piece.output_start + (time - start).max(0.0));
        }
    }
    None
}

/// Chapters and episodes tile the file, so each one ends where the next begins.
fn close_spans(timeline: &mut Timeline) {
    let duration = timeline.duration();
    let starts: Vec<f64> = timeline.episodes.iter().map(|span| span.start).collect();
    for (index, span) in timeline.episodes.iter_mut().enumerate() {
        span.end = starts.get(index + 1).copied().unwrap_or(duration);
        for position in 0..span.chapters.len() {
            span.chapters[position].end =
                span.chapters.get(position + 1).map_or(span.end, |next| next.start);
        }
    }
}

#[cfg(test)]
mod tests;
