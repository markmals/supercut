use std::time::Duration;

use super::item::FeedItem;
use super::parse;
use crate::error::{CliError, ErrorCode};

const USER_AGENT: &str = concat!("supercut/", env!("CARGO_PKG_VERSION"));
/// Feeds of long-running shows run to a few megabytes; anything past this is not a feed.
const FEED_LIMIT: u64 = 64 * 1024 * 1024;

/// Fetches one feed and reads its audio items.
pub fn fetch(url: &str) -> anyhow::Result<Vec<FeedItem>> {
    let mut response = agent().get(url).call().map_err(|error| request_error(url, &error))?;
    let xml = response
        .body_mut()
        .with_config()
        .limit(FEED_LIMIT)
        .read_to_string()
        .map_err(|error| request_error(url, &error))?;
    parse::parse(&xml).map_err(|error| {
        CliError::invalid(format!("the feed at {} is not readable RSS: {error:#}", redacted(url)))
            .detail("feed", redacted(url))
            .into()
    })
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

/// Why the request failed, in terms an agent can act on: a rejected token, throttling and
/// outages are worth retrying or re-authenticating, a wrong URL is not.
pub fn request_error(url: &str, error: &ureq::Error) -> anyhow::Error {
    let url = redacted(url);
    let (code, hint) = match error {
        ureq::Error::StatusCode(401 | 403) => {
            (ErrorCode::Unauthenticated, Some("check the feed's auth token in the config file"))
        }
        ureq::Error::StatusCode(429) => (ErrorCode::RateLimited, None),
        ureq::Error::StatusCode(404 | 410) => {
            (ErrorCode::NotFound, Some("check the feed URL in the config file"))
        }
        ureq::Error::StatusCode(status) if *status < 500 => (ErrorCode::InvalidArgument, None),
        _ => (ErrorCode::Unavailable, None),
    };
    let mut cli = CliError::new(code, format!("fetching {url}: {error}")).detail("url", url);
    if let Some(hint) = hint {
        cli = cli.hint(hint);
    }
    cli.into()
}
