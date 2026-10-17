use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Serialize;

use super::status::has_audio;
use super::text::normalize_title;
use super::{tags, walk};
use crate::error::{CliError, ErrorCode};
use crate::library::{Episode, EpisodeIndex, ShowLibrary};
use crate::media;

/// How far a file's length may sit from the feed's claim before it is the wrong recording.
const DURATION_TOLERANCE: f64 = 0.02;

#[derive(Default, Serialize)]
pub struct AdoptReport {
    pub adopted: Vec<Adopted>,
    /// Files whose title matches no episode that still wants audio.
    pub unmatched_files: Vec<String>,
    pub ambiguous: Vec<Ambiguous>,
    pub rejected: Vec<Rejected>,
}

#[derive(Serialize)]
pub struct Adopted {
    pub id: String,
    pub file: String,
    pub method: Method,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Method {
    HardLink,
    Copy,
}

/// A file and the episodes it could be: too many files or too many episodes share one title.
#[derive(Serialize)]
pub struct Ambiguous {
    pub file: String,
    pub ids: Vec<String>,
}

#[derive(Serialize)]
pub struct Rejected {
    pub file: String,
    pub id: String,
    pub reason: String,
}

/// Matches the audio files under `dir` to episodes that have none, by the title in each file's
/// own tags, and links the matches into the library. `dry_run` reports the same matches without
/// touching anything.
pub fn adopt(
    library: &ShowLibrary,
    index: &mut EpisodeIndex,
    dir: &Path,
    dry_run: bool,
) -> anyhow::Result<AdoptReport> {
    if !dir.is_dir() {
        let error = CliError::not_found(format!("{} is not a folder", dir.display()));
        return Err(error.detail("path", dir.display().to_string()).into());
    }
    let wanted = episodes_wanting_audio(library, index);
    let mut report = AdoptReport::default();
    let files = files_by_title(&walk::audio_files(dir)?, &mut report)?;

    for (title, files) in files {
        let positions = wanted.get(&title).map(Vec::as_slice).unwrap_or_default();
        if positions.is_empty() {
            report.unmatched_files.extend(files.iter().map(|file| shown(file)));
            continue;
        }
        if positions.len() > 1 || files.len() > 1 {
            report.ambiguous.extend(ambiguous(index, positions, &files));
            continue;
        }
        let (file, episode) = (&files[0], &mut index.episodes[positions[0]]);
        let id = episode.id.clone();
        match adopt_one(library, episode, file, dry_run)? {
            Ok(method) => report.adopted.push(Adopted { id, file: shown(file), method }),
            Err(reason) => report.rejected.push(Rejected { file: shown(file), id, reason }),
        }
    }
    Ok(report)
}

/// Where each episode still waiting for audio sits in the index, by normalized title.
fn episodes_wanting_audio(library: &ShowLibrary, index: &EpisodeIndex) -> BTreeMap<String, Vec<usize>> {
    let mut wanted: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (position, episode) in index.episodes.iter().enumerate() {
        if !has_audio(library, episode) {
            wanted.entry(normalize_title(&episode.title)).or_default().push(position);
        }
    }
    wanted
}

/// Files grouped by the normalized title in their tags. Untitled files cannot be matched, so
/// they go straight to the report.
fn files_by_title(files: &[PathBuf], report: &mut AdoptReport) -> anyhow::Result<BTreeMap<String, Vec<PathBuf>>> {
    let mut by_title: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for file in files {
        match tags::title(file)? {
            Some(title) => by_title.entry(normalize_title(&title)).or_default().push(file.clone()),
            None => report.unmatched_files.push(shown(file)),
        }
    }
    Ok(by_title)
}

fn ambiguous(index: &EpisodeIndex, positions: &[usize], files: &[PathBuf]) -> Vec<Ambiguous> {
    let ids: Vec<String> = positions.iter().map(|position| index.episodes[*position].id.clone()).collect();
    files.iter().map(|file| Ambiguous { file: shown(file), ids: ids.clone() }).collect()
}

/// Links one file in, unless it runs too far from the length the feed states; the `Err` side is
/// that refusal, not a failure.
fn adopt_one(
    library: &ShowLibrary,
    episode: &mut Episode,
    file: &Path,
    dry_run: bool,
) -> anyhow::Result<Result<Method, String>> {
    let duration = probe_duration(file)?;
    if let Some(reason) = wrong_length(duration, episode.feed_duration) {
        return Ok(Err(reason));
    }
    if dry_run {
        return Ok(Ok(Method::HardLink));
    }
    let name = format!("{}.{}", episode.id, extension(file));
    let audio_dir = library.audio_dir();
    std::fs::create_dir_all(&audio_dir).with_context(|| format!("creating {}", audio_dir.display()))?;
    let method = link_or_copy(file, &audio_dir.join(&name))?;
    episode.audio_file = Some(name);
    Ok(Ok(method))
}

fn probe_duration(file: &Path) -> anyhow::Result<f64> {
    media::probe(file)
        .map(|probe| probe.duration)
        .map_err(|error| CliError::new(ErrorCode::ExternalTool, format!("{error:#}")).into())
}

fn wrong_length(duration: f64, feed_duration: Option<f64>) -> Option<String> {
    let feed_duration = feed_duration?;
    let drift = (duration - feed_duration).abs() / feed_duration;
    if drift <= DURATION_TOLERANCE {
        return None;
    }
    Some(format!(
        "runs {duration:.0}s but the feed says {feed_duration:.0}s, {:.1}% off (limit {:.0}%)",
        drift * 100.0,
        DURATION_TOLERANCE * 100.0
    ))
}

/// A hard link costs no disk; across volumes there is nothing to link to, so the file is copied.
fn link_or_copy(source: &Path, destination: &Path) -> anyhow::Result<Method> {
    if let Err(error) = std::fs::remove_file(destination) {
        anyhow::ensure!(
            error.kind() == std::io::ErrorKind::NotFound,
            "clearing {}: {error}",
            destination.display()
        );
    }
    if std::fs::hard_link(source, destination).is_ok() {
        return Ok(Method::HardLink);
    }
    std::fs::copy(source, destination)
        .with_context(|| format!("copying {} to {}", source.display(), destination.display()))?;
    Ok(Method::Copy)
}

fn extension(file: &Path) -> String {
    file.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "mp3".to_owned())
}

fn shown(file: &Path) -> String {
    file.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_the_feed_length_disagrees_with_is_refused() {
        assert!(wrong_length(5400.0, Some(5400.0)).is_none());
        assert!(wrong_length(5400.0, None).is_none());
        assert!(wrong_length(5508.0, Some(5400.0)).is_none(), "2% drift is within tolerance");
        assert!(wrong_length(5600.0, Some(5400.0)).is_some());
        assert!(wrong_length(1200.0, Some(5400.0)).is_some());
    }
}
