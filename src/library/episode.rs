use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::error::CliError;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Episode {
    /// Stable short id: `s10e01` for numbered episodes, `<date>-<title words>` otherwise.
    pub id: String,
    pub guid: String,
    pub title: String,
    /// Show notes as plain text.
    pub description: String,
    pub published: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    pub kind: EpisodeKind,
    /// Duration the feed claims, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_duration: Option<f64>,
    pub enclosure_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    /// File name inside the show's `audio/` folder once downloaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_file: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EpisodeKind {
    Full,
    Bonus,
    Trailer,
}

/// Every episode known for a show, oldest first.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct EpisodeIndex {
    pub episodes: Vec<Episode>,
}

impl EpisodeIndex {
    pub fn get(&self, id: &str) -> anyhow::Result<&Episode> {
        self.episodes.iter().find(|episode| episode.id == id).ok_or_else(|| unknown_episode(id))
    }

    pub fn get_mut(&mut self, id: &str) -> anyhow::Result<&mut Episode> {
        self.episodes.iter_mut().find(|episode| episode.id == id).ok_or_else(|| unknown_episode(id))
    }

    pub fn by_guid(&self, guid: &str) -> Option<&Episode> {
        self.episodes.iter().find(|episode| episode.guid == guid)
    }

    /// Episodes chosen on the command line: explicit ids in the given order, or a whole season
    /// (bonus episodes the feed files under that season included), or everything.
    pub fn select(&self, ids: &[String], season: Option<u32>, all: bool) -> anyhow::Result<Vec<&Episode>> {
        if !ids.is_empty() {
            return ids.iter().map(|id| self.get(id)).collect();
        }
        if let Some(season) = season {
            let chosen: Vec<&Episode> =
                self.episodes.iter().filter(|episode| episode.season == Some(season)).collect();
            if chosen.is_empty() {
                return Err(CliError::not_found(format!("no episodes in season {season}")).into());
            }
            return Ok(chosen);
        }
        if all {
            return Ok(self.episodes.iter().collect());
        }
        Err(CliError::invalid("choose episodes: give ids, `--season N` or `--all`").into())
    }

    /// An id for a new episode that no existing episode uses.
    pub fn new_id(&self, season: Option<u32>, number: Option<u32>, published: Timestamp, title: &str) -> String {
        let base = match (season, number) {
            (Some(season), Some(number)) => format!("s{season:02}e{number:02}"),
            (None, Some(number)) => format!("e{number:03}"),
            _ => {
                let date = published.strftime("%Y-%m-%d");
                let words = slug_words(title, 5);
                if words.is_empty() { date.to_string() } else { format!("{date}-{words}") }
            }
        };
        let mut candidate = base.clone();
        let mut suffix = 2;
        while self.episodes.iter().any(|episode| episode.id == candidate) {
            candidate = format!("{base}-{suffix}");
            suffix += 1;
        }
        candidate
    }
}

fn unknown_episode(id: &str) -> anyhow::Error {
    CliError::not_found(format!("no episode `{id}`")).hint("supercut episodes list <show>").into()
}

/// The first `limit` alphanumeric words of `text`, lowercased and joined by hyphens.
fn slug_words(text: &str, limit: usize) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .take(limit)
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(id: &str) -> Episode {
        Episode {
            id: id.into(),
            guid: id.into(),
            title: String::new(),
            description: String::new(),
            published: Timestamp::UNIX_EPOCH,
            season: None,
            number: None,
            kind: EpisodeKind::Full,
            feed_duration: None,
            enclosure_url: String::new(),
            image_url: None,
            audio_file: None,
        }
    }

    #[test]
    fn new_ids_never_collide_with_existing_ones() {
        let published: Timestamp = "2026-03-27T04:00:00Z".parse().unwrap();
        let mut index = EpisodeIndex::default();
        let title = "BONUS ROOM #31: Season 10 Pre-Season Retreat";
        assert_eq!(index.new_id(None, None, published, title), "2026-03-27-bonus-room-31-season-10");
        index.episodes.push(episode("2026-03-27-bonus-room-31-season-10"));
        assert_eq!(index.new_id(None, None, published, title), "2026-03-27-bonus-room-31-season-10-2");
        assert_eq!(index.new_id(Some(10), Some(1), published, title), "s10e01");
    }
}
