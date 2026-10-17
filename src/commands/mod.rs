pub mod cli;
pub mod config;
pub mod episodes;
pub mod history;
pub mod labels;
pub mod plans;
pub mod registry;
pub mod rules;
pub mod schema;
pub mod shows;
pub mod transcripts;

use std::path::PathBuf;

use crate::config::{Config, ShowConfig};
use crate::library::{Episode, EpisodeIndex, ShowLibrary};

/// What every command starts from: the config and where things live.
pub struct Context {
    pub config: Config,
    pub config_path: PathBuf,
    pub library_dir: PathBuf,
}

impl Context {
    pub fn load(config_path: Option<PathBuf>) -> anyhow::Result<Self> {
        let config_path = match config_path {
            Some(path) => path,
            None => Config::default_path()?,
        };
        let config = Config::load(&config_path)?;
        let library_dir = config.library_dir()?;
        Ok(Self { config, config_path, library_dir })
    }

    /// A configured show and its library folder.
    pub fn show(&self, slug: &str) -> anyhow::Result<(&ShowConfig, ShowLibrary)> {
        let show = self.config.show(slug)?;
        Ok((show, ShowLibrary::new(&self.library_dir, slug)))
    }

    /// The show's editorial rules: plain Markdown the agent reads before planning cuts.
    pub fn rules_path(&self, slug: &str) -> PathBuf {
        let config_dir = self.config_path.parent().map(PathBuf::from).unwrap_or_default();
        config_dir.join("rules").join(format!("{slug}.md"))
    }
}

/// Which episodes a command acts on.
#[derive(clap::Args, Debug)]
pub struct Selection {
    /// Episode ids, as listed by `supercut episodes list <show>`.
    pub ids: Vec<String>,
    /// Every episode the feed files under this season, bonus episodes included.
    #[arg(long, conflicts_with = "ids")]
    pub season: Option<u32>,
    /// Every episode of the show.
    #[arg(long, conflicts_with_all = ["ids", "season"])]
    pub all: bool,
}

impl Selection {
    pub fn resolve<'a>(&self, index: &'a EpisodeIndex) -> anyhow::Result<Vec<&'a Episode>> {
        index.select(&self.ids, self.season, self.all)
    }
}
