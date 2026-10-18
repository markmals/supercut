use serde::Serialize;

use crate::commands::Context;
use crate::error::CliError;
use crate::library::{EpisodeIndex, ShowLibrary};
use crate::plan::{Cut, EDGE_FADE_SECONDS, Plan, lint, resolve, timeline};
use crate::timecode::Time;

/// An episode must keep at least this much audio to belong in a supercut.
const MIN_KEPT_SECONDS: f64 = 1.0;

/// What `plan check` reports: errors stop a render, warnings are for the reviewer to judge.
#[derive(Debug, Serialize)]
pub struct Report {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub output_duration: Time,
    pub episodes: Vec<EpisodeReport>,
}

#[derive(Debug, Serialize)]
pub struct EpisodeReport {
    pub id: String,
    pub kept_seconds: f64,
    pub cuts: usize,
}

/// Validates a plan against the config and the library, then lints its cut edges against the
/// audio and the transcripts.
pub fn check(context: &Context, plan: &Plan) -> anyhow::Result<Report> {
    let missing = preflight(context, plan);
    if !missing.is_empty() {
        let episodes = Vec::new();
        return Ok(Report {
            ok: false,
            errors: missing,
            warnings: Vec::new(),
            output_duration: Time(0.0),
            episodes,
        });
    }

    let resolved = resolve::resolve(context, plan)?;
    let errors = cut_errors(&resolved);
    // A timeline only means something for cuts that hold together.
    if !errors.is_empty() {
        return Ok(Report {
            ok: false,
            errors,
            warnings: Vec::new(),
            output_duration: Time(0.0),
            episodes: Vec::new(),
        });
    }

    let timeline = timeline::build(&resolved.input);
    let mut warnings = timeline.warnings.clone();
    warnings.extend(lint::lint(&resolved)?);
    let episodes = timeline
        .episodes
        .iter()
        .map(|span| EpisodeReport {
            id: span.id.clone(),
            kept_seconds: (span.kept_seconds * 1000.0).round() / 1000.0,
            cuts: span.cuts,
        })
        .collect();
    Ok(Report {
        ok: errors.is_empty(),
        errors,
        warnings,
        output_duration: Time(timeline.duration()),
        episodes,
    })
}

/// Every episode's cut errors.
fn cut_errors(resolved: &resolve::Resolved) -> Vec<String> {
    resolved
        .input
        .episodes
        .iter()
        .flat_map(|episode| episode_errors(&episode.id, &episode.cuts, episode.duration))
        .collect()
}

/// Refuses to work from a plan whose cuts don't hold together, so render and preview never
/// quietly reinterpret what `plans validate` would reject.
pub fn require_valid_cuts(resolved: &resolve::Resolved) -> anyhow::Result<()> {
    let errors = cut_errors(resolved);
    if errors.is_empty() {
        return Ok(());
    }
    let invalid = CliError::invalid(format!("the plan's cuts are invalid: {}", errors.join("; ")))
        .hint("supercut plans validate <plan>")
        .detail("errors", errors);
    Err(invalid.into())
}

/// Everything that has to exist before the audio can be measured at all.
fn preflight(context: &Context, plan: &Plan) -> Vec<String> {
    let (show, library) = match context.show(&plan.show) {
        Ok(found) => found,
        Err(error) => return vec![format!("{error:#}")],
    };
    if plan.episodes.is_empty() {
        return vec!["the plan lists no episodes".to_string()];
    }
    let index = match library.load_synced_index() {
        Ok(index) => index,
        Err(error) => return vec![format!("{error:#}")],
    };

    let mut errors: Vec<String> = plan
        .episodes
        .iter()
        .filter_map(|episode| audio_error(&library, &index, &episode.id))
        .collect();
    if let Some(path) = resolve::interstitial_path(show, plan)
        && !path.exists()
    {
        errors.push(format!("the interstitial {} does not exist", path.display()));
    }
    errors
}

fn audio_error(library: &ShowLibrary, index: &EpisodeIndex, id: &str) -> Option<String> {
    let episode = match index.get(id) {
        Ok(episode) => episode,
        Err(error) => return Some(format!("{error:#}")),
    };
    let path = match library.audio_path(episode) {
        Ok(path) => path,
        Err(error) => return Some(format!("{error:#}")),
    };
    (!path.exists()).then(|| format!("{id}: the audio file {} is missing", path.display()))
}

/// Cuts have to be ordered, disjoint and inside the episode, and leave some of it, before their
/// fades mean anything.
pub fn episode_errors(id: &str, cuts: &[Cut], duration: f64) -> Vec<String> {
    let errors = range_errors(id, cuts, duration);
    if !errors.is_empty() {
        return errors;
    }
    let removed: f64 = cuts.iter().map(|cut| cut_end(cut, duration) - cut.start.seconds()).sum();
    if duration - removed < MIN_KEPT_SECONDS {
        return vec![format!(
            "{id}: the cuts remove the whole episode; take it out of the plan instead"
        )];
    }
    fade_errors(id, cuts, duration)
}

fn range_errors(id: &str, cuts: &[Cut], duration: f64) -> Vec<String> {
    let mut errors = Vec::new();
    let mut previous_end = 0.0;
    for (index, cut) in cuts.iter().enumerate() {
        let label = format!("{id} cut {} at {}", index + 1, cut.start);
        let (start, end) = (cut.start.seconds(), cut_end(cut, duration));
        if start < 0.0 || start >= duration {
            errors.push(format!(
                "{label}: starts outside the episode, which runs 0:00.000 to {}",
                Time(duration)
            ));
        } else if end > duration {
            errors.push(format!(
                "{label}: ends at {}, past the end of the episode ({})",
                Time(end),
                Time(duration)
            ));
        } else if end <= start {
            errors.push(format!("{label}: ends at {}, which is not after it starts", Time(end)));
        } else if start < previous_end {
            errors.push(format!(
                "{label}: starts before the cut above it ends ({})",
                Time(previous_end)
            ));
        }
        previous_end = previous_end.max(end);
    }
    errors
}

fn fade_errors(id: &str, cuts: &[Cut], duration: f64) -> Vec<String> {
    let mut errors = Vec::new();
    for (index, cut) in cuts.iter().enumerate() {
        let label = format!("{id} cut {} at {}", index + 1, cut.start);
        let previous = index.checked_sub(1).map(|previous| &cuts[previous]);
        let before =
            cut.start.seconds() - previous.map_or(0.0, |previous| cut_end(previous, duration));
        let after = cuts.get(index + 1).map_or(duration, |next| next.start.seconds())
            - cut_end(cut, duration);
        errors.extend(fade_error(&label, "fade_out", cut.fade_out, before));
        errors.extend(fade_error(&label, "fade_in", cut.fade_in, after));
        // The audio kept between two cuts fades in from the first and out into the second; an
        // edge without a fade of its own still gets the short declicking one.
        if let Some(previous) = previous
            && (previous.fade_in.is_some() || cut.fade_out.is_some())
        {
            let fade_in = previous.fade_in.unwrap_or(EDGE_FADE_SECONDS);
            let fade_out = cut.fade_out.unwrap_or(EDGE_FADE_SECONDS);
            if fade_in + fade_out > before {
                errors.push(format!(
                    "{label}: the {before:.3} s kept before it cannot hold the cut above's {fade_in} s fade_in and this cut's {fade_out} s fade_out"
                ));
            }
        }
    }
    errors
}

fn fade_error(label: &str, name: &str, fade: Option<f64>, kept: f64) -> Option<String> {
    let fade = fade?;
    if fade < 0.0 {
        return Some(format!("{label}: {name} of {fade} s is negative"));
    }
    (fade > kept).then(|| {
        format!("{label}: {name} of {fade} s is longer than the {kept:.3} s of audio it fades")
    })
}

fn cut_end(cut: &Cut, duration: f64) -> f64 {
    cut.end.map_or(duration, Time::seconds)
}

#[cfg(test)]
mod tests {
    use super::episode_errors;
    use crate::plan::Cut;
    use crate::timecode::Time;

    fn cut(start: f64, end: f64) -> Cut {
        Cut {
            start: Time(start),
            end: Some(Time(end)),
            reason: "advert".into(),
            fade_out: None,
            fade_in: None,
        }
    }

    #[test]
    fn accepts_ordered_cuts_inside_the_episode() {
        assert!(episode_errors("a", &[cut(10.0, 20.0), cut(30.0, 40.0)], 100.0).is_empty());
    }

    #[test]
    fn rejects_overlapping_cuts() {
        let errors = episode_errors("a", &[cut(10.0, 30.0), cut(20.0, 40.0)], 100.0);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("starts before the cut above it ends"), "{}", errors[0]);
    }

    #[test]
    fn rejects_cuts_outside_the_episode() {
        let errors = episode_errors("a", &[cut(90.0, 120.0)], 100.0);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("past the end of the episode"), "{}", errors[0]);

        let errors = episode_errors("a", &[cut(120.0, 130.0)], 100.0);
        assert!(errors[0].contains("starts outside the episode"), "{}", errors[0]);
    }

    #[test]
    fn rejects_a_cut_that_does_not_move_forward() {
        let errors = episode_errors("a", &[cut(50.0, 50.0)], 100.0);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("not after it starts"), "{}", errors[0]);
    }

    #[test]
    fn rejects_fades_longer_than_the_audio_they_shape() {
        let mut first = cut(10.0, 20.0);
        first.fade_out = Some(12.0);
        let mut second = cut(30.0, 40.0);
        second.fade_in = Some(0.5);
        let errors = episode_errors("a", &[first, second], 100.0);
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("fade_out of 12 s is longer than the 10.000 s"),
            "{}",
            errors[0]
        );
    }

    #[test]
    fn rejects_negative_fades() {
        let mut only = cut(10.0, 20.0);
        only.fade_in = Some(-1.0);
        let errors = episode_errors("a", &[only], 100.0);
        assert_eq!(errors, vec!["a cut 1 at 0:10.000: fade_in of -1 s is negative".to_string()]);
    }

    #[test]
    fn rejects_cutting_the_whole_episode() {
        let whole = Cut { end: None, ..cut(0.0, 0.0) };
        let errors = episode_errors("a", &[whole], 100.0);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("remove the whole episode"), "{}", errors[0]);
    }

    #[test]
    fn rejects_fades_that_cannot_share_the_audio_between_two_cuts() {
        // One second kept between the cuts, faded in for 0.8 s and out for 0.8 s.
        let mut first = cut(10.0, 20.0);
        first.fade_in = Some(0.8);
        let mut second = cut(21.0, 30.0);
        second.fade_out = Some(0.8);
        let errors = episode_errors("a", &[first, second], 100.0);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("cannot hold"), "{}", errors[0]);
    }

    #[test]
    fn an_explicit_fade_must_leave_room_for_the_default_one_on_the_other_edge() {
        // One second kept, all of it faded in: the declicking fade-out into the next cut is lost.
        let mut first = cut(10.0, 20.0);
        first.fade_in = Some(1.0);
        let errors = episode_errors("a", &[first, cut(21.0, 30.0)], 100.0);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("cannot hold"), "{}", errors[0]);
    }
}
