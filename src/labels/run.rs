//! Labeling episodes: transcripts in, `labels/<id>.json` out.

use anyhow::ensure;
use serde::Serialize;

use crate::analysis::{Envelope, FRAME_SECONDS, episode_analysis, episode_envelope};
use crate::commands::Context;
use crate::config::{SegmentKind, ShowConfig};
use crate::error::CliError;
use crate::labels::jev::{self, Labeler};
use crate::labels::record::{LabeledWindow, Labels, kind_of};
use crate::labels::window::Window;
use crate::labels::{smooth, state, window};
use crate::library::{Episode, ShowLibrary};
use crate::timecode::Time;

/// What `labels create` reports for one episode.
#[derive(Debug, Serialize)]
pub struct Labeled {
    pub id: String,
    pub windows: usize,
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
            let windows = existing.windows.len();
            estimates.push(Estimate {
                id: episode.id.clone(),
                windows,
                requests: 0,
                estimated_input_tokens: 0,
                skipped: true,
            });
            continue;
        }
        let transcript = crate::labels::transcript(&library, episode)?;
        let duration = spoken_duration(&transcript);
        let windows = window::windows(&transcript, &silence(duration));
        let mut tokens = 0;
        for window in &windows {
            let state = state::state(&show.title, episode, window, duration);
            tokens += jev::estimated_tokens(&question, &state)?;
        }
        estimates.push(Estimate {
            id: episode.id.clone(),
            windows: windows.len(),
            requests: windows.len(),
            estimated_input_tokens: tokens,
            skipped: false,
        });
    }
    Ok(estimates)
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
    let windows = window::windows(&transcript, &envelope);
    ensure!(!windows.is_empty(), "the transcript of `{}` has no words", episode.id);

    let answered = ask(labeler, show, episode, &windows, duration)?;
    let input_tokens = answered.iter().map(|(_, tokens)| tokens).sum();
    let answered: Vec<LabeledWindow> = answered.into_iter().map(|(window, _)| window).collect();
    let names: Vec<String> = show.segments.iter().map(|kind| kind.label.clone()).collect();
    let labels = Labels {
        episode: episode.id.clone(),
        model: labeler.model.clone(),
        kinds: show.segments.iter().map(kind_of).collect(),
        segments: smooth::segments(&answered, &names, duration),
        windows: answered,
    };
    labels.save(&library.labels_path(&episode.id))?;
    Ok(summarize(&labels, input_tokens, false))
}

/// One Jev answer per window, with what each cost.
fn ask(
    labeler: &Labeler,
    show: &ShowConfig,
    episode: &Episode,
    windows: &[Window],
    duration: f64,
) -> anyhow::Result<Vec<(LabeledWindow, u64)>> {
    let states: Vec<_> = windows
        .iter()
        .map(|window| state::state(&show.title, episode, window, duration))
        .collect();
    eprintln!("labeling {} ({} windows)", episode.id, windows.len());
    let total = states.len();
    let answers = labeler.answer_all(&states, |done| eprint!("\r  {done}/{total} windows"))?;
    eprintln!();
    Ok(windows
        .iter()
        .zip(answers)
        .map(|(window, answer)| {
            let labeled = LabeledWindow {
                start: Time(window.start),
                end: Time(window.end),
                selected: answer.selected,
                confidence: answer.confidence,
                probabilities: answer.probabilities,
            };
            (labeled, answer.input_tokens)
        })
        .collect())
}

fn summarize(labels: &Labels, input_tokens: u64, skipped: bool) -> Labeled {
    Labeled {
        id: labels.episode.clone(),
        windows: labels.windows.len(),
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

fn spoken_duration(transcript: &crate::transcript::Transcript) -> f64 {
    transcript.words().map(|word| word.end).fold(0.0, f64::max)
}

/// A level envelope of pure silence: what a dry run uses instead of decoding the audio.
fn silence(duration: f64) -> Envelope {
    let frames = (duration / FRAME_SECONDS).ceil().max(1.0) as usize;
    Envelope::from_mean_squares(std::iter::repeat_n(0.0, frames))
}
