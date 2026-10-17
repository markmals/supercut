use crate::analysis::{self, Envelope};
use crate::plan::EDGE_FADE_SECONDS;
use crate::plan::resolve::Resolved;
use crate::timecode::Time;
use crate::transcript::Transcript;

/// A word this close to an edge is cut through by it.
const WORD_MARGIN_SECONDS: f64 = 0.1;
/// Audio either side of an edge that decides whether the splice is audible.
const EDGE_WINDOW_SECONDS: f64 = 0.05;
/// Above this level the edge is in the middle of something, not between things.
const LOUD_EDGE_DB: f32 = -40.0;
/// How far from an edge a better, quieter place to cut is worth suggesting.
const QUIET_SEARCH_SECONDS: f64 = 5.0;
/// A gap shorter than this is a breath, not a seam.
const QUIET_MIN_SECONDS: f64 = 0.25;

/// Warnings about where the cuts land: mid-word, or in loud audio that will click.
pub fn lint(resolved: &Resolved) -> anyhow::Result<Vec<String>> {
    let mut warnings = Vec::new();
    for (episode, planned) in resolved.episodes.iter().zip(&resolved.input.episodes) {
        if planned.cuts.is_empty() {
            continue;
        }
        let envelope = analysis::episode_envelope(&resolved.library, episode)?;
        let transcript = resolved.transcript(&episode.id)?;
        for (index, cut) in planned.cuts.iter().enumerate() {
            let label = format!("{} cut {}", episode.id, index + 1);
            let end = cut.end.map_or(planned.duration, Time::seconds);
            let start = cut.start.seconds();
            let edges = [
                (format!("{label}: the start"), start, cut.fade_out),
                (format!("{label}: the end"), end, cut.fade_in),
            ];
            // A cut from the top of the file or to its end joins nothing there: no splice to lint.
            let splices = edges.into_iter().filter(|(_, edge, _)| *edge > 0.0 && *edge < planned.duration);
            for (label, edge, fade) in splices {
                warnings.extend(edge_warnings(&label, edge, fade, &envelope, transcript.as_ref()));
            }
        }
    }
    Ok(warnings)
}

fn edge_warnings(
    label: &str,
    edge: f64,
    fade: Option<f64>,
    envelope: &Envelope,
    transcript: Option<&Transcript>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let word = transcript
        .and_then(|transcript| transcript.words_between(edge - WORD_MARGIN_SECONDS, edge + WORD_MARGIN_SECONDS).next());
    if let Some(word) = word {
        warnings.push(format!(
            "{label} at {} lands mid-word: `{}` runs {} to {}",
            Time(edge),
            word.text,
            Time(word.start),
            Time(word.end)
        ));
    }

    let level = envelope.mean_db(edge - EDGE_WINDOW_SECONDS, edge + EDGE_WINDOW_SECONDS);
    let faded = fade.is_some_and(|fade| fade > EDGE_FADE_SECONDS);
    if level <= LOUD_EDGE_DB || faded {
        return warnings;
    }
    let mut message = format!("{label} at {} is in loud audio ({level:.0} dBFS); give it a longer fade", Time(edge));
    if let Some((start, end)) = nearest_quiet_span(envelope, edge) {
        message.push_str(&format!(" or move it to the quiet span {} to {}", Time(start), Time(end)));
    }
    warnings.push(message);
    warnings
}

fn nearest_quiet_span(envelope: &Envelope, edge: f64) -> Option<(f64, f64)> {
    let from = (edge - QUIET_SEARCH_SECONDS).max(0.0);
    envelope
        .quiet_spans(from, edge + QUIET_SEARCH_SECONDS, LOUD_EDGE_DB, QUIET_MIN_SECONDS)
        .into_iter()
        .min_by(|left, right| distance(left, edge).total_cmp(&distance(right, edge)))
}

fn distance(span: &(f64, f64), edge: f64) -> f64 {
    let (start, end) = *span;
    if edge < start {
        return start - edge;
    }
    (edge - end).max(0.0)
}
