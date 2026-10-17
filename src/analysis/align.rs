//! Finding where one recording plays inside another, to the sample: decoded mono audio at a
//! common rate in, the lag and how well the waveforms agree there out.

/// Samples per analysis frame (10 ms at 11 025 Hz).
pub const FRAME: usize = 110;
/// Coarse peaks examined at sample resolution: the envelope alone can prefer a look-alike.
const COARSE_CANDIDATES: usize = 4;

/// Lag, in samples, at which `haystack[lag..]` best lines up with `needle`, searching
/// `0..=max_lag`, and the normalized correlation of the waveforms there (1 = identical).
pub fn best_lag(needle: &[f32], haystack: &[f32], max_lag: usize) -> Option<(usize, f32)> {
    coarse_lags(needle, haystack, max_lag)
        .into_iter()
        .filter_map(|coarse| {
            let low = coarse.saturating_sub(2 * FRAME);
            let high = (coarse + 2 * FRAME).min(max_lag);
            (low..=high)
                .filter(|lag| lag + needle.len() <= haystack.len())
                .map(|lag| (lag, correlation(needle, &haystack[lag..lag + needle.len()])))
                .max_by(|a, b| a.1.total_cmp(&b.1))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// The frame-resolution lags whose RMS envelopes agree best, strongest first.
fn coarse_lags(needle: &[f32], haystack: &[f32], max_lag: usize) -> Vec<usize> {
    let needle_envelope = envelope(needle);
    let haystack_envelope = envelope(haystack);
    let frames = needle_envelope.len();
    let scores: Vec<f32> = (0..=max_lag / FRAME)
        .take_while(|shift| shift + frames <= haystack_envelope.len())
        .map(|shift| correlation(&needle_envelope, &haystack_envelope[shift..shift + frames]))
        .collect();
    let mut peaks: Vec<(usize, f32)> = (0..scores.len())
        .filter(|&shift| {
            let before = shift.checked_sub(1).map_or(f32::MIN, |previous| scores[previous]);
            let after = scores.get(shift + 1).copied().unwrap_or(f32::MIN);
            scores[shift] >= before && scores[shift] >= after
        })
        .map(|shift| (shift, scores[shift]))
        .collect();
    peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
    peaks.into_iter().take(COARSE_CANDIDATES).map(|(shift, _)| shift * FRAME).collect()
}

fn envelope(samples: &[f32]) -> Vec<f32> {
    samples.as_chunks::<FRAME>().0.iter().map(|frame| rms(frame)).collect()
}

pub fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

/// Pearson correlation of two equally long slices.
pub fn correlation(a: &[f32], b: &[f32]) -> f32 {
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

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Noise-like test signal: deterministic and uncorrelated with itself at any lag.
    pub fn signal(seed: u32, len: usize) -> Vec<f32> {
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
        let haystack = signal(7, 30_000);
        let needle: Vec<f32> = haystack[4_321..4_321 + 11_025].iter().map(|sample| sample * 0.5).collect();
        let (lag, score) = best_lag(&needle, &haystack, 8_000).unwrap();
        assert_eq!(lag, 4_321);
        assert!(score > 0.99, "{score}");
    }

    #[test]
    fn a_look_alike_envelope_does_not_hide_the_real_match() {
        // Earlier in the haystack, other audio with the same loudness contour as the needle.
        let needle = signal(3, 11_000);
        let mut haystack: Vec<f32> = signal(4, 11_000)
            .iter()
            .zip(needle.as_chunks::<FRAME>().0.iter().flatten())
            .map(|(noise, shape)| noise.signum() * shape.abs())
            .collect();
        haystack.extend(signal(5, 3_000));
        haystack.extend(&needle);
        let (lag, score) = best_lag(&needle, &haystack, haystack.len() - needle.len()).unwrap();
        assert_eq!(lag, 14_000);
        assert!(score > 0.99, "{score}");
    }
}
