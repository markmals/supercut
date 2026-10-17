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

pub use adopt::adopt;
pub use download::{download, planned_file};
pub use fetch::{fetch, redacted};
pub use merge::merge;
pub use status::{Counts, counts, detail, existing_audio, statuses};
