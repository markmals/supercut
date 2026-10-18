//! A show's own chapter layout: one chapter per stretch of an episode between its breaks, each
//! titled from a template filled with pieces of the episode's title and its number.

use regex::Regex;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

/// The placeholder `number` fills; the rest come from `title_pattern`'s groups.
const NUMBER: &str = "number";
/// What a `number` template may use besides `title_pattern`'s groups.
const NUMBER_FIELDS: [&str; 2] = ["season", "episode"];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "Written", into = "Written")]
pub struct ChapterScheme {
    parts: Vec<String>,
    number: Vec<String>,
    title_pattern: Regex,
}

/// A show's own chapter layout: one chapter per stretch of an episode between its breaks.
#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ChapterScheme")]
struct Written {
    /// One title per stretch between breaks, in order; `{name}` stands for the `title_pattern`
    /// group of that name, `{number}` for the episode's number.
    parts: Vec<String>,
    /// How `{number}` reads: the first of these templates the episode can fill, from `{season}`
    /// and `{episode}` (the feed's numbers, each at least two digits) and `title_pattern`'s
    /// groups, e.g. `["S{season}E{episode} - ", "Bonus #{bonus} - "]`. Empty when none fills.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    number: Vec<String>,
    /// Splits an episode's title into named groups, e.g. `(?P<title>.+) with (?P<guest>.+)`.
    title_pattern: String,
}

/// An episode as the feed names and numbers it.
pub struct EpisodeName<'a> {
    pub title: &'a str,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

impl TryFrom<Written> for ChapterScheme {
    type Error = String;

    fn try_from(written: Written) -> Result<Self, Self::Error> {
        let title_pattern = Regex::new(&written.title_pattern)
            .map_err(|error| format!("`title_pattern`: {error}"))?;
        if written.parts.is_empty() {
            return Err("`parts` needs at least one title".into());
        }
        let groups: Vec<&str> = title_pattern.capture_names().flatten().collect();
        let part_names: Vec<&str> = groups.iter().copied().chain([NUMBER]).collect();
        if let Some(part) = written.parts.iter().find(|part| !fully_named(part, &part_names)) {
            return Err(format!(
                "`parts` entry `{part}` uses a name neither `number` nor `title_pattern` gives"
            ));
        }
        let number_names: Vec<&str> = groups.iter().copied().chain(NUMBER_FIELDS).collect();
        if let Some(number) =
            written.number.iter().find(|number| !fully_named(number, &number_names))
        {
            return Err(format!(
                "`number` entry `{number}` uses a name other than {{season}}, {{episode}} or a `title_pattern` group"
            ));
        }
        Ok(Self { parts: written.parts, number: written.number, title_pattern })
    }
}

/// Whether every `{…}` in `template` is one of `names`.
fn fully_named(template: &str, names: &[&str]) -> bool {
    let unfilled = names
        .iter()
        .fold(template.to_owned(), |rest, name| rest.replace(&format!("{{{name}}}"), ""));
    !unfilled.contains(['{', '}'])
}

impl From<ChapterScheme> for Written {
    fn from(scheme: ChapterScheme) -> Self {
        Self {
            parts: scheme.parts,
            number: scheme.number,
            title_pattern: scheme.title_pattern.as_str().to_owned(),
        }
    }
}

impl JsonSchema for ChapterScheme {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        Written::schema_name()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        Written::json_schema(generator)
    }
}

impl ChapterScheme {
    /// How many stretches an episode must split into for the scheme to name them.
    pub fn parts(&self) -> usize {
        self.parts.len()
    }

    /// Each part's title for this episode; `None` when the title does not match the pattern or a
    /// group a part uses did not match.
    pub fn titles(&self, episode: &EpisodeName) -> Option<Vec<String>> {
        let captures = self.title_pattern.captures(episode.title)?;
        let mut values: Vec<(&str, Option<String>)> = self
            .title_pattern
            .capture_names()
            .flatten()
            .map(|name| (name, captures.name(name).map(|found| found.as_str().trim().to_owned())))
            .collect();
        values.push(("season", episode.season.map(|season| format!("{season:02}"))));
        values.push(("episode", episode.episode.map(|number| format!("{number:02}"))));
        let number =
            self.number.iter().find_map(|template| fill(template, &values)).unwrap_or_default();
        values.push((NUMBER, Some(number)));
        self.parts.iter().map(|part| fill(part, &values)).collect()
    }
}

/// `template` with each `{name}` it uses replaced; `None` when one it uses has no value.
fn fill(template: &str, values: &[(&str, Option<String>)]) -> Option<String> {
    values.iter().try_fold(template.to_owned(), |title, (name, value)| {
        let placeholder = format!("{{{name}}}");
        if !title.contains(&placeholder) {
            return Some(title);
        }
        Some(title.replace(&placeholder, value.as_deref()?))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Neighborhood Listen's scheme, as its config writes it.
    fn neighborhood_listen() -> ChapterScheme {
        let written = Written {
            parts: vec!["Intro: {number}{title}".into(), "Guest: {number}{guest}".into(), "Outro: {number}{title}".into()],
            number: vec!["S{season}E{episode} - ".into(), "Bonus #{bonus} - ".into()],
            title_pattern: r"^(?:BONUS ROOM #?(?P<bonus>\d+): )?(?P<title>.+) (?:with|w/) (?P<guest>.+?)(?: \(Live at [^)]*\))?$"
                .into(),
        };
        ChapterScheme::try_from(written).unwrap()
    }

    fn titles(title: &str, season: Option<u32>, episode: Option<u32>) -> Option<Vec<String>> {
        neighborhood_listen().titles(&EpisodeName { title, season, episode })
    }

    #[test]
    fn an_episode_title_and_number_fill_each_part() {
        assert_eq!(
            titles("Non-Rabbie Behavior with Joe Wengert", Some(9), Some(8)).unwrap(),
            [
                "Intro: S09E08 - Non-Rabbie Behavior",
                "Guest: S09E08 - Joe Wengert",
                "Outro: S09E08 - Non-Rabbie Behavior"
            ]
        );
        assert_eq!(
            titles("Chili Recipe PLEASE! w/ Mano Agapion", Some(3), Some(12)).unwrap()[1],
            "Guest: S03E12 - Mano Agapion"
        );
        assert_eq!(
            titles("Ask Us Anything with Everyone", None, None).unwrap()[0],
            "Intro: Ask Us Anything",
            "no number at all"
        );
        assert_eq!(
            titles("BONUS ROOM #31: Season 10 Pre-Season Retreat", Some(10), None),
            None,
            "no guest to name"
        );
    }

    #[test]
    fn a_bonus_episode_without_a_feed_number_takes_the_next_number_that_fills() {
        let bonus =
            "BONUS ROOM #30: Rain of Moonfire with Ify Nwadiwe (Live at SF SKetchfest 2026)";
        assert_eq!(
            titles(bonus, Some(9), None).unwrap(),
            [
                "Intro: Bonus #30 - Rain of Moonfire",
                "Guest: Bonus #30 - Ify Nwadiwe",
                "Outro: Bonus #30 - Rain of Moonfire"
            ]
        );
        assert_eq!(
            titles("BONUS ROOM 8: Double Date At The Sizzler with Janet Varney", Some(6), None)
                .unwrap()[0],
            "Intro: Bonus #8 - Double Date At The Sizzler"
        );
    }

    #[test]
    fn a_template_using_a_name_nothing_gives_is_refused() {
        let part = Written {
            parts: vec!["Guest: {guest}".into()],
            number: Vec::new(),
            title_pattern: "(?P<title>.+)".into(),
        };
        assert!(ChapterScheme::try_from(part).unwrap_err().contains("Guest: {guest}"));
        let number = Written {
            parts: vec!["{title}".into()],
            number: vec!["Ep {number}".into()],
            title_pattern: "(?P<title>.+)".into(),
        };
        assert!(ChapterScheme::try_from(number).unwrap_err().contains("Ep {number}"));
    }
}
