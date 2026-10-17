use std::path::Path;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use super::refine::{self, FRAME, FrameMatch};
use crate::media::decode_window;

const RATE: u32 = 11_025;
const FRAME_SECONDS: f64 = FRAME as f64 / RATE as f64;
/// How far the exact offset may sit from the fingerprint's estimate.
const OFFSET_SEARCH_SECONDS: f64 = 0.6;
/// Audio examined on each side of a fingerprint edge.
const EDGE_OUTSIDE_SECONDS: f64 = 2.5;
const EDGE_INSIDE_SECONDS: f64 = 2.5;

/// A stretch of the export that plays one stretch of a source episode, timed to the sample.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Piece {
    pub episode: String,
    pub export_start: f64,
    pub export_end: f64,
    pub source_start: f64,
    pub source_end: f64,
    /// Export level relative to the source over the steady part, in dB.
    pub gain_db: f32,
    /// Seconds over which the export fades this stretch in and out.
    pub fade_in: f64,
    pub fade_out: f64,
    /// Normalized correlation at the exact offset (1 = identical waveform).
    pub correlation: f32,
}

impl Piece {
    pub fn offset(&self) -> f64 {
        self.export_start - self.source_start
    }

    pub fn duration(&self) -> f64 {
        self.export_end - self.export_start
    }
}

/// A coarse match, in seconds, from the fingerprints.
pub struct Estimate<'a> {
    pub episode: &'a str,
    pub export_start: f64,
    pub export_end: f64,
    pub source_start: f64,
}

pub fn refine_piece(export: &Path, source: &Path, estimate: &Estimate) -> anyhow::Result<Piece> {
    let (offset, correlation, steady_gain) = exact_offset(export, source, estimate)
        .with_context(|| format!("aligning {} at {:.1}s of the export", estimate.episode, estimate.export_start))?;
    let gain = steady_gain.unwrap_or(1.0);
    let (export_start, fade_in) = edge(export, source, offset, estimate.export_start, -1, gain)?;
    let (export_end, fade_out) = edge(export, source, offset, estimate.export_end, 1, gain)?;
    Ok(Piece {
        episode: estimate.episode.to_owned(),
        export_start,
        export_end,
        source_start: export_start - offset,
        source_end: export_end - offset,
        gain_db: 20.0 * gain.log10(),
        fade_in,
        fade_out,
        correlation,
    })
}

/// `export time - source time` to the sample, the waveform correlation there, and the steady
/// export/source gain, measured in the middle of the estimate.
fn exact_offset(export: &Path, source: &Path, estimate: &Estimate) -> anyhow::Result<(f64, f32, Option<f32>)> {
    let coarse_offset = estimate.export_start - estimate.source_start;
    let middle = (estimate.export_start + estimate.export_end) / 2.0;
    let half = ((estimate.export_end - estimate.export_start) / 2.0 - 0.3).clamp(0.5, 3.0);
    let export_window_start = middle - half;
    let source_window_start = (export_window_start - coarse_offset - OFFSET_SEARCH_SECONDS).max(0.0);
    let export_samples = decode_window(export, export_window_start, 2.0 * half, RATE, 1)?;
    let source_samples = decode_window(source, source_window_start, 2.0 * (half + OFFSET_SEARCH_SECONDS), RATE, 1)?;
    let max_lag = source_samples.len().saturating_sub(export_samples.len());
    let (lag, correlation) = crate::analysis::align::best_lag(&export_samples, &source_samples, max_lag)
        .context("windows too short to align")?;
    let aligned = &source_samples[lag..lag + export_samples.len()];
    let gain = refine::steady_gain(&refine::compare_frames(&export_samples, aligned));
    let offset = export_window_start - (source_window_start + lag as f64 / f64::from(RATE));
    Ok((offset, correlation, gain))
}

/// Where the match ends near `estimate` (seconds in the export) and the fade length there.
/// `outward` is -1 for a start edge, +1 for an end edge.
fn edge(export: &Path, source: &Path, offset: f64, estimate: f64, outward: isize, gain: f32) -> anyhow::Result<(f64, f64)> {
    let (before, after) =
        if outward < 0 { (EDGE_OUTSIDE_SECONDS, EDGE_INSIDE_SECONDS) } else { (EDGE_INSIDE_SECONDS, EDGE_OUTSIDE_SECONDS) };
    let window_start = (estimate - before).max(offset.max(0.0));
    let length = estimate + after - window_start;
    let export_samples = decode_window(export, window_start, length, RATE, 1)?;
    let source_samples = decode_window(source, window_start - offset, length, RATE, 1)?;
    let frames = refine::compare_frames(&export_samples, &source_samples);
    let Some(inner) = inner_matched_frame(&frames, ((estimate - window_start) / FRAME_SECONDS) as usize, outward) else {
        return Ok((estimate, 0.0));
    };
    let edge_frame = refine::match_edge(&frames, inner, outward);
    let fade = refine::fade_frames(&frames, edge_frame, -outward, gain);
    Ok((window_start + edge_frame as f64 * FRAME_SECONDS, fade as f64 * FRAME_SECONDS))
}

/// A matching frame on the inside of the estimated edge to walk outwards from.
fn inner_matched_frame(frames: &[FrameMatch], estimate: usize, outward: isize) -> Option<usize> {
    let inside: Box<dyn Iterator<Item = usize>> =
        if outward < 0 { Box::new(estimate.min(frames.len())..frames.len()) } else { Box::new((0..estimate.min(frames.len())).rev()) };
    let mut inside = inside.peekable();
    let first = *inside.peek()?;
    let found = inside.find(|&index| matches!(frames[index], FrameMatch::Matched { .. }));
    found.or(Some(first)).filter(|&index| matches!(frames[index], FrameMatch::Matched { .. }))
}
