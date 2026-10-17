//! Podcast feeds: reading them, folding them into the episode index, and getting the audio —
//! downloaded, or adopted from files that are already on disk.

mod adopt;
mod download;
mod fetch;
mod item;
mod merge;
mod parse;
mod status;
mod tags;
mod text;
mod walk;

pub use adopt::{AdoptReport, adopt};
pub use download::{Download, download};
pub use fetch::{fetch, redacted};
pub use item::FeedItem;
pub use merge::{MergeReport, merge};
pub use status::{EpisodeStatus, existing_audio, has_audio, statuses};
