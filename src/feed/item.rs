use jiff::Timestamp;

use crate::library::{Episode, EpisodeKind};

/// One audio item of a feed, already cleaned up: the library's view of an episode before it has
/// an id or a local file.
#[derive(Clone, Debug)]
pub struct FeedItem {
    /// The feed's `guid`, or the enclosure URL when the feed states none.
    pub guid: String,
    pub title: String,
    /// Show notes as plain text.
    pub description: String,
    pub published: Timestamp,
    pub season: Option<u32>,
    pub number: Option<u32>,
    pub kind: EpisodeKind,
    /// Seconds, from `itunes:duration`.
    pub duration: Option<f64>,
    pub enclosure_url: String,
    pub image_url: Option<String>,
}

impl FeedItem {
    /// The library episode this item describes. The id and the downloaded file come from the
    /// index, so a re-synced episode keeps both.
    pub fn to_episode(&self, id: String, audio_file: Option<String>) -> Episode {
        Episode {
            id,
            guid: self.guid.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            published: self.published,
            season: self.season,
            number: self.number,
            kind: self.kind,
            feed_duration: self.duration,
            enclosure_url: self.enclosure_url.clone(),
            image_url: self.image_url.clone(),
            audio_file,
        }
    }
}
