use std::collections::HashSet;

use super::fetch::redacted;
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
    let incoming: HashSet<&str> = items.iter().map(|item| item.guid.as_str()).collect();
    let mut seen = HashSet::with_capacity(items.len());
    for item in items.iter().filter(|item| seen.insert(item.guid.as_str())) {
        match known(index, item, &incoming) {
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

/// The indexed episode this item is: the one with its guid, or, for an item without a guid of
/// its own, an episode whose made-up id (under any scheme supercut has used) is no longer in the
/// feed, released at the same time with the same title or the same audio path. That is a renamed
/// id, not another episode: refreshing moves the episode to the new id, so a title edit or a
/// rotated token never duplicates it or strands its download, and an episode the feed still
/// carries is never taken over by a newcomer.
fn known<'a>(index: &'a mut EpisodeIndex, item: &FeedItem, incoming: &HashSet<&str>) -> Option<&'a mut Episode> {
    let position = index.episodes.iter().position(|episode| episode.guid == item.guid).or_else(|| {
        item.guid.starts_with(FALLBACK_GUID_PREFIX).then_some(())?;
        index.episodes.iter().position(|episode| {
            made_up(episode)
                && !incoming.contains(episode.guid.as_str())
                && episode.published == item.published
                && (episode.title == item.title || redacted(&episode.enclosure_url) == redacted(&item.enclosure_url))
        })
    })?;
    index.episodes.get_mut(position)
}

/// Whether the episode's id came from supercut rather than the publisher: today's fallback, or
/// the enclosure URL with or without its query string that earlier versions used.
fn made_up(episode: &Episode) -> bool {
    episode.guid.starts_with(FALLBACK_GUID_PREFIX)
        || episode.guid == episode.enclosure_url
        || episode.guid == redacted(&episode.enclosure_url)
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

    /// A guid-less item as `parse` makes it.
    fn guidless(title: &str, day: i64, enclosure_url: &str) -> FeedItem {
        let mut made = item("unused", title, day, None);
        made.enclosure_url = enclosure_url.into();
        made.guid = super::super::parse::fallback_guid(made.published, enclosure_url, title);
        made
    }

    /// Syncs `first`, marks it downloaded, then syncs `then`: the report and the one episode.
    fn resync(first: FeedItem, then: FeedItem) -> (MergeReport, EpisodeIndex) {
        let mut index = EpisodeIndex::default();
        merge(&mut index, &[first]);
        let id = index.episodes[0].id.clone();
        index.episodes[0].audio_file = Some(format!("{id}.mp3"));
        let report = merge(&mut index, &[then]);
        (report, index)
    }

    fn assert_same_episode((report, index): (MergeReport, EpisodeIndex)) {
        assert!(report.added.is_empty(), "{:?}", report.added);
        assert_eq!(index.episodes.len(), 1);
        let episode = &index.episodes[0];
        assert_eq!(episode.audio_file.as_deref(), Some(format!("{}.mp3", episode.id).as_str()));
        assert!(episode.guid.starts_with("no-guid:") && !episode.guid.contains("token"), "{}", episode.guid);
    }

    #[test]
    fn a_guid_less_episode_survives_a_rotated_token_and_a_retitling() {
        let original = guidless("Bonus", 1, "https://example.com/bonus.mp3?token=old");
        assert_same_episode(resync(guidless("Bonus", 1, "https://example.com/bonus.mp3?token=old"), guidless("Bonus", 1, "https://example.com/bonus.mp3?token=new")));
        assert_same_episode(resync(original, guidless("Bonus (corrected)", 1, "https://example.com/bonus.mp3?token=old")));
    }

    #[test]
    fn episodes_indexed_under_earlier_url_ids_carry_over() {
        for old_id in ["https://example.com/bonus.mp3?token=old", "https://example.com/bonus.mp3"] {
            let mut old = item("unused", "Bonus", 1, None);
            old.guid = old_id.into();
            old.enclosure_url = "https://example.com/bonus.mp3?token=old".into();
            assert_same_episode(resync(old, guidless("Bonus", 1, "https://example.com/bonus.mp3?token=new")));
        }
    }

    #[test]
    fn distinct_guid_less_items_stay_apart() {
        let mut index = EpisodeIndex::default();
        let by_query = [guidless("Part 1", 1, "https://example.com/stream?episode=1"), guidless("Part 2", 1, "https://example.com/stream?episode=2")];
        assert_eq!(merge(&mut index, &by_query).added.len(), 2);
        let mut index = EpisodeIndex::default();
        let same_title = [guidless("Bonus", 1, "https://example.com/a.mp3"), guidless("Bonus", 1, "https://example.com/b.mp3")];
        assert_eq!(merge(&mut index, &same_title).added.len(), 2);
    }

    #[test]
    fn a_same_titled_newcomer_on_a_later_sync_does_not_take_over_a_known_episode() {
        let mut index = EpisodeIndex::default();
        merge(&mut index, &[guidless("Bonus", 1, "https://example.com/a.mp3")]);
        let id = index.episodes[0].id.clone();
        index.episodes[0].audio_file = Some(format!("{id}.mp3"));
        // The feed now carries a second "Bonus" released at the same moment.
        let both = [guidless("Bonus", 1, "https://example.com/a.mp3"), guidless("Bonus", 1, "https://example.com/b.mp3")];
        let report = merge(&mut index, &both);
        assert_eq!(report.added.len(), 1, "{:?}", report.added);
        let known = index.get(&id).unwrap();
        assert_eq!(known.enclosure_url, "https://example.com/a.mp3");
        assert_eq!(known.audio_file.as_deref(), Some(format!("{id}.mp3").as_str()));
    }
}
