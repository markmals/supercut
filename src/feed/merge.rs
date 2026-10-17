use std::collections::HashSet;

use super::item::FeedItem;
use super::parse::FALLBACK_GUID_PREFIX;
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
        match known(index, item) {
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

/// The indexed episode this item is. An item without a guid of its own may have been indexed
/// when its enclosure URL served as its id; it is the same episode if the release time and
/// title agree, and its refresh moves it to the new id.
fn known<'a>(index: &'a mut EpisodeIndex, item: &FeedItem) -> Option<&'a mut Episode> {
    let position = index.episodes.iter().position(|episode| episode.guid == item.guid).or_else(|| {
        item.guid.starts_with(FALLBACK_GUID_PREFIX).then_some(())?;
        index.episodes.iter().position(|episode| {
            episode.guid == episode.enclosure_url && episode.published == item.published && episode.title == item.title
        })
    })?;
    index.episodes.get_mut(position)
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

    #[test]
    fn an_episode_indexed_under_its_old_url_id_keeps_its_id_and_audio() {
        // Indexed when a guid-less item's enclosure URL (token included) was its id.
        let mut index = EpisodeIndex::default();
        let mut old = item("a", "Bonus", 1, None);
        old.guid = "https://example.com/bonus.mp3?token=old".into();
        old.enclosure_url = old.guid.clone();
        merge(&mut index, &[old]);
        let id = index.episodes[0].id.clone();
        index.episodes[0].audio_file = Some(format!("{id}.mp3"));

        let mut now = item("unused", "Bonus", 1, None);
        now.guid = super::super::parse::fallback_guid(now.published, "Bonus");
        now.enclosure_url = "https://example.com/bonus.mp3?token=new".into();
        let report = merge(&mut index, &[now]);

        assert!(report.added.is_empty(), "{:?}", report.added);
        assert_eq!(index.episodes.len(), 1);
        assert_eq!(index.episodes[0].id, id);
        assert_eq!(index.episodes[0].audio_file.as_deref(), Some(format!("{id}.mp3").as_str()));
        assert!(!index.episodes[0].guid.contains("token"), "the id moves off the tokened URL");
    }

    #[test]
    fn guid_less_items_differing_only_by_query_stay_apart() {
        let mut first = item("x", "Part 1", 1, None);
        first.guid = super::super::parse::fallback_guid(first.published, "Part 1");
        first.enclosure_url = "https://example.com/stream?episode=1".into();
        let mut second = item("x", "Part 2", 1, None);
        second.guid = super::super::parse::fallback_guid(second.published, "Part 2");
        second.enclosure_url = "https://example.com/stream?episode=2".into();
        let mut index = EpisodeIndex::default();
        assert_eq!(merge(&mut index, &[first, second]).added.len(), 2);
    }
}
