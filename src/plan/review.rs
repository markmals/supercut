mod markdown;

use serde::Serialize;

pub use markdown::markdown;

use crate::commands::Context;
use crate::plan::timeline::{self, EpisodeSpan, InputEpisode};
use crate::plan::{Cut, EDGE_FADE_SECONDS, Plan, resolve};
use crate::timecode::Time;
use crate::transcript::Transcript;

/// Words of transcript quoted either side of a cut edge.
const CONTEXT_WORDS: usize = 12;
/// Words of show notes a chapter line carries.
const SUMMARY_WORDS: usize = 16;

/// A plan explained: what each cut removes, what the listener hears around it, and the totals.
#[derive(Debug, Serialize)]
pub struct Review {
    pub title: String,
    pub show: String,
    pub episodes: Vec<EpisodeReview>,
    pub totals: Totals,
}

#[derive(Debug, Serialize)]
pub struct EpisodeReview {
    pub id: String,
    pub title: String,
    pub duration: Time,
    pub kept: Time,
    pub plays_at: Time,
    /// Whether the interstitial plays after the episode; not after one that ends on its bumper.
    pub interstitial_after: bool,
    pub cuts: Vec<CutReview>,
    pub chapters: Vec<ChapterReview>,
}

#[derive(Debug, Serialize)]
pub struct CutReview {
    pub index: usize,
    pub start: Time,
    pub end: Time,
    pub length: Time,
    pub reason: String,
    pub fade_out: f64,
    pub fade_in: f64,
    /// Transcript around the splice, absent until the episode is transcribed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<CutContext>,
}

/// What is heard before and after the splice, and what goes away between them.
#[derive(Debug, Serialize)]
pub struct CutContext {
    pub before: String,
    pub cut_starts: String,
    pub cut_ends: String,
    pub after: String,
}

#[derive(Debug, Serialize)]
pub struct ChapterReview {
    pub start: Time,
    pub end: Time,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Totals {
    pub episodes: usize,
    pub cuts: usize,
    pub removed: Time,
    pub kept: Time,
    pub output_duration: Time,
}

pub fn review(context: &Context, plan: &Plan) -> anyhow::Result<Review> {
    let resolved = resolve::resolve(context, plan)?;
    let timeline = timeline::build(&resolved.input);

    let mut episodes = Vec::new();
    for (span, episode) in timeline.episodes.iter().zip(&resolved.input.episodes) {
        let transcript = resolved.transcript(&episode.id)?;
        let interstitial_after = episode.interstitial_after && resolved.input.interstitial.is_some();
        episodes.push(episode_review(span, episode, interstitial_after, transcript.as_ref()));
    }
    let source: f64 = resolved.input.episodes.iter().map(|episode| episode.duration).sum();
    let kept: f64 = timeline.episodes.iter().map(|span| span.kept_seconds).sum();
    let totals = Totals {
        episodes: episodes.len(),
        cuts: episodes.iter().map(|episode| episode.cuts.len()).sum(),
        removed: Time(source - kept),
        kept: Time(kept),
        output_duration: Time(timeline.duration()),
    };
    Ok(Review { title: plan.title.clone(), show: resolved.show.title.clone(), episodes, totals })
}

fn episode_review(
    span: &EpisodeSpan,
    episode: &InputEpisode,
    interstitial_after: bool,
    transcript: Option<&Transcript>,
) -> EpisodeReview {
    let cuts = episode
        .cuts
        .iter()
        .enumerate()
        .map(|(index, cut)| cut_review(index, cut, episode.duration, transcript))
        .collect();
    let chapters = span
        .chapters
        .iter()
        .map(|chapter| ChapterReview {
            start: Time(chapter.start),
            end: Time(chapter.end),
            title: chapter.title.clone(),
            description: chapter.description.as_deref().map(summarize),
        })
        .collect();
    EpisodeReview {
        id: span.id.clone(),
        title: span.title.clone(),
        duration: Time(episode.duration),
        kept: Time(span.kept_seconds),
        plays_at: Time(span.start),
        interstitial_after,
        cuts,
        chapters,
    }
}

fn cut_review(index: usize, cut: &Cut, duration: f64, transcript: Option<&Transcript>) -> CutReview {
    let start = cut.start.seconds();
    let end = cut.end.map_or(duration, Time::seconds);
    CutReview {
        index: index + 1,
        start: Time(start),
        end: Time(end),
        length: Time(end - start),
        reason: cut.reason.clone(),
        fade_out: cut.fade_out.unwrap_or(EDGE_FADE_SECONDS),
        fade_in: cut.fade_in.unwrap_or(EDGE_FADE_SECONDS),
        context: transcript.map(|transcript| cut_context(transcript, start, end)),
    }
}

fn cut_context(transcript: &Transcript, start: f64, end: f64) -> CutContext {
    let before = words_before(transcript, start);
    let inside: Vec<&str> = transcript.words_between(start, end).map(|word| word.text.as_str()).collect();
    let after: Vec<&str> = transcript
        .words()
        .filter(|word| word.start >= end)
        .take(CONTEXT_WORDS)
        .map(|word| word.text.as_str())
        .collect();
    let (opening, closing) = match inside.len() > CONTEXT_WORDS * 2 {
        true => (&inside[..CONTEXT_WORDS], &inside[inside.len() - CONTEXT_WORDS..]),
        false => (&inside[..], &[][..]),
    };
    CutContext {
        before: before.join(" "),
        cut_starts: opening.join(" "),
        cut_ends: closing.join(" "),
        after: after.join(" "),
    }
}

/// The last words heard before the splice.
fn words_before(transcript: &Transcript, start: f64) -> Vec<&str> {
    let mut words: Vec<&str> = Vec::new();
    for word in transcript.words().take_while(|word| word.end <= start) {
        if words.len() == CONTEXT_WORDS {
            words.remove(0);
        }
        words.push(word.text.as_str());
    }
    words
}

/// Show notes are paragraphs; a chapter line wants the first sentence of them.
fn summarize(description: &str) -> String {
    let mut text: String = description.split_whitespace().take(SUMMARY_WORDS).collect::<Vec<_>>().join(" ");
    if text.len() < description.len() {
        text.push('…');
    }
    text
}
