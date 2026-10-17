use serde::Serialize;

use crate::library::{Episode, EpisodeKind, ShowLibrary};

/// One line of `supercut episodes`: what the feed says, plus how far the episode has come.
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

pub fn statuses(library: &ShowLibrary, episodes: &[&Episode]) -> Vec<EpisodeStatus> {
    episodes.iter().map(|episode| status(library, episode)).collect()
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

/// The episode's audio file name, when the file is really on disk: the index can outlive it.
pub fn existing_audio<'a>(library: &ShowLibrary, episode: &'a Episode) -> Option<&'a str> {
    let file = episode.audio_file.as_deref()?;
    library.audio_dir().join(file).exists().then_some(file)
}

pub fn has_audio(library: &ShowLibrary, episode: &Episode) -> bool {
    existing_audio(library, episode).is_some()
}
