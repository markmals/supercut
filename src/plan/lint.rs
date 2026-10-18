use crate::analysis::{self, Envelope, FRAME_SECONDS};
use crate::plan::EDGE_FADE_SECONDS;
use crate::plan::resolve::Resolved;
use crate::timecode::Time;
use crate::transcript::Transcript;

/// Word timings are this loose; an edge closer than this to a word's own edge does not cut it.
const WORD_MARGIN_SECONDS: f64 = 0.02;
/// Audio on the kept side of an edge that decides whether the splice is audible.
const EDGE_WINDOW_SECONDS: f64 = 0.02;
/// Above this level the edge is in the middle of something, not between things.
const LOUD_EDGE_DB: f32 = -40.0;
/// How far from an edge a better, quieter place to cut is worth suggesting.
const QUIET_SEARCH_SECONDS: f64 = 5.0;
/// An edge this close to where silence begins (or ends, on the kept side) only clips a tail.
const QUIET_LIP_SECONDS: f64 = 0.05;
/// A gap shorter than this is a breath, not a seam.
const QUIET_MIN_SECONDS: f64 = 0.25;

/// Which side of an edge the supercut keeps: a cut's start keeps the audio before it, a cut's
/// end the audio after it.
#[derive(Clone, Copy)]
enum Kept {
    Before,
    After,
}

/// Warnings about where the cuts land: inside a word, or on kept audio that is still sounding.
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
            let edges = [
                (format!("{label}: the start"), cut.start.seconds(), Kept::Before, cut.fade_out),
                (format!("{label}: the end"), end, Kept::After, cut.fade_in),
            ];
            // A cut from the top of the file or to its end joins nothing there: no splice to lint.
            let splices =
                edges.into_iter().filter(|(_, edge, _, _)| *edge > 0.0 && *edge < planned.duration);
            for (label, edge, kept, fade) in splices {
                let edge = Edge { label: &label, time: edge, kept, fade };
                warnings.extend(mid_word(&edge, transcript.as_ref()));
                warnings.extend(still_sounding(&edge, &envelope));
            }
        }
    }
    Ok(warnings)
}

struct Edge<'a> {
    label: &'a str,
    time: f64,
    kept: Kept,
    fade: Option<f64>,
}

fn mid_word(edge: &Edge, transcript: Option<&Transcript>) -> Option<String> {
    let word = transcript?.words_between(edge.time, edge.time).find(|word| {
        word.start < edge.time - WORD_MARGIN_SECONDS && word.end > edge.time + WORD_MARGIN_SECONDS
    })?;
    Some(format!(
        "{} at {} cuts through a word: `{}` runs {} to {}",
        edge.label,
        Time(edge.time),
        word.text,
        Time(word.start),
        Time(word.end)
    ))
}

/// Kept audio still sounding at the edge stops abruptly unless it fades. An edge at the lip of
/// a quiet span only shaves the decay of a sound, which the edge fade covers.
fn still_sounding(edge: &Edge, envelope: &Envelope) -> Option<String> {
    let from = match edge.kept {
        Kept::Before => edge.time - EDGE_WINDOW_SECONDS,
        Kept::After => edge.time,
    };
    let level = envelope.mean_db(from, from + EDGE_WINDOW_SECONDS.max(FRAME_SECONDS));
    if level <= LOUD_EDGE_DB || edge.fade.is_some_and(|fade| fade > EDGE_FADE_SECONDS) {
        return None;
    }
    let quiet = nearest_quiet_span(envelope, edge.time);
    let at_lip = quiet.is_some_and(|(start, end)| match edge.kept {
        Kept::Before => (start - edge.time).abs() <= QUIET_LIP_SECONDS,
        Kept::After => (edge.time - end).abs() <= QUIET_LIP_SECONDS,
    });
    if at_lip {
        return None;
    }
    let mut message = format!(
        "{} at {} is on audio still sounding ({level:.0} dBFS); give it a longer fade",
        edge.label,
        Time(edge.time)
    );
    if let Some((start, end)) = quiet {
        message.push_str(&format!(
            " or move it into the quiet span {} to {}",
            Time(start),
            Time(end)
        ));
    }
    Some(message)
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
