use std::fmt::{self, Write as _};

use super::{CutReview, EpisodeReview, Review};

/// The review as Markdown, for the human who decides whether these cuts are right.
pub fn markdown(review: &Review) -> String {
    let mut page = String::new();
    write_review(&mut page, review).expect("writing to a String cannot fail");
    page
}

fn write_review(page: &mut String, review: &Review) -> fmt::Result {
    writeln!(page, "# {}\n", review.title)?;
    writeln!(
        page,
        "{} · {} episodes · {} after cuts\n",
        review.show, review.totals.episodes, review.totals.output_duration
    )?;
    for episode in &review.episodes {
        write_episode(page, episode)?;
    }
    writeln!(page, "## Totals\n")?;
    writeln!(page, "- Episodes: {}", review.totals.episodes)?;
    writeln!(page, "- Cuts: {}, removing {}", review.totals.cuts, review.totals.removed)?;
    writeln!(page, "- Kept audio: {}", review.totals.kept)?;
    writeln!(page, "- Rendered length: {} (interstitials included)", review.totals.output_duration)
}

fn write_episode(page: &mut String, episode: &EpisodeReview) -> fmt::Result {
    writeln!(page, "## {} — {}\n", episode.id, episode.title)?;
    writeln!(
        page,
        "Kept {} of {} · {} · plays at {} · {}\n",
        episode.kept,
        episode.duration,
        cuts(episode.cuts.len()),
        episode.plays_at,
        if episode.interstitial_after {
            "the interstitial follows"
        } else {
            "no interstitial follows"
        }
    )?;
    for cut in &episode.cuts {
        write_cut(page, cut)?;
    }
    if episode.chapters.is_empty() {
        return Ok(());
    }
    writeln!(page, "### Chapters\n")?;
    for chapter in &episode.chapters {
        let description = match chapter.description.as_deref() {
            Some(description) if !description.is_empty() => format!(": {description}"),
            _ => String::new(),
        };
        writeln!(page, "- {} to {} — {}{description}", chapter.start, chapter.end, chapter.title)?;
    }
    writeln!(page)
}

fn write_cut(page: &mut String, cut: &CutReview) -> fmt::Result {
    writeln!(page, "### Cut {} · {} to {} ({})\n", cut.index, cut.start, cut.end, cut.length)?;
    writeln!(page, "{}  ", cut.reason)?;
    writeln!(page, "Fades: {} s out, {} s in\n", cut.fade_out, cut.fade_in)?;
    let Some(context) = &cut.context else { return Ok(()) };
    writeln!(page, "- heard before: {}", quote(&context.before, ""))?;
    if context.cut_ends.is_empty() {
        writeln!(page, "- cut: {}", quote(&context.cut_starts, ""))?;
    } else {
        writeln!(page, "- cut starts: {}", quote(&context.cut_starts, " …"))?;
        writeln!(page, "- cut ends: {}", quote(&context.cut_ends, ""))?;
    }
    writeln!(page, "- heard after: {}\n", quote(&context.after, " …"))
}

fn cuts(count: usize) -> String {
    match count {
        1 => "1 cut".to_string(),
        other => format!("{other} cuts"),
    }
}

fn quote(text: &str, trailing: &str) -> String {
    if text.is_empty() {
        return "(silence)".to_string();
    }
    format!("“{text}{trailing}”")
}
