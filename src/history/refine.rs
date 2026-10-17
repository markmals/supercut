//! Sample-level comparison of two recordings already aligned to the sample (see
//! `analysis::align`): where they stop matching, and how the export's gain moves against the
//! source's (fades).

pub use crate::analysis::align::FRAME;
use crate::analysis::align::{correlation, rms};

/// Below this RMS (about -55 dBFS) a frame carries too little signal to compare.
const SILENT_RMS: f32 = 0.0018;
/// Frames whose normalized correlation reaches this play the same audio.
const MATCH_CORRELATION: f32 = 0.7;
/// Consecutive non-matching frames that end a matching stretch.
const MISMATCH_FRAMES: usize = 3;
/// A fade has ended once the gain stays within 1 dB of the stretch's own gain this long.
const STEADY_FRAMES: usize = 5;
const STEADY_RATIO: f32 = 0.89;
/// Only frames this loud in the source give a trustworthy gain ratio (about -40 dBFS).
const GAIN_RMS: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameMatch {
    Matched { gain: f32 },
    Silent,
    Different,
}

/// Frame-by-frame comparison of two already aligned, equally long recordings.
pub fn compare_frames(export: &[f32], source: &[f32]) -> Vec<FrameMatch> {
    export
        .as_chunks::<FRAME>()
        .0
        .iter()
        .zip(source.as_chunks::<FRAME>().0)
        .map(|(x, y)| {
            let (level_x, level_y) = (rms(x), rms(y));
            if level_x < SILENT_RMS && level_y < SILENT_RMS {
                return FrameMatch::Silent;
            }
            if level_x >= SILENT_RMS && level_y >= SILENT_RMS && correlation(x, y) >= MATCH_CORRELATION {
                return FrameMatch::Matched { gain: if level_y >= GAIN_RMS { level_x / level_y } else { f32::NAN } };
            }
            FrameMatch::Different
        })
        .collect()
}

/// Walking from `inner` (a frame known to match) towards `outward` (-1 or +1), the frame index
/// one past the last matching frame before the audio stops matching.
pub fn match_edge(frames: &[FrameMatch], inner: usize, outward: isize) -> usize {
    let mut last_matched = inner;
    let mut misses = 0;
    let mut index = inner as isize;
    while index >= 0 && (index as usize) < frames.len() {
        match frames[index as usize] {
            FrameMatch::Matched { .. } => {
                last_matched = index as usize;
                misses = 0;
            }
            FrameMatch::Different => {
                misses += 1;
                if misses >= MISMATCH_FRAMES {
                    break;
                }
            }
            FrameMatch::Silent => {}
        }
        index += outward;
    }
    if outward < 0 { last_matched } else { last_matched + 1 }
}

/// Frames from the edge through the last frame the export plays quieter than `steady_gain`
/// before the gain settles: the fade length. Silence right at the edge is not a fade. `edge` is
/// the first frame inside the stretch (fade-in) or one past its last frame (fade-out, walked
/// backwards).
pub fn fade_frames(frames: &[FrameMatch], edge: usize, inward: isize, steady_gain: f32) -> usize {
    let mut steady_run = 0;
    let mut fade = 0;
    let mut index = if inward > 0 { edge as isize } else { edge as isize - 1 };
    let mut walked = 0;
    while index >= 0 && (index as usize) < frames.len() && steady_run < STEADY_FRAMES {
        if let FrameMatch::Matched { gain } = frames[index as usize]
            && gain.is_finite()
        {
            if gain >= steady_gain * STEADY_RATIO {
                steady_run += 1;
            } else {
                steady_run = 0;
                fade = walked + 1;
            }
        }
        walked += 1;
        index += inward;
    }
    fade
}

/// The typical export/source gain over a stretch of matching frames.
pub fn steady_gain(frames: &[FrameMatch]) -> Option<f32> {
    let mut gains: Vec<f32> = frames
        .iter()
        .filter_map(|frame| match frame {
            FrameMatch::Matched { gain } if gain.is_finite() => Some(*gain),
            _ => None,
        })
        .collect();
    if gains.is_empty() {
        return None;
    }
    gains.sort_by(f32::total_cmp);
    Some(gains[gains.len() / 2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::align::tests::signal;

    #[test]
    fn locates_a_cut_and_measures_its_fade_out() {
        // 1 s of shared audio whose last 0.3 s fades to silence, then 0.5 s of unrelated audio.
        let source = signal(11, 16_500);
        let mut export: Vec<f32> = source[..11_000].to_vec();
        let fade_start = 11_000 - 3_300;
        for (offset, sample) in export[fade_start..].iter_mut().enumerate() {
            *sample *= 1.0 - offset as f32 / 3_300.0;
        }
        export.extend(signal(99, 5_500));

        let frames = compare_frames(&export, &source);
        let edge = match_edge(&frames, 10, 1);
        assert!(edge.abs_diff(100) <= 2, "edge frame {edge}");
        let gain = steady_gain(&frames[..60]).unwrap();
        let fade = fade_frames(&frames, edge, -1, gain);
        assert!(fade.abs_diff(30) <= 4, "fade frames {fade}");
    }

    #[test]
    fn silence_at_a_hard_cut_is_not_a_fade() {
        // Both recordings open with 0.5 s of silence, then play the same audio at a 6 dB lower
        // level in the export: the edge has no fade even though the steady gain arrives late.
        let mut source = vec![0.0f32; 5_500];
        source.extend(signal(5, 11_000));
        let export: Vec<f32> = source.iter().map(|sample| sample * 0.5).collect();
        let frames = compare_frames(&export, &source);
        let gain = steady_gain(&frames).unwrap();
        assert!((gain - 0.5).abs() < 0.01, "{gain}");
        assert_eq!(fade_frames(&frames, 0, 1, gain), 0);
    }
}
