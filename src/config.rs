mod chapters;
mod file;
mod transcription;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, anyhow};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use chapters::{ChapterScheme, EpisodeName};
pub use file::{CONFIG_FILE, SCHEMA_FILE, schema};
pub use transcription::{TranscriptionConfig, TranscriptionEngine};

use crate::error::CliError;

const APP_DIR: &str = "supercut";

/// `supercut.config.jsonc` in the platform config directory (`~/Library/Application
/// Support/supercut` on macOS, `$XDG_CONFIG_HOME/supercut` on Linux, `%APPDATA%\supercut` on
/// Windows).
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The JSON Schema this file follows, for editors: `./supercut.config.schema.json`, which
    /// `supercut config init` writes and `supercut config schema` prints.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Downloaded episodes, transcripts, analysis and plans. Defaults to the platform data directory.
    pub library_dir: Option<PathBuf>,
    /// Where rendered supercuts go, one folder per show.
    pub output_dir: Option<PathBuf>,
    /// Podcast app sideload folder that `render --sideload` copies finished supercuts into.
    pub sideload_dir: Option<PathBuf>,
    #[serde(default)]
    pub transcription: TranscriptionConfig,
    #[serde(default)]
    pub loudness: LoudnessConfig,
    #[serde(default)]
    pub jev: JevSettings,
    #[serde(default)]
    pub shows: BTreeMap<String, ShowConfig>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LoudnessConfig {
    /// Integrated loudness every episode is brought to, in LUFS.
    pub target_lufs: f64,
    /// Ceiling for the true-peak limiter, in dBTP.
    pub true_peak_db: f64,
}

impl Default for LoudnessConfig {
    fn default() -> Self {
        Self { target_lufs: -16.0, true_peak_db: -1.5 }
    }
}

/// TypeSafe's Jev decision model, which labels transcript windows.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JevSettings {
    /// The System One endpoint requests are POSTed to.
    pub endpoint: String,
    /// Pinned model id; probabilities shift between releases.
    pub model: String,
    /// Command that prints the API key, e.g. `["op", "read", "op://…"]`. The `TYPESAFE_API_KEY`
    /// environment variable takes precedence.
    #[serde(default)]
    pub api_key_command: Vec<String>,
    /// Requests in flight at once.
    pub concurrency: usize,
}

impl Default for JevSettings {
    fn default() -> Self {
        Self {
            endpoint: "https://api.typesafe.ai/v1/systemone".into(),
            model: "jev-1.13.0".into(),
            api_key_command: Vec::new(),
            concurrency: 8,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShowConfig {
    pub title: String,
    /// RSS feeds to read episodes from, private feeds included. Later feeds add episodes the
    /// earlier ones lack; an episode in several feeds keeps the first feed's copy.
    pub feeds: Vec<String>,
    /// Cover art embedded in rendered supercuts.
    pub artwork: Option<PathBuf>,
    /// Clip played between episodes.
    pub interstitial: Option<PathBuf>,
    /// Recordings of the theme songs and jingles that open episodes (a clip from one episode is
    /// enough; `episodes clip` makes one). `plans draft` cuts each episode up to the end of the
    /// one that plays in its opening.
    #[serde(default)]
    pub themes: Vec<PathBuf>,
    /// Recordings of the first notes of the music that leads into the credits (cut from just
    /// after a goodbye). `plans draft` cuts each episode from where one starts to the end.
    #[serde(default)]
    pub outros: Vec<PathBuf>,
    /// Recordings of jingles that close some episodes (a bonus series' own sign-off). `plans
    /// draft` keeps one that ends an episode, and no interstitial follows that episode.
    #[serde(default)]
    pub bumpers: Vec<PathBuf>,
    /// The show's recurring segments. `label` sorts every stretch of an episode into one of
    /// these; `draft` turns them into cuts and chapters.
    #[serde(default)]
    pub segments: Vec<SegmentKind>,
    /// The show's own chapter layout. Without it, `plans draft` makes a chapter per labeled
    /// segment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chapters: Option<ChapterScheme>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SegmentKind {
    /// Short id, e.g. `classified`.
    pub label: String,
    /// Chapter title for stretches of this segment.
    pub title: String,
    /// What the segment sounds like, written for the classifier.
    pub describe: String,
    /// `draft` cuts this segment from every episode.
    #[serde(default)]
    pub cut: bool,
    /// Where a cut of this segment begins.
    #[serde(default)]
    pub cut_start: CutStart,
    /// Where it ends.
    #[serde(default)]
    pub cut_end: CutEnd,
    /// Phrases said right before this segment, e.g. "be right back": a cut of it starts after
    /// the one said nearest where the labels put the segment's start.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub after_phrases: Vec<String>,
    /// Phrases said right after this segment, e.g. "welcome back": a cut of it ends before the
    /// first one said once the segment has begun.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub before_phrases: Vec<String>,
}

/// Which side of the pause before the segment the cut starts on.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CutStart {
    /// Right after the last word kept before the segment: music leading into it goes too.
    #[default]
    AfterPreviousSpeech,
    /// Just before the segment's own first word: the music leading into it stays.
    BeforeOwnSpeech,
}

/// Which side of the pause after the segment the cut ends on.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CutEnd {
    /// Just before the next kept word: music trailing out of the segment goes too.
    #[default]
    BeforeNextSpeech,
    /// Right after the segment's own last word: the music trailing out of it stays.
    AfterOwnSpeech,
}

impl Config {
    pub fn default_path() -> anyhow::Result<PathBuf> {
        let base = dirs::config_dir().ok_or_else(|| anyhow!("no config directory on this platform"))?;
        Ok(base.join(APP_DIR).join(CONFIG_FILE))
    }

    /// Reads the config file; a missing file is an empty config so `config init` can run.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
        };
        let mut config = file::parse(&text).map_err(|problem| {
            CliError::invalid(format!("{} is not a valid config: {problem}", path.display()))
                .hint("`supercut config init --config <scratch>.jsonc` writes a commented example of every setting")
        })?;
        config.expand_home_paths();
        Ok(config)
    }

    pub fn library_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(dir) = &self.library_dir {
            return Ok(dir.clone());
        }
        let base = dirs::data_local_dir().ok_or_else(|| anyhow!("no data directory on this platform"))?;
        Ok(base.join(APP_DIR).join("library"))
    }

    pub fn output_dir(&self) -> anyhow::Result<&Path> {
        self.output_dir.as_deref().ok_or_else(|| {
            CliError::precondition("`output_dir` is not set in the config file").hint("supercut config get").into()
        })
    }

    pub fn show(&self, slug: &str) -> anyhow::Result<&ShowConfig> {
        self.shows.get(slug).ok_or_else(|| {
            let known: Vec<&str> = self.shows.keys().map(String::as_str).collect();
            CliError::not_found(format!("no show `{slug}` in the config file"))
                .hint("supercut shows list")
                .detail("configured", known)
                .into()
        })
    }

    fn expand_home_paths(&mut self) {
        for path in [&mut self.library_dir, &mut self.output_dir, &mut self.sideload_dir] {
            expand_home(path);
        }
        if let Some(model) = self.transcription.model.as_mut().filter(|model| model.starts_with('~')) {
            *model = home_expanded(Path::new(model)).to_string_lossy().into_owned();
        }
        for show in self.shows.values_mut() {
            expand_home(&mut show.artwork);
            expand_home(&mut show.interstitial);
            for recording in show.themes.iter_mut().chain(show.outros.iter_mut()).chain(show.bumpers.iter_mut()) {
                *recording = home_expanded(recording);
            }
        }
    }
}

fn expand_home(path: &mut Option<PathBuf>) {
    if let Some(current) = path.as_mut() {
        *current = home_expanded(current);
    }
}

fn home_expanded(path: &Path) -> PathBuf {
    match (path.strip_prefix("~"), dirs::home_dir()) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => path.to_path_buf(),
    }
}
