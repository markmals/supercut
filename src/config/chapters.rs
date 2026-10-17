//! A show's own chapter layout: one chapter per stretch of an episode between its breaks, each
//! titled from a template filled with pieces of the episode's title.

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "Written", into = "Written")]
pub struct ChapterScheme {
    parts: Vec<String>,
    title_pattern: Regex,
}

/// The scheme as the config file writes it.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Written {
    /// One title per stretch between breaks, in order; `{name}` stands for the `title_pattern`
    /// group of that name.
    parts: Vec<String>,
    /// Splits an episode's title into named groups, e.g. `(?P<title>.+) with (?P<guest>.+)`.
    title_pattern: String,
}

impl TryFrom<Written> for ChapterScheme {
    type Error = String;

    fn try_from(written: Written) -> Result<Self, Self::Error> {
        let title_pattern = Regex::new(&written.title_pattern).map_err(|error| format!("`title_pattern`: {error}"))?;
        let names: Vec<&str> = title_pattern.capture_names().flatten().collect();
        for part in &written.parts {
            let unfilled = names.iter().fold(part.clone(), |rest, name| rest.replace(&format!("{{{name}}}"), ""));
            if unfilled.contains(['{', '}']) {
                return Err(format!("`parts` entry `{part}` uses a name `title_pattern` has no group for"));
            }
        }
        if written.parts.is_empty() {
            return Err("`parts` needs at least one title".into());
        }
        Ok(Self { parts: written.parts, title_pattern })
    }
}

impl From<ChapterScheme> for Written {
    fn from(scheme: ChapterScheme) -> Self {
        Self { parts: scheme.parts, title_pattern: scheme.title_pattern.as_str().to_owned() }
    }
}

impl ChapterScheme {
    /// How many stretches an episode must split into for the scheme to name them.
    pub fn parts(&self) -> usize {
        self.parts.len()
    }

    /// Each part's title for this episode; `None` when the title does not match the pattern or a
    /// group a part uses did not match.
    pub fn titles(&self, episode_title: &str) -> Option<Vec<String>> {
        let captures = self.title_pattern.captures(episode_title)?;
        self.parts
            .iter()
            .map(|part| {
                self.title_pattern.capture_names().flatten().try_fold(part.clone(), |title, name| {
                    let placeholder = format!("{{{name}}}");
                    if !title.contains(&placeholder) {
                        return Some(title);
                    }
                    Some(title.replace(&placeholder, captures.name(name)?.as_str().trim()))
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Neighborhood Listen's scheme, as its config writes it.
    fn neighborhood_listen() -> ChapterScheme {
        let written = Written {
            parts: vec!["Intro: {title}".into(), "Guest: {guest}".into(), "Outro: {title}".into()],
            title_pattern: r"^(?P<title>.+) (?:with|w/) (?P<guest>.+?)(?: \(Live at [^)]*\))?$".into(),
        };
        ChapterScheme::try_from(written).unwrap()
    }

    #[test]
    fn an_episode_title_fills_each_part() {
        let titles = |episode: &str| neighborhood_listen().titles(episode);
        assert_eq!(
            titles("Non-Rabbie Behavior with Joe Wengert").unwrap(),
            ["Intro: Non-Rabbie Behavior", "Guest: Joe Wengert", "Outro: Non-Rabbie Behavior"]
        );
        assert_eq!(titles("Chili Recipe PLEASE! w/ Mano Agapion").unwrap()[1], "Guest: Mano Agapion");
        assert_eq!(
            titles("BONUS ROOM #30: Rain of Moonfire with Ify Nwadiwe (Live at SF SKetchfest 2026)").unwrap(),
            ["Intro: BONUS ROOM #30: Rain of Moonfire", "Guest: Ify Nwadiwe", "Outro: BONUS ROOM #30: Rain of Moonfire"]
        );
        assert_eq!(titles("BONUS ROOM #31: Season 10 Pre-Season Retreat"), None, "no guest to name");
    }

    #[test]
    fn a_part_naming_a_group_the_pattern_lacks_is_refused() {
        let written = Written { parts: vec!["Guest: {guest}".into()], title_pattern: "(?P<title>.+)".into() };
        let error = ChapterScheme::try_from(written).unwrap_err();
        assert!(error.contains("Guest: {guest}"), "{error}");
    }
}
