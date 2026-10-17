mod episode;

use std::path::{Path, PathBuf};

use anyhow::Context as _;

pub use episode::{Episode, EpisodeIndex, EpisodeKind};

/// One show's folder in the library:
///
/// ```text
/// <library>/<show>/
///   episodes.json          episode index (feed metadata + local audio file name)
///   audio/<id>.<ext>       downloaded episodes
///   transcripts/<id>.json  word-timed transcripts
///   analysis/<id>.json     loudness and format; <id>.envelope holds the level envelope
///   labels/<id>.json       Jev segment labels per transcript window
///   fingerprints/<id>.fp   chromaprint fingerprints (history alignment)
///   plans/                 cut plans
///   previews/              transition reels rendered from plans
///   history/               cuts recovered from hand-made supercuts
/// ```
pub struct ShowLibrary {
    pub slug: String,
    pub root: PathBuf,
}

impl ShowLibrary {
    pub fn new(library_dir: &Path, slug: &str) -> Self {
        Self { slug: slug.to_owned(), root: library_dir.join(slug) }
    }

    pub fn index_path(&self) -> PathBuf {
        self.root.join("episodes.json")
    }

    pub fn audio_dir(&self) -> PathBuf {
        self.root.join("audio")
    }

    pub fn plans_dir(&self) -> PathBuf {
        self.root.join("plans")
    }

    pub fn previews_dir(&self) -> PathBuf {
        self.root.join("previews")
    }

    pub fn labels_path(&self, id: &str) -> PathBuf {
        self.root.join("labels").join(format!("{id}.json"))
    }

    pub fn history_dir(&self) -> PathBuf {
        self.root.join("history")
    }

    pub fn transcript_path(&self, id: &str) -> PathBuf {
        self.root.join("transcripts").join(format!("{id}.json"))
    }

    pub fn analysis_path(&self, id: &str) -> PathBuf {
        self.root.join("analysis").join(format!("{id}.json"))
    }

    pub fn envelope_path(&self, id: &str) -> PathBuf {
        self.root.join("analysis").join(format!("{id}.envelope"))
    }

    pub fn fingerprint_path(&self, id: &str) -> PathBuf {
        self.root.join("fingerprints").join(format!("{id}.fp"))
    }

    /// The episode's downloaded audio, or an error naming the command that fetches it.
    pub fn audio_path(&self, episode: &Episode) -> anyhow::Result<PathBuf> {
        let file = episode.audio_file.as_deref().with_context(|| {
            format!("episode `{}` is not downloaded; run `supercut download {} {}`", episode.id, self.slug, episode.id)
        })?;
        Ok(self.audio_dir().join(file))
    }

    /// The index, or an empty one before the first `sync`.
    pub fn load_index(&self) -> anyhow::Result<EpisodeIndex> {
        let path = self.index_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(EpisodeIndex::default()),
            Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// The index of a show that has been synced at least once.
    pub fn load_synced_index(&self) -> anyhow::Result<EpisodeIndex> {
        let index = self.load_index()?;
        anyhow::ensure!(!index.episodes.is_empty(), "no episodes for `{0}`; run `supercut sync {0}` first", self.slug);
        Ok(index)
    }

    pub fn save_index(&self, index: &EpisodeIndex) -> anyhow::Result<()> {
        write_json_atomically(&self.index_path(), index)
    }
}

/// Writes JSON beside the destination and renames it into place, creating parent folders.
pub fn write_json_atomically(path: &Path, value: &impl serde::Serialize) -> anyhow::Result<()> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    write_atomically(path, text.as_bytes())
}

pub fn write_atomically(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("path has no parent folder")?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    let partial = partial_path(path);
    std::fs::write(&partial, bytes).with_context(|| format!("writing {}", partial.display()))?;
    std::fs::rename(&partial, path).with_context(|| format!("moving {} into place", path.display()))
}

/// `<name>.partial` beside `path`: where long writes go until they finish.
pub fn partial_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".partial");
    path.with_file_name(name)
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}
