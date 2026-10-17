//! Finding the theme song (or jingle) that opens an episode, from a recording of it. Shows play
//! the same recording every episode, so it lines up with the episode to the sample, wherever
//! the out-of-character introduction before it ends.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Serialize;

use super::align::best_lag;
use crate::error::CliError;
use crate::media::decode_window;

const RATE: u32 = 11_025;
/// How far into an episode its theme may end; a cold open can push it past three minutes.
const SEARCH_SECONDS: f64 = 300.0;
/// Longest theme recording read; a longer file is not a theme.
const MAX_THEME_SECONDS: f64 = 120.0;
/// Waveform agreement above which the theme plays. The same recording scores 0.97 to 0.99 in
/// another episode, a live or reworked version 0.25 to 0.5, unrelated audio near 0.
const MATCH_SCORE: f32 = 0.8;

#[derive(Clone, Debug, Serialize)]
pub struct ThemeMatch {
    pub theme: PathBuf,
    /// Where the recording starts and ends in the episode, in seconds.
    pub start: f64,
    pub end: f64,
    pub score: f32,
}

/// A show's theme recordings, decoded once for every episode they are looked for in.
pub struct Themes {
    recordings: Vec<(PathBuf, Vec<f32>)>,
}

impl Themes {
    pub fn load(paths: &[PathBuf]) -> anyhow::Result<Self> {
        let mut recordings = Vec::with_capacity(paths.len());
        for path in paths {
            if !path.is_file() {
                let missing = CliError::precondition(format!("theme recording {} does not exist", path.display()));
                return Err(missing.hint("supercut config validate").into());
            }
            let samples = decode_window(path, 0.0, MAX_THEME_SECONDS, RATE, 1)
                .with_context(|| format!("reading theme recording {}", path.display()))?;
            recordings.push((path.clone(), samples));
        }
        Ok(Self { recordings })
    }

    /// The theme that plays in the episode's opening; when several do (a jingle before the theme
    /// proper), the one that ends last.
    pub fn find_in(&self, audio: &Path) -> anyhow::Result<Option<ThemeMatch>> {
        if self.recordings.is_empty() {
            return Ok(None);
        }
        let opening = decode_window(audio, 0.0, SEARCH_SECONDS, RATE, 1)?;
        let found = self
            .recordings
            .iter()
            .filter_map(|(path, recording)| locate(path, recording, &opening))
            .max_by(|a, b| a.end.total_cmp(&b.end));
        Ok(found)
    }
}

fn locate(path: &Path, recording: &[f32], opening: &[f32]) -> Option<ThemeMatch> {
    let max_lag = opening.len().checked_sub(recording.len())?;
    let (lag, score) = best_lag(recording, opening, max_lag)?;
    let start = lag as f64 / f64::from(RATE);
    (score >= MATCH_SCORE).then(|| ThemeMatch {
        theme: path.to_path_buf(),
        start,
        end: start + recording.len() as f64 / f64::from(RATE),
        score,
    })
}
