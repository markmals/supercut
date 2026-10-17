use std::collections::HashSet;

use super::item::FeedItem;
use crate::library::{Episode, EpisodeIndex};

/// What one `sync` changed.
pub struct MergeReport {
    /// Ids of the episodes the feeds added, oldest first.
    pub added: Vec<String>,
    /// How many known episodes the feeds changed.
    pub updated: usize,
}

/// Folds freshly fetched items into the index. Known episodes (matched by guid) keep their id
/// and their downloaded file while their metadata refreshes; new ones are numbered oldest first
/// so the same feed always yields the same ids. An episode carried by several feeds keeps the
/// copy from the earliest feed, so `items` must be in feed order.
pub fn merge(index: &mut EpisodeIndex, items: &[FeedItem]) -> MergeReport {
    let mut report = MergeReport { added: Vec::new(), updated: 0 };
    let mut fresh = Vec::new();
    let mut seen = HashSet::with_capacity(items.len());
    for item in items.iter().filter(|item| seen.insert(item.guid.as_str())) {
        match known(index, &item.guid) {
            Some(episode) => report.updated += usize::from(refresh(episode, item)),
            None => fresh.push(item),
        }
    }

    fresh.sort_by_key(|item| item.published);
    for item in fresh {
        let id = index.new_id(item.season, item.number, item.published, &item.title);
        index.episodes.push(item.to_episode(id.clone(), None));
        report.added.push(id);
    }
    index.episodes.sort_by_key(|episode| episode.published);
    report
}

fn known<'a>(index: &'a mut EpisodeIndex, guid: &str) -> Option<&'a mut Episode> {
    index.episodes.iter_mut().find(|episode| episode.guid == guid)
}

/// Whether the feed had anything new to say about an episode we already know.
fn refresh(episode: &mut Episode, item: &FeedItem) -> bool {
    let refreshed = item.to_episode(episode.id.clone(), episode.audio_file.clone());
    if *episode == refreshed {
        return false;
    }
    *episode = refreshed;
    true
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;

    use super::*;
    use crate::library::EpisodeKind;

    fn item(guid: &str, title: &str, day: i64, number: Option<u32>) -> FeedItem {
        FeedItem {
            guid: guid.into(),
            title: title.into(),
            description: String::new(),
            published: Timestamp::from_second(day * 86_400).unwrap(),
            season: number.map(|_| 1),
            number,
            kind: EpisodeKind::Full,
            duration: None,
            enclosure_url: format!("https://example.com/{guid}.mp3"),
            image_url: None,
        }
    }

    #[test]
    fn numbers_new_episodes_oldest_first_whatever_the_feed_order() {
        let mut index = EpisodeIndex::default();
        let items = [item("c", "Third", 3, None), item("a", "First", 1, None), item("b", "Second", 2, None)];
        let report = merge(&mut index, &items);

        assert_eq!(report.added, ["1970-01-02-first", "1970-01-03-second", "1970-01-04-third"]);
        let titles: Vec<&str> = index.episodes.iter().map(|episode| episode.title.as_str()).collect();
        assert_eq!(titles, ["First", "Second", "Third"]);
    }

    #[test]
    fn resyncing_keeps_ids_and_downloaded_files() {
        let mut index = EpisodeIndex::default();
        merge(&mut index, &[item("a", "First", 1, Some(1)), item("b", "Second", 2, Some(2))]);
        index.get_mut("s01e01").unwrap().audio_file = Some("s01e01.mp3".into());

        let renamed = [item("a", "First, remastered", 1, Some(1)), item("b", "Second", 2, Some(2))];
        let report = merge(&mut index, &renamed);

        assert!(report.added.is_empty());
        assert_eq!(report.updated, 1);
        let first = index.get("s01e01").unwrap();
        assert_eq!(first.title, "First, remastered");
        assert_eq!(first.audio_file.as_deref(), Some("s01e01.mp3"));
        assert_eq!(index.episodes.len(), 2);
    }

    #[test]
    fn the_first_feed_to_carry_an_episode_wins() {
        let mut index = EpisodeIndex::default();
        let mut later_feed_copy = item("a", "From the second feed", 1, Some(1));
        later_feed_copy.enclosure_url = "https://example.com/other.mp3".into();
        let report = merge(&mut index, &[item("a", "From the first feed", 1, Some(1)), later_feed_copy]);

        assert_eq!(report.added, ["s01e01"]);
        assert_eq!(index.episodes.len(), 1);
        assert_eq!(index.get("s01e01").unwrap().title, "From the first feed");
    }
}
