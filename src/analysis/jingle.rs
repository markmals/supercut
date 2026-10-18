//! Finding a show's recurring recordings in an episode: the theme song (or jingle) that opens
//! it and the bumper that closes it. Shows play the same recording every episode, so it lines
//! up with the episode to the sample.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Serialize;

use super::align::best_lag;
use crate::error::CliError;
use crate::media::decode_window;

const RATE: u32 = 11_025;
/// How far into an episode its theme may end; a cold open can push it past three minutes.
const OPENING_SECONDS: f64 = 300.0;
/// How much of an episode's end is searched for a closing bumper, at least: a longer bumper
/// widens the search to fit it.
const BUMPER_SECONDS: f64 = 30.0;
/// How much of an episode's end is searched for its outro: the credits and the plug before them
/// fit in well under this.
const OUTRO_SECONDS: f64 = 180.0;
/// Some files stop a few hundredths of a second before their bumper does; the search runs on
/// past the end in silence so a clipped bumper still lines up.
const PAST_THE_END_SECONDS: f64 = 0.5;
/// A bumper that ends further than this from the end of the file does not close the episode.
const CLOSES_WITHIN_SECONDS: f64 = 1.0;
/// Longest recording read; a longer file is not a jingle.
const MAX_RECORDING_SECONDS: f64 = 120.0;
/// Waveform agreement above which the recording plays. The same recording scores 0.97 to 0.99
/// in another episode, a live or reworked version 0.25 to 0.5, unrelated audio near 0.
const MATCH_SCORE: f32 = 0.8;

#[derive(Clone, Debug, Serialize)]
pub struct JingleMatch {
    pub recording: PathBuf,
    /// Where the recording starts and ends in the episode, in seconds.
    pub start: f64,
    pub end: f64,
    pub score: f32,
}

/// A show's recordings of one kind, decoded once for every episode they are looked for in.
pub struct Jingles {
    recordings: Vec<(PathBuf, Vec<f32>)>,
}

impl Jingles {
    pub fn load(paths: &[PathBuf]) -> anyhow::Result<Self> {
        let mut recordings = Vec::with_capacity(paths.len());
        for path in paths {
            if !path.is_file() {
                let missing =
                    CliError::precondition(format!("recording {} does not exist", path.display()));
                return Err(missing.hint("supercut config validate").into());
            }
            let samples = decode_window(path, 0.0, MAX_RECORDING_SECONDS, RATE, 1)
                .with_context(|| format!("reading recording {}", path.display()))?;
            recordings.push((path.clone(), samples));
        }
        Ok(Self { recordings })
    }

    pub fn is_empty(&self) -> bool {
        self.recordings.is_empty()
    }

    /// The recording that plays in the episode's opening; when several do (a jingle before the
    /// theme proper), the one that ends last.
    pub fn opening_of(&self, audio: &Path) -> anyhow::Result<Option<JingleMatch>> {
        self.search(audio, 0.0, OPENING_SECONDS, |a, b| a.end.total_cmp(&b.end))
    }

    /// The recording the episode ends with: one that runs out at the end of the file. When
    /// several play near the end, one that closes it wins over a better match that does not.
    pub fn ending_of(&self, audio: &Path, duration: f64) -> anyhow::Result<Option<JingleMatch>> {
        let closes = |found: &JingleMatch| (duration - found.end).abs() <= CLOSES_WITHIN_SECONDS;
        let reach = BUMPER_SECONDS.max(self.longest_seconds() + CLOSES_WITHIN_SECONDS);
        let found = self.search(audio, duration - reach, duration, |a, b| {
            closes(a).cmp(&closes(b)).then(a.score.total_cmp(&b.score))
        })?;
        Ok(found.filter(closes).map(|found| JingleMatch { end: found.end.min(duration), ..found }))
    }

    /// How long the longest of the recordings plays.
    fn longest_seconds(&self) -> f64 {
        let longest = self.recordings.iter().map(|(_, samples)| samples.len()).max();
        longest.unwrap_or(0) as f64 / f64::from(RATE)
    }

    /// The recording that plays in the episode's last minutes, wherever it falls there; when
    /// several do (spoken credits, then closing music), the one that starts first.
    pub fn outro_of(&self, audio: &Path, duration: f64) -> anyhow::Result<Option<JingleMatch>> {
        self.search(audio, duration - OUTRO_SECONDS, duration, |a, b| b.start.total_cmp(&a.start))
    }

    /// The preferred match among the recordings playing in `[from, to)` of the episode.
    fn search(
        &self,
        audio: &Path,
        from: f64,
        to: f64,
        prefer: impl Fn(&JingleMatch, &JingleMatch) -> std::cmp::Ordering,
    ) -> anyhow::Result<Option<JingleMatch>> {
        if self.is_empty() {
            return Ok(None);
        }
        let from = from.max(0.0);
        let mut window = decode_window(audio, from, to - from, RATE, 1)?;
        window.resize(window.len() + (PAST_THE_END_SECONDS * f64::from(RATE)) as usize, 0.0);
        let found = self
            .recordings
            .iter()
            .filter_map(|(path, recording)| locate(path, recording, &window, from));
        Ok(found.max_by(prefer))
    }
}

fn locate(path: &Path, recording: &[f32], window: &[f32], offset: f64) -> Option<JingleMatch> {
    let max_lag = window.len().checked_sub(recording.len())?;
    let (lag, score) = best_lag(recording, window, max_lag)?;
    let start = offset + lag as f64 / f64::from(RATE);
    (score >= MATCH_SCORE).then(|| JingleMatch {
        recording: path.to_path_buf(),
        start,
        end: start + recording.len() as f64 / f64::from(RATE),
        score,
    })
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    /// Writes the output of `graph`, an lavfi filter graph, to `path`.
    fn render(graph: &str, path: &Path) {
        let status = Command::new("ffmpeg")
            .args(["-nostdin", "-hide_banner", "-v", "error", "-f", "lavfi", "-i", graph, "-y"])
            .arg(path)
            .status()
            .expect("running ffmpeg");
        assert!(status.success(), "ffmpeg could not write {}", path.display());
    }

    /// Noise that comes out the same for the same seed: a stand-in for a recording.
    fn noise(seconds: f64, seed: u32) -> String {
        format!("anoisesrc=d={seconds}:seed={seed}:r={RATE}:a=0.3")
    }

    #[test]
    fn a_bumper_longer_than_the_usual_search_is_found_closing_the_episode() {
        let folder = tempfile::tempdir().unwrap();
        let (bumper, episode) = (folder.path().join("bumper.wav"), folder.path().join("ep.wav"));
        render(&noise(35.0, 1), &bumper);
        let (talk, closing) = (noise(40.0, 2), noise(35.0, 1));
        render(&format!("{talk}[a];{closing}[b];[a][b]concat=n=2:v=0:a=1[out0]"), &episode);
        let found = Jingles::load(&[bumper]).unwrap().ending_of(&episode, 75.0).unwrap();
        let found = found.expect("the bumper closes the episode");
        assert!((found.start - 40.0).abs() < 0.01, "{found:?}");
    }

    #[test]
    fn the_bumper_that_closes_the_episode_beats_a_better_match_before_it() {
        let folder = tempfile::tempdir().unwrap();
        let [sting, bumper, episode] =
            ["sting", "bumper", "ep"].map(|name| folder.path().join(name).with_extension("wav"));
        render(&noise(3.0, 3), &sting);
        render(&noise(8.0, 1), &bumper);
        // Talk, the sting untouched, a pause, then the bumper under a little hiss to the end.
        let parts = [noise(40.0, 2), noise(3.0, 3), noise(1.0, 4), noise(8.0, 1), noise(8.0, 5)];
        let graph = format!(
            "{}[talk];{}[sting];{}[pause];{}[bumper];{}[hiss];[hiss]volume=0.3[soft];\
             [bumper][soft]amix=inputs=2:normalize=0[closing];\
             [talk][sting][pause][closing]concat=n=4:v=0:a=1[out0]",
            parts[0], parts[1], parts[2], parts[3], parts[4]
        );
        render(&graph, &episode);
        let jingles = Jingles::load(&[sting, bumper.clone()]).unwrap();
        let found = jingles.ending_of(&episode, 52.0).unwrap().expect("the bumper closes it");
        assert_eq!(found.recording, bumper);
        assert!((found.start - 44.0).abs() < 0.01, "{found:?}");
    }
}
