use std::path::PathBuf;

use serde::Serialize;

use super::fetch::redacted;
use crate::library::{Episode, EpisodeIndex, EpisodeKind, ShowLibrary};

/// One line of `episodes list`: what the feed says, plus how far the episode has come.
#[derive(Serialize)]
pub struct EpisodeStatus {
    pub id: String,
    pub title: String,
    /// Release date, `YYYY-MM-DD`.
    pub published: String,
    pub season: Option<u32>,
    pub number: Option<u32>,
    pub kind: EpisodeKind,
    pub duration_seconds: Option<f64>,
    pub downloaded: bool,
    pub transcribed: bool,
    pub labeled: bool,
}

/// Everything known about one episode, including where its files are and whether they exist.
#[derive(Serialize)]
pub struct EpisodeDetail {
    #[serde(flatten)]
    pub status: EpisodeStatus,
    pub description: String,
    pub guid: String,
    /// Without its query string: private feeds keep an auth token there.
    pub enclosure_url: String,
    pub image_url: Option<String>,
    pub files: Files,
}

#[derive(Serialize)]
pub struct Files {
    /// Missing until the episode is downloaded or adopted.
    pub audio: Option<FileState>,
    pub transcript: FileState,
    pub labels: FileState,
    pub analysis: FileState,
}

#[derive(Serialize)]
pub struct FileState {
    pub path: PathBuf,
    pub exists: bool,
}

/// How many episodes of a show have reached each stage.
#[derive(Default, Serialize)]
pub struct Counts {
    pub episodes: usize,
    pub downloaded: usize,
    pub transcribed: usize,
    pub labeled: usize,
}

pub fn statuses(library: &ShowLibrary, episodes: &[&Episode]) -> Vec<EpisodeStatus> {
    episodes.iter().map(|episode| status(library, episode)).collect()
}

pub fn counts(library: &ShowLibrary, index: &EpisodeIndex) -> Counts {
    let mut counts = Counts { episodes: index.episodes.len(), ..Counts::default() };
    for episode in &index.episodes {
        let status = status(library, episode);
        counts.downloaded += usize::from(status.downloaded);
        counts.transcribed += usize::from(status.transcribed);
        counts.labeled += usize::from(status.labeled);
    }
    counts
}

pub fn detail(library: &ShowLibrary, episode: &Episode) -> EpisodeDetail {
    EpisodeDetail {
        status: status(library, episode),
        description: episode.description.clone(),
        guid: episode.guid.clone(),
        enclosure_url: redacted(&episode.enclosure_url).to_owned(),
        image_url: episode.image_url.clone(),
        files: Files {
            audio: episode.audio_file.as_ref().map(|file| state(library.audio_dir().join(file))),
            transcript: state(library.transcript_path(&episode.id)),
            labels: state(library.labels_path(&episode.id)),
            analysis: state(library.analysis_path(&episode.id)),
        },
    }
}

fn status(library: &ShowLibrary, episode: &Episode) -> EpisodeStatus {
    EpisodeStatus {
        id: episode.id.clone(),
        title: episode.title.clone(),
        published: episode.published.strftime("%Y-%m-%d").to_string(),
        season: episode.season,
        number: episode.number,
        kind: episode.kind,
        duration_seconds: episode.feed_duration,
        downloaded: has_audio(library, episode),
        transcribed: library.transcript_path(&episode.id).exists(),
        labeled: library.labels_path(&episode.id).exists(),
    }
}

fn state(path: PathBuf) -> FileState {
    FileState { exists: path.exists(), path }
}

/// The episode's audio file name, when the file is really on disk: the index can outlive it.
pub fn existing_audio<'a>(library: &ShowLibrary, episode: &'a Episode) -> Option<&'a str> {
    let file = episode.audio_file.as_deref()?;
    library.audio_dir().join(file).exists().then_some(file)
}

pub fn has_audio(library: &ShowLibrary, episode: &Episode) -> bool {
    existing_audio(library, episode).is_some()
}
