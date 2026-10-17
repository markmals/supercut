//! Drafting a cut plan from labels: the segments the show always cuts become cuts, the rest
//! become chapters.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::analysis::jingle::Jingles;
use crate::analysis::{Envelope, episode_analysis, episode_envelope};
use crate::labels::{closing, opening};
use crate::commands::Context;
use crate::config::{SegmentKind, ShowConfig};
use crate::error::CliError;
use crate::labels::also_cut::AlsoCut;
use crate::labels::record::{LabeledSegment, LabeledUnit, Labels};
use crate::labels::phrases::{self, Spoken};
use crate::labels::refine::{self, Anchors, Gap, Policy, Search};
use crate::library::{Episode, ShowLibrary};
use crate::plan::{Chapter, Cut, Plan, PlanEpisode};
use crate::timecode::Time;

/// Cuts this close to each other are one stretch; a cut this close to the end runs out the
/// episode.
const CUT_GAP_SECONDS: f64 = 2.0;
/// However short the units around a cut edge are, the pause it moves to may be this far away.
const LEAST_SEARCH_SECONDS: f64 = 10.0;

/// A drafted plan and what the agent should check before rendering it.
pub struct Draft {
    pub plan: Plan,
    pub low_confidence: Vec<Uncertain>,
    /// Episodes whose opening the show's theme recordings could not place.
    pub warnings: Vec<String>,
}

/// A segment the labels are not sure about, and what the draft did with it.
#[derive(Debug, Serialize)]
pub struct Uncertain {
    pub episode: String,
    pub label: String,
    pub start: Time,
    pub end: Time,
    pub probability: f64,
    /// `cut` when the draft cut it anyway (the command line asked for it), `kept` when it was
    /// left in as content.
    pub action: &'static str,
}

/// Drafts a plan covering `episodes`, in the order the library lists them.
pub fn draft(
    context: &Context,
    slug: &str,
    episodes: &[&Episode],
    title: String,
    output: String,
    also_cut: &AlsoCut,
) -> anyhow::Result<Draft> {
    let (show, library) = context.show(slug)?;
    let recordings = Recordings {
        themes: Jingles::load(&show.themes)?,
        outros: Jingles::load(&show.outros)?,
        bumpers: Jingles::load(&show.bumpers)?,
    };
    let mut planned = Vec::with_capacity(episodes.len());
    let mut low_confidence = Vec::new();
    let mut warnings = Vec::new();
    for (position, episode) in episodes.iter().enumerate() {
        let labels = crate::labels::labels(&library, episode)?;
        if !labels.matches(&show.segments) {
            return Err(CliError::precondition(format!(
                "the labels for `{}` were made with different segments",
                episode.id
            ))
            .hint(format!("supercut labels create {slug} {} --force", episode.id))
            .into());
        }
        let dropped = Dropped { kinds: &show.segments, episode: &episode.id, also: also_cut };
        low_confidence.extend(uncertain_segments(&labels, &dropped));
        let drafted = plan_episode(&library, show, &recordings, episode, position == 0, &labels, also_cut)?;
        warnings.extend(unplaced(&recordings, &drafted, &episode.id));
        planned.push(drafted.episode);
    }
    let plan = Plan { show: slug.to_owned(), title, output, interstitial: None, episodes: planned };
    Ok(Draft { plan, low_confidence, warnings })
}

/// Where the plan goes: where the caller said, or `<library>/<show>/plans/<output stem>.json`.
pub fn plan_path(library: &ShowLibrary, output: &str, chosen: Option<PathBuf>) -> PathBuf {
    chosen.unwrap_or_else(|| {
        let stem = Path::new(output).file_stem().unwrap_or(output.as_ref());
        library.plans_dir().join(stem).with_extension("json")
    })
}

/// The show's recurring recordings, decoded once for the whole draft.
struct Recordings {
    themes: Jingles,
    outros: Jingles,
    bumpers: Jingles,
}

struct Drafted {
    episode: PlanEpisode,
    opened_by_theme: bool,
    /// An outro or a bumper ends the episode.
    closed_by_recording: bool,
}

/// What the agent must place by hand: an opening or an ending no recording marks.
fn unplaced(recordings: &Recordings, drafted: &Drafted, id: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    if !recordings.themes.is_empty() && !drafted.opened_by_theme {
        warnings.push(format!("{id}: none of the show's themes plays in its opening, so its opening cut comes from the labels"));
    }
    if !recordings.outros.is_empty() && !drafted.closed_by_recording {
        warnings.push(format!("{id}: none of the show's outros or bumpers ends it, so its credits cut comes from the labels"));
    }
    warnings
}

fn plan_episode(
    library: &ShowLibrary,
    show: &ShowConfig,
    recordings: &Recordings,
    episode: &Episode,
    first_in_supercut: bool,
    labels: &Labels,
    also_cut: &AlsoCut,
) -> anyhow::Result<Drafted> {
    let duration = episode_analysis(library, episode)?.duration;
    let envelope = episode_envelope(library, episode)?;
    let transcript = crate::labels::transcript(library, episode)?;
    let audio = library.audio_path(episode)?;
    let gaps = refine::gaps(&transcript, duration);
    let spoken = phrases::spoken(&transcript);
    let dropped = Dropped { kinds: &show.segments, episode: &episode.id, also: also_cut };
    let mut cuts = cuts(labels, &dropped, &gaps, &spoken, &envelope);
    let theme = recordings.themes.opening_of(&audio)?;
    if let Some(theme) = &theme {
        let end = opening::opening_end(theme, first_in_supercut, &envelope);
        cuts = opening::with_opening(cuts, theme, end);
    }
    let outro = recordings.outros.outro_of(&audio, duration)?;
    if let Some(outro) = &outro {
        cuts = closing::with_outro(cuts, outro);
    }
    let mut cuts = tidy(cuts, duration);
    let bumper = recordings.bumpers.ending_of(&audio, duration)?;
    if let Some(bumper) = &bumper {
        cuts = closing::keeping_bumper(cuts, bumper);
    }
    let chapters = chapters(labels, &dropped, &cuts);
    let id = episode.id.clone();
    let episode = PlanEpisode { id, cuts, chapters, interstitial_after: bumper.is_none(), note: None };
    Ok(Drafted { episode, opened_by_theme: theme.is_some(), closed_by_recording: outro.is_some() || bumper.is_some() })
}

/// Which labels this episode loses, and how each one's edges are placed.
struct Dropped<'a> {
    kinds: &'a [SegmentKind],
    episode: &'a str,
    also: &'a AlsoCut,
}

impl Dropped<'_> {
    /// A cut-kind segment the labels are unsure of stays in: it is reported, not removed. One
    /// the command line named is cut whatever its probability.
    fn cut(&self, segment: &LabeledSegment) -> bool {
        self.also.contains(self.episode, &segment.label)
            || (self.kind(&segment.label).is_some_and(|kind| kind.cut)
                && !crate::labels::uncertain(segment))
    }

    fn policy(&self, label: &str) -> Policy {
        self.kind(label)
            .map(|kind| Policy { start: kind.cut_start, end: kind.cut_end })
            .unwrap_or_default()
    }

    fn title(&self, label: &str) -> Option<&str> {
        self.kind(label).map(|kind| kind.title.as_str())
    }

    fn kind(&self, label: &str) -> Option<&SegmentKind> {
        self.kinds.iter().find(|kind| kind.label == label)
    }

    /// The edges the segment kind's phrases pin down around `bounds`, if it names any.
    fn anchors(&self, label: &str, bounds: (f64, f64), spoken: &[Spoken]) -> Anchors {
        let Some(kind) = self.kind(label) else { return Anchors::default() };
        let start_after = phrases::start_after(spoken, &kind.after_phrases, bounds.0);
        let end_before = phrases::end_before(spoken, &kind.before_phrases, start_after.unwrap_or(bounds.0), bounds.1);
        Anchors { start_after, end_before }
    }
}

fn cuts(labels: &Labels, dropped: &Dropped<'_>, gaps: &[Gap], spoken: &[Spoken], envelope: &Envelope) -> Vec<Cut> {
    let mut cuts: Vec<Cut> = Vec::new();
    let last = labels.segments.len().saturating_sub(1);
    for (index, segment) in labels.segments.iter().enumerate().filter(|(_, segment)| dropped.cut(segment)) {
        let bounds = (segment.start.seconds(), segment.end.seconds());
        let search = Search {
            around_start: neighbourhood(&labels.units, bounds.0),
            around_end: neighbourhood(&labels.units, bounds.1),
        };
        let anchors = dropped.anchors(&segment.label, bounds, spoken);
        let mut edges = refine::refine(bounds, dropped.policy(&segment.label), anchors, search, gaps, envelope);
        // Nothing follows the episode's last segment for its end edge to spare: a word heard
        // inside it (Whisper hears "you" in closing music) is not the hosts coming back.
        if index == last {
            edges.end = edges.end.max(bounds.1);
            edges.fade_in = None;
        }
        let reason = format!("segment {} (p={:.2})", segment.label, segment.probability);
        let start = match cuts.last().and_then(|last| last.end) {
            Some(previous) => edges.start.max(previous.seconds()),
            None => edges.start,
        };
        // The cut before this one already reaches past it; say so rather than cutting twice.
        if start >= edges.end {
            if let Some(last) = cuts.last_mut() {
                last.reason = format!("{} + {reason}", last.reason);
            }
            continue;
        }
        cuts.push(Cut {
            start: Time(start),
            end: Some(Time(edges.end)),
            reason,
            fade_out: edges.fade_out,
            fade_in: edges.fade_in,
        });
    }
    cuts
}

/// Back-to-back cuts become one; a cut that reaches the end of the episode loses its end.
fn tidy(cuts: Vec<Cut>, duration: f64) -> Vec<Cut> {
    let mut tidied: Vec<Cut> = Vec::with_capacity(cuts.len());
    for cut in cuts {
        match tidied.last_mut() {
            Some(previous) if adjacent(previous, &cut) => join(previous, cut),
            _ => tidied.push(cut),
        }
    }
    if let Some(last) = tidied.last_mut()
        && last.end.is_some_and(|end| end.seconds() >= duration - CUT_GAP_SECONDS)
    {
        last.end = None;
        last.fade_in = None;
    }
    tidied
}

fn adjacent(previous: &Cut, next: &Cut) -> bool {
    previous.end.is_some_and(|end| next.start.seconds() - end.seconds() <= CUT_GAP_SECONDS)
}

fn join(previous: &mut Cut, next: Cut) {
    previous.end = next.end;
    previous.reason = format!("{} + {}", previous.reason, next.reason);
    previous.fade_in = next.fade_in;
}

/// One chapter per kept segment, neighbours with the same title merged, and no chapter inside a
/// cut.
fn chapters(labels: &Labels, dropped: &Dropped<'_>, cuts: &[Cut]) -> Vec<Chapter> {
    let mut chapters: Vec<Chapter> = Vec::new();
    for segment in labels.segments.iter().filter(|segment| !dropped.cut(segment)) {
        let Some(title) = dropped.title(&segment.label) else {
            continue;
        };
        if chapters.last().is_some_and(|last| last.title == title) {
            continue;
        }
        let Some(start) = outside_cuts(segment.start.seconds(), cuts) else {
            continue;
        };
        chapters.push(Chapter { start: Time(start), title: title.to_owned(), description: None });
    }
    chapters
}

/// `time` itself, or the end of the cut it falls inside; nothing if that cut runs to the end.
fn outside_cuts(time: f64, cuts: &[Cut]) -> Option<f64> {
    let mut time = time;
    for cut in cuts {
        if time < cut.start.seconds() {
            continue;
        }
        match cut.end {
            Some(end) if time < end.seconds() => time = end.seconds(),
            Some(_) => {}
            None => return None,
        }
    }
    Some(time)
}

/// How far a cut edge labeled at `time` may travel: the unit it lands in and its neighbours,
/// and never less than a few seconds either way, since units can be one utterance long.
fn neighbourhood(units: &[LabeledUnit], time: f64) -> (f64, f64) {
    let Some(last) = units.len().checked_sub(1) else {
        return (time, time);
    };
    let index = units.iter().position(|unit| unit.end.seconds() >= time).unwrap_or(last);
    let span = (units[index.saturating_sub(1)].start.seconds(), units[(index + 1).min(last)].end.seconds());
    (span.0.min(time - LEAST_SEARCH_SECONDS), span.1.max(time + LEAST_SEARCH_SECONDS))
}

/// Uncertain segments whose fate the draft decided: those of a kind it cuts (or was asked to
/// cut). An unsure boundary between two kept kinds changes nothing in the supercut.
fn uncertain_segments<'a>(
    labels: &'a Labels,
    dropped: &'a Dropped<'a>,
) -> impl Iterator<Item = Uncertain> + 'a {
    let at_stake = |segment: &&LabeledSegment| {
        dropped.kind(&segment.label).is_some_and(|kind| kind.cut) || dropped.also.contains(dropped.episode, &segment.label)
    };
    labels.segments.iter().filter(|segment| crate::labels::uncertain(segment)).filter(at_stake).map(
        move |segment: &LabeledSegment| Uncertain {
            episode: labels.episode.clone(),
            label: segment.label.clone(),
            start: segment.start,
            end: segment.end,
            probability: segment.probability,
            action: if dropped.cut(segment) { "cut" } else { "kept" },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::tidy;
    use crate::plan::Cut;
    use crate::timecode::Time;

    fn cut(start: f64, end: f64, reason: &str) -> Cut {
        Cut {
            start: Time(start),
            end: Some(Time(end)),
            reason: reason.to_owned(),
            fade_out: None,
            fade_in: Some(0.25),
        }
    }

    #[test]
    fn cuts_that_meet_become_one() {
        let tidied = tidy(vec![cut(100.0, 200.0, "plug"), cut(201.0, 260.0, "credits")], 600.0);
        assert_eq!(tidied.len(), 1);
        assert_eq!(tidied[0].start, Time(100.0));
        assert_eq!(tidied[0].end, Some(Time(260.0)));
        assert_eq!(tidied[0].reason, "plug + credits");
    }

    #[test]
    fn cuts_with_kept_audio_between_them_stay_apart() {
        let tidied = tidy(vec![cut(100.0, 200.0, "plug"), cut(230.0, 260.0, "credits")], 600.0);
        assert_eq!(tidied.len(), 2);
    }

    #[test]
    fn a_cut_that_reaches_the_end_runs_to_it() {
        let tidied = tidy(vec![cut(100.0, 598.5, "credits")], 600.0);
        assert_eq!(tidied[0].end, None, "no end means to the end of the episode");
        assert_eq!(tidied[0].fade_in, None, "nothing resumes, so nothing fades in");
    }
}
