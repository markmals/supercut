//! Sample-level alignment math on decoded mono audio at a common rate: the exact offset
//! between two recordings of the same audio, where they stop matching, and how the export's
//! gain moves against the source's (fades).

/// Samples per analysis frame (10 ms at 11 025 Hz).
pub const FRAME: usize = 110;
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

/// Lag, in samples, at which `source[lag..]` best lines up with `export`, searching
/// `0..=max_lag`, and the normalized correlation there.
pub fn best_lag(export: &[f32], source: &[f32], max_lag: usize) -> Option<(usize, f32)> {
    let coarse = coarse_lag(export, source, max_lag)?;
    let low = coarse.saturating_sub(2 * FRAME);
    let high = (coarse + 2 * FRAME).min(max_lag);
    (low..=high)
        .filter(|lag| lag + export.len() <= source.len())
        .map(|lag| (lag, correlation(export, &source[lag..lag + export.len()])))
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// Frame-resolution lag from the RMS envelopes, cheap enough to search widely.
fn coarse_lag(export: &[f32], source: &[f32], max_lag: usize) -> Option<usize> {
    let export_envelope = envelope(export);
    let source_envelope = envelope(source);
    let frames = export_envelope.len();
    (0..=max_lag / FRAME)
        .filter(|shift| shift + frames <= source_envelope.len())
        .map(|shift| (shift, correlation(&export_envelope, &source_envelope[shift..shift + frames])))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(shift, _)| shift * FRAME)
}

fn envelope(samples: &[f32]) -> Vec<f32> {
    samples.chunks_exact(FRAME).map(rms).collect()
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

/// Pearson correlation of two equally long slices.
fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len()) as f32;
    let (mean_a, mean_b) = (a.iter().sum::<f32>() / n, b.iter().sum::<f32>() / n);
    let (mut cross, mut energy_a, mut energy_b) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b) {
        let (dx, dy) = (x - mean_a, y - mean_b);
        cross += dx * dy;
        energy_a += dx * dx;
        energy_b += dy * dy;
    }
    if energy_a <= 0.0 || energy_b <= 0.0 { 0.0 } else { cross / (energy_a * energy_b).sqrt() }
}

/// Frame-by-frame comparison of two already aligned, equally long recordings.
pub fn compare_frames(export: &[f32], source: &[f32]) -> Vec<FrameMatch> {
    export
        .chunks_exact(FRAME)
        .zip(source.chunks_exact(FRAME))
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

    /// Noise-like test signal: deterministic and uncorrelated with itself at any lag.
    fn signal(seed: u32, len: usize) -> Vec<f32> {
        let mut state = seed;
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state as f32 / u32::MAX as f32 - 0.5) * 0.5
            })
            .collect()
    }

    #[test]
    fn finds_the_exact_sample_lag_at_a_different_gain() {
        let source = signal(7, 30_000);
        let export: Vec<f32> = source[4_321..4_321 + 11_025].iter().map(|sample| sample * 0.5).collect();
        let (lag, score) = best_lag(&export, &source, 8_000).unwrap();
        assert_eq!(lag, 4_321);
        assert!(score > 0.99, "{score}");
    }

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
