use std::time::Duration;

use anyhow::Context as _;

use super::item::FeedItem;
use super::parse;

const USER_AGENT: &str = concat!("supercut/", env!("CARGO_PKG_VERSION"));
/// Feeds of long-running shows run to a few megabytes; anything past this is not a feed.
const FEED_LIMIT: u64 = 64 * 1024 * 1024;

/// Fetches one feed and reads its audio items.
pub fn fetch(url: &str) -> anyhow::Result<Vec<FeedItem>> {
    let mut response = agent()
        .get(url)
        .call()
        .with_context(|| format!("fetching {}", redacted(url)))?;
    let xml = response
        .body_mut()
        .with_config()
        .limit(FEED_LIMIT)
        .read_to_string()
        .with_context(|| format!("reading the feed at {}", redacted(url)))?;
    parse::parse(&xml).with_context(|| format!("the feed at {}", redacted(url)))
}

pub fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .user_agent(USER_AGENT)
            .timeout_connect(Some(Duration::from_secs(30)))
            .build(),
    )
}

/// A URL without its query string. Private feeds carry an auth token there, so this is the only
/// form that may reach the terminal, a log or an error.
pub fn redacted(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}
