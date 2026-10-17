//! Drafting a cut plan from labels: the segments the show always cuts become cuts, the rest
//! become chapters.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::analysis::{Envelope, episode_analysis, episode_envelope};
use crate::commands::Context;
use crate::config::{SegmentKind, ShowConfig};
use crate::error::CliError;
use crate::labels::record::{LabeledSegment, LabeledWindow, Labels};
use crate::labels::refine::{self, Gap, Search};
use crate::library::{Episode, ShowLibrary};
use crate::plan::{Chapter, Cut, Plan, PlanEpisode};
use crate::timecode::Time;

/// A drafted plan and what the agent should check before rendering it.
pub struct Draft {
    pub plan: Plan,
    pub low_confidence: Vec<Uncertain>,
}

/// A segment the labels are not sure about.
#[derive(Debug, Serialize)]
pub struct Uncertain {
    pub episode: String,
    pub label: String,
    pub start: Time,
    pub end: Time,
    pub probability: f64,
}

/// Drafts a plan covering `episodes`, in the order the library lists them.
pub fn draft(
    context: &Context,
    slug: &str,
    episodes: &[&Episode],
    title: String,
    output: String,
) -> anyhow::Result<Draft> {
    let (show, library) = context.show(slug)?;
    let mut planned = Vec::with_capacity(episodes.len());
    let mut low_confidence = Vec::new();
    for episode in episodes {
        let labels = crate::labels::labels(&library, episode)?;
        if !labels.matches(&show.segments) {
            return Err(CliError::precondition(format!(
                "the labels for `{}` were made with different segments",
                episode.id
            ))
            .hint(format!("supercut labels create {slug} {} --force", episode.id))
            .into());
        }
        low_confidence.extend(uncertain_segments(&labels));
        planned.push(plan_episode(&library, show, episode, &labels)?);
    }
    let plan = Plan { show: slug.to_owned(), title, output, interstitial: None, episodes: planned };
    Ok(Draft { plan, low_confidence })
}

/// Where the plan goes: where the caller said, or `<library>/<show>/plans/<output stem>.json`.
pub fn plan_path(library: &ShowLibrary, output: &str, chosen: Option<PathBuf>) -> PathBuf {
    chosen.unwrap_or_else(|| {
        let stem = Path::new(output).file_stem().unwrap_or(output.as_ref());
        library.plans_dir().join(stem).with_extension("json")
    })
}

fn plan_episode(
    library: &ShowLibrary,
    show: &ShowConfig,
    episode: &Episode,
    labels: &Labels,
) -> anyhow::Result<PlanEpisode> {
    let duration = episode_analysis(library, episode)?.duration;
    let envelope = episode_envelope(library, episode)?;
    let transcript = crate::labels::transcript(library, episode)?;
    let gaps = refine::gaps(&transcript, duration);
    let cuts = cuts(labels, &show.segments, &gaps, &envelope);
    let chapters = chapters(labels, &show.segments, &cuts);
    Ok(PlanEpisode { id: episode.id.clone(), cuts, chapters, note: None })
}

fn cuts(
    labels: &Labels,
    kinds: &[SegmentKind],
    gaps: &[Gap],
    envelope: &Envelope,
) -> Vec<Cut> {
    let mut cuts: Vec<Cut> = Vec::new();
    for segment in labels.segments.iter().filter(|segment| is_cut(kinds, &segment.label)) {
        let bounds = (segment.start.seconds(), segment.end.seconds());
        let search = Search {
            around_start: neighbourhood(&labels.windows, bounds.0),
            around_end: neighbourhood(&labels.windows, bounds.1),
        };
        let edges = refine::refine(bounds, search, gaps, envelope);
        let start = match cuts.last().and_then(|last| last.end) {
            Some(previous) => edges.start.max(previous.seconds()),
            None => edges.start,
        };
        if start >= edges.end {
            continue;
        }
        cuts.push(Cut {
            start: Time(start),
            end: Some(Time(edges.end)),
            reason: format!("segment {} (p={:.2})", segment.label, segment.probability),
            fade_out: edges.fade_out,
            fade_in: edges.fade_in,
        });
    }
    cuts
}

/// One chapter per kept segment, neighbours with the same title merged, and no chapter starting
/// inside a cut.
fn chapters(labels: &Labels, kinds: &[SegmentKind], cuts: &[Cut]) -> Vec<Chapter> {
    let mut chapters: Vec<Chapter> = Vec::new();
    for segment in labels.segments.iter().filter(|segment| !is_cut(kinds, &segment.label)) {
        let Some(title) = title_of(kinds, &segment.label) else {
            continue;
        };
        if chapters.last().is_some_and(|last| last.title == title) {
            continue;
        }
        chapters.push(Chapter {
            start: Time(outside_cuts(segment.start.seconds(), cuts)),
            title: title.to_owned(),
            description: None,
        });
    }
    chapters
}

/// `time` itself, or the end of the cut it falls inside.
fn outside_cuts(time: f64, cuts: &[Cut]) -> f64 {
    cuts.iter().fold(time, |time, cut| {
        let end = cut.end.map_or(f64::INFINITY, Time::seconds);
        if time >= cut.start.seconds() && time < end { end } else { time }
    })
}

/// How far a cut edge labeled at `time` may travel: the window it lands in and its neighbours.
fn neighbourhood(windows: &[LabeledWindow], time: f64) -> (f64, f64) {
    let Some(last) = windows.len().checked_sub(1) else {
        return (time, time);
    };
    let index = windows.iter().position(|window| window.end.seconds() >= time).unwrap_or(last);
    (windows[index.saturating_sub(1)].start.seconds(), windows[(index + 1).min(last)].end.seconds())
}

fn uncertain_segments(labels: &Labels) -> impl Iterator<Item = Uncertain> + '_ {
    labels.segments.iter().filter(|segment| crate::labels::uncertain(segment)).map(
        |segment: &LabeledSegment| Uncertain {
            episode: labels.episode.clone(),
            label: segment.label.clone(),
            start: segment.start,
            end: segment.end,
            probability: segment.probability,
        },
    )
}

fn is_cut(kinds: &[SegmentKind], label: &str) -> bool {
    kinds.iter().any(|kind| kind.label == label && kind.cut)
}

fn title_of<'a>(kinds: &'a [SegmentKind], label: &str) -> Option<&'a str> {
    kinds.iter().find(|kind| kind.label == label).map(|kind| kind.title.as_str())
}
