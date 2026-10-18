use anyhow::{Context as _, bail, ensure};
use jiff::Timestamp;
use rss::Channel;
use rss::extension::itunes::ITunesItemExtension;

use super::item::FeedItem;
use super::text::plain_text;
use crate::library::EpisodeKind;

/// Every item of the feed that carries audio, in feed order. Items without an enclosure are
/// announcements, not episodes.
pub fn parse(xml: &str) -> anyhow::Result<Vec<FeedItem>> {
    let channel = Channel::read_from(xml.as_bytes()).context("reading the feed as RSS")?;
    channel
        .items()
        .iter()
        .filter_map(|item| item.enclosure().map(|enclosure| feed_item(item, enclosure)))
        .collect()
}

fn feed_item(item: &rss::Item, enclosure: &rss::Enclosure) -> anyhow::Result<FeedItem> {
    let title = plain_text(item.title().context("a feed item has no title")?);
    let itunes = item.itunes_ext();
    let published = item
        .pub_date()
        .with_context(|| format!("feed item `{title}` has no pubDate"))
        .and_then(published)
        .with_context(|| format!("feed item `{title}`"))?;
    Ok(FeedItem {
        // Private feeds put rotating tokens in the enclosure's query string, so an item without
        // a guid is known by its release time, its audio's path and its title instead.
        guid: guid(item).unwrap_or_else(|| fallback_guid(published, enclosure.url(), &title)),
        description: plain_text(description(item, itunes)),
        published,
        season: number(itunes.and_then(ITunesItemExtension::season), "itunes:season", &title),
        number: number(itunes.and_then(ITunesItemExtension::episode), "itunes:episode", &title),
        kind: kind(itunes.and_then(ITunesItemExtension::episode_type))?,
        duration: itunes
            .and_then(ITunesItemExtension::duration)
            .map(duration)
            .transpose()
            .with_context(|| format!("feed item `{title}`"))?,
        enclosure_url: enclosure.url().to_owned(),
        image_url: itunes.and_then(ITunesItemExtension::image).map(str::to_owned),
        title,
    })
}

/// The identity of an item that has no guid of its own: never its URL's query string, which may
/// hold a token. `merge` still recognizes the episode after a title edit or a token change.
pub fn fallback_guid(published: Timestamp, enclosure_url: &str, title: &str) -> String {
    format!("{FALLBACK_GUID_PREFIX}{published}:{}:{title}", super::fetch::redacted(enclosure_url))
}

pub const FALLBACK_GUID_PREFIX: &str = "no-guid:";

fn guid(item: &rss::Item) -> Option<String> {
    let value = item.guid()?.value().trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// iTunes summaries are written for listeners; `description` and `content:encoded` are the
/// fallbacks, in that order.
fn description<'a>(item: &'a rss::Item, itunes: Option<&'a ITunesItemExtension>) -> &'a str {
    itunes
        .and_then(ITunesItemExtension::summary)
        .or_else(|| item.description())
        .or_else(|| item.content())
        .unwrap_or_default()
}

fn published(pub_date: &str) -> anyhow::Result<Timestamp> {
    let zoned = jiff::fmt::rfc2822::parse(pub_date.trim())
        .with_context(|| format!("pubDate `{pub_date}` is not an RFC 2822 date"))?;
    Ok(zoned.timestamp())
}

/// Feeds number half-episodes `235.5` and such. Those cannot become `s07e235.5`, so they are
/// filed unnumbered, loudly.
fn number(value: Option<&str>, field: &str, title: &str) -> Option<u32> {
    let value = value.map(str::trim).filter(|value| !value.is_empty())?;
    match value.parse() {
        Ok(number) => Some(number),
        Err(_) => {
            eprintln!("`{title}`: {field} is `{value}`, not a whole number; filing it unnumbered");
            None
        }
    }
}

fn kind(episode_type: Option<&str>) -> anyhow::Result<EpisodeKind> {
    match episode_type.unwrap_or("full").trim().to_ascii_lowercase().as_str() {
        "full" => Ok(EpisodeKind::Full),
        "bonus" => Ok(EpisodeKind::Bonus),
        "trailer" => Ok(EpisodeKind::Trailer),
        other => bail!("unknown itunes:episodeType `{other}`"),
    }
}

/// `itunes:duration`: seconds, `MM:SS` or `H:MM:SS`.
fn duration(value: &str) -> anyhow::Result<f64> {
    let parts: Vec<&str> = value.trim().split(':').collect();
    ensure!(
        (1..=3).contains(&parts.len()),
        "itunes:duration `{value}` is not seconds, MM:SS or H:MM:SS"
    );
    let mut seconds = 0.0;
    for part in parts {
        let part: f64 = part.parse().with_context(|| format!("itunes:duration `{value}`"))?;
        seconds = seconds * 60.0 + part;
    }
    Ok(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd"
     xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel>
    <title>Example Show</title>
    <item>
      <title>12. Tea Towels &amp; A Surprise</title>
      <guid isPermaLink="false">abc-123</guid>
      <pubDate>Tue, 06 Aug 2024 07:00:00 -0700</pubDate>
      <description>&lt;p&gt;Hosted by &lt;b&gt;Burt&lt;/b&gt;.&lt;/p&gt;</description>
      <enclosure url="https://example.com/audio/12.mp3" type="audio/mpeg" length="1000"/>
      <itunes:summary><![CDATA[<p>Towels, and  a surprise.</p><p>More at <a href="https://example.com">example</a>.</p>]]></itunes:summary>
      <itunes:season>3</itunes:season>
      <itunes:episode>12</itunes:episode>
      <itunes:episodeType>full</itunes:episodeType>
      <itunes:duration>1:02:03</itunes:duration>
      <itunes:image href="https://example.com/art.jpg"/>
    </item>
    <item>
      <title>Bonus Room</title>
      <pubDate>Wed, 07 Aug 2024 07:00:00 -0700</pubDate>
      <content:encoded>Just &lt;i&gt;chatting&lt;/i&gt;.</content:encoded>
      <enclosure url="https://example.com/audio/bonus.m4a?token=x" type="audio/mp4" length="1000"/>
      <itunes:episodeType>bonus</itunes:episodeType>
      <itunes:duration>45:30</itunes:duration>
    </item>
    <item>
      <title>Trailer</title>
      <guid>  </guid>
      <pubDate>Thu, 08 Aug 2024 07:00:00 -0700</pubDate>
      <enclosure url="https://example.com/audio/trailer.mp3" type="audio/mpeg" length="10"/>
      <itunes:duration>95</itunes:duration>
    </item>
    <item>
      <title>A note from the hosts</title>
      <pubDate>Fri, 09 Aug 2024 07:00:00 -0700</pubDate>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn reads_itunes_fields_and_cleans_text() {
        let items = parse(FEED).unwrap();
        let first = &items[0];
        assert_eq!(first.guid, "abc-123");
        assert_eq!(first.title, "12. Tea Towels & A Surprise");
        assert_eq!(first.description, "Towels, and a surprise. More at example.");
        assert_eq!(first.published.to_string(), "2024-08-06T14:00:00Z");
        assert_eq!((first.season, first.number), (Some(3), Some(12)));
        assert_eq!(first.kind, EpisodeKind::Full);
        assert_eq!(first.image_url.as_deref(), Some("https://example.com/art.jpg"));
    }

    #[test]
    fn skips_items_without_audio() {
        let titles: Vec<String> =
            parse(FEED).unwrap().iter().map(|item| item.title.clone()).collect();
        assert_eq!(titles, ["12. Tea Towels & A Surprise", "Bonus Room", "Trailer"]);
    }

    #[test]
    fn an_item_without_a_guid_never_takes_its_url_token_into_its_id() {
        let items = parse(FEED).unwrap();
        assert_eq!(
            items[1].guid,
            "no-guid:2024-08-07T14:00:00Z:https://example.com/audio/bonus.m4a:Bonus Room"
        );
        assert_eq!(
            items[2].guid,
            "no-guid:2024-08-08T14:00:00Z:https://example.com/audio/trailer.mp3:Trailer"
        );
        assert!(
            items.iter().all(|item| !item.guid.contains("token")),
            "a private token never becomes an id"
        );
    }

    #[test]
    fn reads_every_duration_form() {
        let items = parse(FEED).unwrap();
        let durations: Vec<Option<f64>> = items.iter().map(|item| item.duration).collect();
        assert_eq!(durations, [Some(3723.0), Some(2730.0), Some(95.0)]);
    }

    #[test]
    fn describes_from_content_when_there_is_no_summary() {
        assert_eq!(parse(FEED).unwrap()[1].description, "Just chatting.");
        assert_eq!(parse(FEED).unwrap()[1].kind, EpisodeKind::Bonus);
    }

    #[test]
    fn rejects_an_item_without_a_date() {
        let broken = FEED.replace("<pubDate>Tue, 06 Aug 2024 07:00:00 -0700</pubDate>", "");
        let error = parse(&broken).unwrap_err();
        assert!(format!("{error:#}").contains("no pubDate"), "{error:#}");
    }
}
