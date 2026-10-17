use std::path::PathBuf;

/// Seconds a preview marker lasts: silence, a short quiet beep, silence.
pub const BEEP_SECONDS: f64 = 0.7;

/// One stretch of audio in a rendered file: what to decode, how loud to play it and when.
#[derive(Clone, Debug)]
pub struct Piece {
    pub source: Source,
    /// Applied before the fades, in dB.
    pub gain_db: f64,
    pub fade_in: f64,
    pub fade_out: f64,
    /// When the piece starts in the rendered file, in seconds.
    pub output_start: f64,
}

#[derive(Clone, Debug)]
pub enum Source {
    /// `[start, end)` of an audio file.
    File { path: PathBuf, start: f64, end: f64 },
    /// Marker generated between preview transitions.
    Beep,
}

impl Source {
    pub fn seconds(&self) -> f64 {
        match self {
            Source::File { start, end, .. } => (end - start).max(0.0),
            Source::Beep => BEEP_SECONDS,
        }
    }

    pub fn file(path: impl Into<PathBuf>, start: f64, end: f64) -> Self {
        Source::File { path: path.into(), start, end }
    }
}

impl Piece {
    pub fn new(source: Source, gain_db: f64, output_start: f64) -> Self {
        Self { source, gain_db, fade_in: 0.0, fade_out: 0.0, output_start }
    }

    /// Fades, never longer than the audio they shape.
    pub fn faded(mut self, fade_in: f64, fade_out: f64) -> Self {
        let seconds = self.seconds();
        self.fade_in = fade_in.clamp(0.0, seconds);
        self.fade_out = fade_out.clamp(0.0, seconds - self.fade_in);
        self
    }

    pub fn seconds(&self) -> f64 {
        self.source.seconds()
    }

    pub fn output_end(&self) -> f64 {
        self.output_start + self.seconds()
    }
}
