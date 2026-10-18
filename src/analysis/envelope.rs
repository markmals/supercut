use std::path::Path;

use anyhow::Context as _;

/// Seconds covered by one envelope frame.
pub const FRAME_SECONDS: f64 = 0.01;

/// Level floor, in dBFS, for digital silence.
const FLOOR_DB: f32 = -100.0;

/// Absorbs floating-point error so times on a frame boundary map to that frame.
const ROUNDING: f64 = 1e-6;

/// RMS level of the mono mix in consecutive 10 ms frames, in dBFS.
pub struct Envelope {
    levels_db: Vec<f32>,
}

impl Envelope {
    /// Builds an envelope from per-frame mean squares.
    pub fn from_mean_squares(mean_squares: impl IntoIterator<Item = f64>) -> Self {
        let levels_db = mean_squares
            .into_iter()
            .map(|mean_square| {
                if mean_square <= 0.0 {
                    FLOOR_DB
                } else {
                    (10.0 * mean_square.log10()).max(f64::from(FLOOR_DB)) as f32
                }
            })
            .collect();
        Self { levels_db }
    }

    pub fn duration(&self) -> f64 {
        self.levels_db.len() as f64 * FRAME_SECONDS
    }

    pub fn levels_db(&self) -> &[f32] {
        &self.levels_db
    }

    pub fn frame_at(&self, time: f64) -> usize {
        ((time / FRAME_SECONDS + ROUNDING).floor().max(0.0) as usize)
            .min(self.levels_db.len().saturating_sub(1))
    }

    /// Frames covering `[start, end)`; at least one frame.
    fn frames_in(&self, start: f64, end: f64) -> std::ops::Range<usize> {
        let first = self.frame_at(start);
        let last = ((end / FRAME_SECONDS - ROUNDING).ceil().max(0.0) as usize)
            .clamp(first + 1, self.levels_db.len().max(first + 1));
        first..last
    }

    pub fn time_of(frame: usize) -> f64 {
        frame as f64 * FRAME_SECONDS
    }

    /// Level of the frame containing `time`.
    pub fn level_at(&self, time: f64) -> f32 {
        self.levels_db.get(self.frame_at(time)).copied().unwrap_or(FLOOR_DB)
    }

    /// Energy-averaged level over `[start, end)`.
    pub fn mean_db(&self, start: f64, end: f64) -> f32 {
        let frames = &self.levels_db[self.frames_in(start, end)];
        let energy: f64 = frames.iter().map(|db| 10f64.powf(f64::from(*db) / 10.0)).sum::<f64>()
            / frames.len() as f64;
        if energy <= 0.0 { FLOOR_DB } else { (10.0 * energy.log10()) as f32 }
    }

    /// Spans inside `[start, end)` that stay below `threshold_db` for at least `min_seconds`.
    pub fn quiet_spans(
        &self,
        start: f64,
        end: f64,
        threshold_db: f32,
        min_seconds: f64,
    ) -> Vec<(f64, f64)> {
        let frames = self.frames_in(start, end);
        let min_frames = (min_seconds / FRAME_SECONDS).ceil() as usize;
        let mut spans = Vec::new();
        let mut run_start = None;
        for frame in frames.clone().chain(std::iter::once(frames.end)) {
            let quiet = frame < frames.end && self.levels_db[frame] < threshold_db;
            match (quiet, run_start) {
                (true, None) => run_start = Some(frame),
                (false, Some(begin)) => {
                    if frame - begin >= min_frames {
                        spans.push((Self::time_of(begin), Self::time_of(frame)));
                    }
                    run_start = None;
                }
                _ => {}
            }
        }
        spans
    }

    /// Start of the first frame in `[start, end)` louder than `threshold_db`.
    pub fn first_above(&self, start: f64, end: f64, threshold_db: f32) -> Option<f64> {
        self.frames_in(start, end)
            .find(|&frame| self.levels_db[frame] > threshold_db)
            .map(Self::time_of)
    }

    /// End of the last frame in `[start, end)` louder than `threshold_db`.
    pub fn last_above(&self, start: f64, end: f64, threshold_db: f32) -> Option<f64> {
        self.frames_in(start, end)
            .rev()
            .find(|&frame| self.levels_db[frame] > threshold_db)
            .map(|frame| Self::time_of(frame + 1))
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let bytes: Vec<u8> = self.levels_db.iter().flat_map(|level| level.to_le_bytes()).collect();
        crate::library::write_atomically(path, &bytes)
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let levels_db =
            bytes.as_chunks::<4>().0.iter().map(|chunk| f32::from_le_bytes(*chunk)).collect();
        Ok(Self { levels_db })
    }
}

#[cfg(test)]
mod tests {
    use super::Envelope;

    #[test]
    fn quiet_spans_honor_threshold_and_minimum_length() {
        // 0.5 s loud, 0.3 s quiet, 0.2 s loud, 0.05 s quiet, 0.5 s loud, 0.4 s quiet to the end.
        let pattern = [(50, 0.1), (30, 1e-8), (20, 0.1), (5, 1e-8), (50, 0.1), (40, 1e-8)];
        let squares =
            pattern.iter().flat_map(|&(frames, square)| std::iter::repeat_n(square, frames));
        let envelope = Envelope::from_mean_squares(squares);
        let spans = envelope.quiet_spans(0.0, envelope.duration(), -50.0, 0.1);
        assert_eq!(spans.len(), 2, "{spans:?}");
        assert!((spans[0].0 - 0.5).abs() < 1e-9 && (spans[0].1 - 0.8).abs() < 1e-9, "{spans:?}");
        assert!((spans[1].0 - 1.55).abs() < 1e-9 && (spans[1].1 - 1.95).abs() < 1e-9, "{spans:?}");
    }
}
