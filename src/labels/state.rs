//! The JSON state one labeling request carries.

use serde::Serialize;

use crate::labels::unit::Unit;
use crate::library::Episode;
use crate::timecode::Time;

/// Show notes are background, not evidence; a few hundred characters are plenty.
const DESCRIPTION_LIMIT: usize = 600;

/// What Jev sees: one unit of the episode — a window or a single utterance — and enough
/// around it to place that unit.
#[derive(Debug, Serialize)]
pub struct WindowState<'a> {
    pub show: &'a str,
    pub episode: &'a str,
    pub description: String,
    /// `start-end of total`.
    pub position: String,
    /// How far into the episode the unit starts, 0 to 1.
    pub fraction: f64,
    pub before: &'a str,
    pub window: &'a str,
    pub after: &'a str,
}

pub fn state<'a>(
    show: &'a str,
    episode: &'a Episode,
    unit: &'a Unit,
    duration: f64,
) -> WindowState<'a> {
    WindowState {
        show,
        episode: &episode.title,
        description: shorten(&episode.description, DESCRIPTION_LIMIT),
        position: format!(
            "{}-{} of {}",
            Time(unit.start),
            Time(unit.end),
            Time(duration.max(unit.end))
        ),
        fraction: (unit.start / duration.max(unit.end)).clamp(0.0, 1.0),
        before: &unit.before,
        window: &unit.text,
        after: &unit.after,
    }
}

/// `text` cut at the last word boundary that fits in `limit` characters.
fn shorten(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let kept: String = text.chars().take(limit).collect();
    let end = kept.rfind(char::is_whitespace).unwrap_or(kept.len());
    format!("{}…", kept[..end].trim_end())
}

#[cfg(test)]
mod tests {
    use super::shorten;

    #[test]
    fn shortening_keeps_whole_words_and_whole_characters() {
        assert_eq!(shorten("short enough", 20), "short enough");
        assert_eq!(shorten("a very long sentence indeed", 12), "a very long…");
        assert_eq!(shorten("héllo wörld again", 12), "héllo wörld…");
    }
}
