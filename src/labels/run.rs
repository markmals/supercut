//! Labeling episodes: transcripts in, `labels/<id>.json` out. Two passes — ~30 s windows over
//! the whole episode, then utterances where a boundary has to be exact.

use anyhow::ensure;
use serde::Serialize;

use crate::analysis::{Envelope, FRAME_SECONDS, episode_analysis, episode_envelope};
use crate::commands::Context;
use crate::config::{SegmentKind, ShowConfig};
use crate::error::CliError;
use crate::labels::jev::{self, Labeler};
use crate::labels::record::{LabeledUnit, Labels, kind_of};
use crate::labels::unit::{Sound, Unit};
use crate::labels::{fine, smooth, state, utterance, window};
use crate::library::{Episode, ShowLibrary};
use crate::timecode::Time;
use crate::transcript::Transcript;

/// Boundaries a dry run assumes an unlabeled episode has, for the fine pass it cannot plan yet.
const ASSUMED_BOUNDARIES: usize = 12;

/// What `labels create` reports for one episode.
#[derive(Debug, Serialize)]
pub struct Labeled {
    pub id: String,
    pub windows: usize,
    /// Units the segments were smoothed from: utterances where the fine pass ran, windows
    /// elsewhere.
    pub units: usize,
    pub segments: Vec<Summary>,
    pub input_tokens: u64,
    /// Set when the episode already carried labels made with the show's current segments.
    #[serde(skip_serializing_if = "is_false")]
    pub skipped: bool,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub label: String,
    pub start: Time,
    pub end: Time,
    pub probability: f64,
}

/// What `labels create --dry-run` reports for one episode: no requests, no writes.
#[derive(Debug, Serialize)]
pub struct Estimate {
    pub id: String,
    pub windows: usize,
    /// Utterances of the fine pass: exact for the opening and closing, assumed around the
    /// boundaries the coarse pass has not found yet.
    pub utterances: usize,
    /// Coarse boundaries the utterance count assumes.
    pub assumed_boundaries: usize,
    pub requests: usize,
    pub estimated_input_tokens: u64,
    #[serde(skip_serializing_if = "is_false")]
    pub skipped: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Labels every episode that needs it, leaving the rest alone.
pub fn label_episodes(
    context: &Context,
    slug: &str,
    episodes: &[&Episode],
    force: bool,
) -> anyhow::Result<Vec<Labeled>> {
    let (show, library) = context.show(slug)?;
    let kinds = segment_kinds(show, slug)?;
    let mut labeler: Option<Labeler> = None;
    let mut labeled = Vec::with_capacity(episodes.len());
    for episode in episodes {
        if !force && let Some(existing) = current_labels(&library, episode, kinds)? {
            eprintln!("{}: already labeled", episode.id);
            labeled.push(summarize(&existing, 0, true));
            continue;
        }
        if labeler.is_none() {
            labeler = Some(Labeler::new(&context.config.jev, &show.title, kinds)?);
        }
        let labeler = labeler.as_ref().expect("the labeler was just built");
        labeled.push(label_episode(&library, labeler, episode, show)?);
    }
    Ok(labeled)
}

fn label_episode(
    library: &ShowLibrary,
    labeler: &Labeler,
    episode: &Episode,
    show: &ShowConfig,
) -> anyhow::Result<Labeled> {
    let transcript = crate::labels::transcript(library, episode)?;
    let duration = episode_analysis(library, episode)?.duration;
    let envelope = episode_envelope(library, episode)?;
    let sound = Sound { envelope: &envelope, unreliable: &transcript.unreliable };
    let names: Vec<String> = show.segments.iter().map(|kind| kind.label.clone()).collect();

    let windows = window::windows(&transcript, sound);
    ensure!(!windows.is_empty(), "the transcript of `{}` has no words", episode.id);
    let (coarse, coarse_tokens) = ask(labeler, show, episode, &windows, duration, "windows")?;

    let rough = smooth::segments(&coarse, &names, duration);
    let regions = fine::regions(&coarse, &rough, duration);
    let utterances = utterance::utterances(&transcript, sound, &regions);
    let (fine, fine_tokens) = ask(labeler, show, episode, &utterances, duration, "utterances")?;

    let units = fine::mixed(&coarse, fine, &regions);
    let labels = Labels {
        episode: episode.id.clone(),
        model: labeler.model.clone(),
        kinds: show.segments.iter().map(kind_of).collect(),
        segments: smooth::segments(&units, &names, duration),
        windows: coarse,
        units,
    };
    labels.save(&library.labels_path(&episode.id))?;
    Ok(summarize(&labels, coarse_tokens + fine_tokens, false))
}

/// One Jev answer per unit, with what the pass cost.
fn ask(
    labeler: &Labeler,
    show: &ShowConfig,
    episode: &Episode,
    units: &[Unit],
    duration: f64,
    noun: &str,
) -> anyhow::Result<(Vec<LabeledUnit>, u64)> {
    let states: Vec<_> =
        units.iter().map(|unit| state::state(&show.title, episode, unit, duration)).collect();
    let total = states.len();
    eprintln!("labeling {} ({total} {noun})", episode.id);
    let terminal = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let answers = labeler.answer_all(&states, |done| {
        if terminal {
            eprint!("\r  {done}/{total} {noun}");
        }
    })?;
    if terminal {
        eprintln!();
    }
    let tokens = answers.iter().map(|answer| answer.input_tokens).sum();
    let labeled = units
        .iter()
        .zip(answers)
        .map(|(unit, answer)| LabeledUnit {
            start: Time(unit.start),
            end: Time(unit.end),
            selected: answer.selected,
            confidence: answer.confidence,
            probabilities: answer.probabilities,
        })
        .collect();
    Ok((labeled, tokens))
}

/// What labeling would cost, without calling Jev or touching the library.
pub fn estimate_episodes(
    context: &Context,
    slug: &str,
    episodes: &[&Episode],
    force: bool,
) -> anyhow::Result<Vec<Estimate>> {
    let (show, library) = context.show(slug)?;
    let kinds = segment_kinds(show, slug)?;
    let question = jev::question(&show.title, kinds)?;
    let mut estimates = Vec::with_capacity(episodes.len());
    for episode in episodes {
        if !force && let Some(existing) = current_labels(&library, episode, kinds)? {
            estimates.push(Estimate {
                id: episode.id.clone(),
                windows: existing.windows.len(),
                utterances: 0,
                assumed_boundaries: 0,
                requests: 0,
                estimated_input_tokens: 0,
                skipped: true,
            });
            continue;
        }
        let transcript = crate::labels::transcript(&library, episode)?;
        estimates.push(estimate_episode(show, episode, &transcript, &question)?);
    }
    Ok(estimates)
}

/// Both passes priced from the transcript alone: the coarse windows exactly, the fine pass from
/// the utterances of the opening and closing plus a typical episode's worth of boundaries.
fn estimate_episode(
    show: &ShowConfig,
    episode: &Episode,
    transcript: &Transcript,
    question: &jev::Question,
) -> anyhow::Result<Estimate> {
    let duration = spoken_duration(transcript);
    let silence = silence(duration);
    let sound = Sound { envelope: &silence, unreliable: &transcript.unreliable };
    let windows = window::windows(transcript, sound);
    let known = fine::opening_and_closing(duration);
    let utterances = utterance::utterances(transcript, sound, &known);
    let everywhere = utterance::utterances(transcript, sound, &[(0.0, duration)]);

    let coarse_tokens = tokens_for(show, episode, &windows, duration, question)?;
    let fine_tokens = tokens_for(show, episode, &utterances, duration, question)?;
    let per_boundary = fine::utterances_per_boundary(everywhere.len(), duration);
    let assumed = ASSUMED_BOUNDARIES * per_boundary;
    let per_utterance = fine_tokens.checked_div(utterances.len() as u64).unwrap_or(0);
    Ok(Estimate {
        id: episode.id.clone(),
        windows: windows.len(),
        utterances: utterances.len() + assumed,
        assumed_boundaries: ASSUMED_BOUNDARIES,
        requests: windows.len() + utterances.len() + assumed,
        estimated_input_tokens: coarse_tokens + fine_tokens + per_utterance * assumed as u64,
        skipped: false,
    })
}

fn tokens_for(
    show: &ShowConfig,
    episode: &Episode,
    units: &[Unit],
    duration: f64,
    question: &jev::Question,
) -> anyhow::Result<u64> {
    let mut tokens = 0;
    for unit in units {
        let state = state::state(&show.title, episode, unit, duration);
        tokens += jev::estimated_tokens(question, &state)?;
    }
    Ok(tokens)
}

fn summarize(labels: &Labels, input_tokens: u64, skipped: bool) -> Labeled {
    Labeled {
        id: labels.episode.clone(),
        windows: labels.windows.len(),
        units: labels.units.len(),
        segments: labels
            .segments
            .iter()
            .map(|segment| Summary {
                label: segment.label.clone(),
                start: segment.start,
                end: segment.end,
                probability: segment.probability,
            })
            .collect(),
        input_tokens,
        skipped,
    }
}

fn segment_kinds<'a>(show: &'a ShowConfig, slug: &str) -> anyhow::Result<&'a [SegmentKind]> {
    if show.segments.is_empty() {
        return Err(CliError::precondition(format!("show `{slug}` configures no segments"))
            .hint(format!("add `[[shows.{slug}.segments]]` entries to the config file"))
            .into());
    }
    Ok(&show.segments)
}

/// The episode's labels when they were made with the show's current segments.
fn current_labels(
    library: &ShowLibrary,
    episode: &Episode,
    kinds: &[SegmentKind],
) -> anyhow::Result<Option<Labels>> {
    let path = library.labels_path(&episode.id);
    if !path.exists() {
        return Ok(None);
    }
    let labels = Labels::load(&path)?;
    Ok(labels.matches(kinds).then_some(labels))
}

fn spoken_duration(transcript: &Transcript) -> f64 {
    transcript.words().map(|word| word.end).fold(0.0, f64::max)
}

/// A level envelope of pure silence: what a dry run uses instead of decoding the audio.
fn silence(duration: f64) -> Envelope {
    let frames = (duration / FRAME_SECONDS).ceil().max(1.0) as usize;
    Envelope::from_mean_squares(std::iter::repeat_n(0.0, frames))
}
