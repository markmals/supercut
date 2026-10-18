//! Chapters from a show's own layout: one per stretch of an episode between its breaks, so the
//! labels' flickers inside a segment never become chapters.

use crate::config::{ChapterScheme, EpisodeName};
use crate::plan::{Chapter, Cut};
use crate::timecode::Time;

/// One chapter per stretch between the episode's breaks (the cuts with audio kept on both
/// sides), titled by the scheme; why not when the episode does not fit it. `cuts` are the
/// episode's cuts before a closing bumper is let back in, so its closing cut runs to the end.
pub fn chapters(
    scheme: &ChapterScheme,
    episode: &EpisodeName,
    cuts: &[Cut],
) -> Result<Vec<Chapter>, String> {
    let breaks: Vec<f64> = cuts
        .iter()
        .filter(|cut| cut.start.seconds() > 0.0)
        .filter_map(|cut| cut.end.map(Time::seconds))
        .collect();
    if breaks.len() + 1 != scheme.parts() {
        return Err(format!(
            "its {} breaks split it into {} parts, and the show's chapters name {}",
            breaks.len(),
            breaks.len() + 1,
            scheme.parts()
        ));
    }
    let titles = scheme
        .titles(episode)
        .ok_or("its title does not match the show's chapter `title_pattern`")?;
    let opening_end = cuts
        .first()
        .filter(|cut| cut.start.seconds() <= 0.0)
        .and_then(|cut| cut.end)
        .map_or(0.0, Time::seconds);
    let starts = std::iter::once(opening_end).chain(breaks);
    Ok(starts
        .zip(titles)
        .map(|(start, title)| Chapter { start: Time(start), title, description: None })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheme() -> ChapterScheme {
        let written = r#"{
            "parts": ["Intro: {title}", "Guest: {guest}", "Outro: {title}"],
            "title_pattern": "^(?P<title>.+) with (?P<guest>.+)$"
        }"#;
        serde_json::from_str(written).unwrap()
    }

    fn cut(start: f64, end: Option<f64>) -> Cut {
        Cut {
            start: Time(start),
            end: end.map(Time),
            reason: String::new(),
            fade_out: None,
            fade_in: None,
        }
    }

    fn named(title: &str) -> EpisodeName<'_> {
        EpisodeName { title, season: Some(9), episode: Some(8) }
    }

    #[test]
    fn each_part_starts_where_the_cut_before_it_ends() {
        // Theme, two classified breaks, outro.
        let cuts = [
            cut(0.0, Some(49.1)),
            cut(1569.2, Some(1604.8)),
            cut(4588.7, Some(4670.4)),
            cut(5543.8, None),
        ];
        let chapters =
            chapters(&scheme(), &named("Non-Rabbie Behavior with Joe Wengert"), &cuts).unwrap();
        let laid_out: Vec<(f64, &str)> = chapters
            .iter()
            .map(|chapter| (chapter.start.seconds(), chapter.title.as_str()))
            .collect();
        assert_eq!(
            laid_out,
            [
                (49.1, "Intro: Non-Rabbie Behavior"),
                (1604.8, "Guest: Joe Wengert"),
                (4670.4, "Outro: Non-Rabbie Behavior")
            ]
        );
    }

    #[test]
    fn an_episode_the_layout_does_not_fit_is_explained() {
        let no_breaks =
            chapters(&scheme(), &named("Retreat with Everyone"), &[cut(0.0, Some(14.5))])
                .unwrap_err();
        assert!(no_breaks.contains("0 breaks split it into 1 parts"), "{no_breaks}");
        let two_breaks = [cut(0.0, Some(14.5)), cut(100.0, Some(200.0)), cut(300.0, Some(400.0))];
        let no_guest =
            chapters(&scheme(), &named("Season 10 Pre-Season Retreat"), &two_breaks).unwrap_err();
        assert!(no_guest.contains("title_pattern"), "{no_guest}");
    }
}
