use std::fmt::Write as _;

use crate::commands::Context;
use crate::plan::timeline::{EpisodeSpan, InputEpisode, Timeline};
use crate::plan::{Cut, Plan, resolve, timeline};
use crate::timecode::Time;
use crate::transcript::Transcript;

/// Words of transcript quoted either side of a cut edge.
const CONTEXT_WORDS: usize = 12;

/// The plan as prose for the human who decides whether these cuts are right.
pub fn review(context: &Context, plan: &Plan) -> anyhow::Result<String> {
    let resolved = resolve::resolve(context, plan)?;
    let timeline = timeline::build(&resolved.input);

    let mut page = String::new();
    writeln!(page, "# {}\n", plan.title)?;
    writeln!(
        page,
        "{} · {} episodes · {} of audio\n",
        resolved.show.title,
        timeline.episodes.len(),
        Time(timeline.duration())
    )?;

    for (span, episode) in timeline.episodes.iter().zip(&resolved.input.episodes) {
        let transcript = resolved.transcript(&episode.id)?;
        write_episode(&mut page, span, episode, transcript.as_ref())?;
    }
    write_totals(&mut page, &timeline, &resolved.input.episodes)?;
    Ok(page)
}

fn write_episode(
    page: &mut String,
    span: &EpisodeSpan,
    episode: &InputEpisode,
    transcript: Option<&Transcript>,
) -> anyhow::Result<()> {
    writeln!(page, "## {} — {}\n", span.id, span.title)?;
    writeln!(
        page,
        "Kept {} of {} · {} cuts · plays {} to {}\n",
        Time(span.kept_seconds),
        Time(episode.duration),
        span.cuts,
        Time(span.start),
        Time(span.end)
    )?;

    for (index, cut) in episode.cuts.iter().enumerate() {
        write_cut(page, index, cut, episode.duration, transcript)?;
    }
    if span.chapters.is_empty() {
        return Ok(());
    }
    writeln!(page, "### Chapters\n")?;
    for chapter in &span.chapters {
        let description = chapter.description.as_deref().map(summarize).unwrap_or_default();
        writeln!(page, "- {} {} {description}", Time(chapter.start), chapter.title)?;
    }
    writeln!(page)?;
    Ok(())
}

fn write_cut(
    page: &mut String,
    index: usize,
    cut: &Cut,
    duration: f64,
    transcript: Option<&Transcript>,
) -> anyhow::Result<()> {
    let end = cut.end.map_or(duration, Time::seconds);
    let start = cut.start.seconds();
    writeln!(
        page,
        "### Cut {} · {} to {} ({}) · {}\n",
        index + 1,
        Time(start),
        Time(end),
        Time(end - start),
        cut.reason
    )?;
    writeln!(page, "Fades: out {} s, in {} s\n", fade(cut.fade_out), fade(cut.fade_in))?;
    let Some(transcript) = transcript else { return Ok(()) };
    for (label, text) in quotes(transcript, start, end) {
        writeln!(page, "- {label}: {text}")?;
    }
    writeln!(page)?;
    Ok(())
}

/// What the listener hears either side of the splice, and what disappears between.
fn quotes(transcript: &Transcript, start: f64, end: f64) -> Vec<(&'static str, String)> {
    let before: Vec<&str> = words_before(transcript, start);
    let inside: Vec<&str> = transcript.words_between(start, end).map(|word| word.text.as_str()).collect();
    let after: Vec<&str> =
        transcript.words().filter(|word| word.start >= end).take(CONTEXT_WORDS).map(|word| word.text.as_str()).collect();

    let mut quotes = vec![("before", quote(&before, ""))];
    if inside.len() <= CONTEXT_WORDS * 2 {
        quotes.push(("cut", quote(&inside, "")));
    } else {
        quotes.push(("cut starts", quote(&inside[..CONTEXT_WORDS], " …")));
        quotes.push(("cut ends", quote(&inside[inside.len() - CONTEXT_WORDS..], "")));
    }
    quotes.push(("after", quote(&after, " …")));
    quotes
}

fn words_before<'a>(transcript: &'a Transcript, start: f64) -> Vec<&'a str> {
    let mut words: Vec<&str> = Vec::new();
    for word in transcript.words().take_while(|word| word.end <= start) {
        if words.len() == CONTEXT_WORDS {
            words.remove(0);
        }
        words.push(word.text.as_str());
    }
    words
}

fn quote(words: &[&str], trailing: &str) -> String {
    if words.is_empty() {
        return "(silence)".to_string();
    }
    format!("“{}{trailing}”", words.join(" "))
}

fn write_totals(page: &mut String, timeline: &Timeline, episodes: &[InputEpisode]) -> anyhow::Result<()> {
    let source: f64 = episodes.iter().map(|episode| episode.duration).sum();
    let kept: f64 = timeline.episodes.iter().map(|span| span.kept_seconds).sum();
    let cuts: usize = timeline.episodes.iter().map(|span| span.cuts).sum();
    writeln!(page, "## Totals\n")?;
    writeln!(page, "- Episodes: {}", episodes.len())?;
    writeln!(page, "- Cuts: {cuts}, removing {}", Time(source - kept))?;
    writeln!(page, "- Kept audio: {}", Time(kept))?;
    writeln!(page, "- Rendered length: {} (with interstitials)", Time(timeline.duration()))?;
    Ok(())
}

fn fade(seconds: Option<f64>) -> String {
    seconds.map_or_else(|| format!("{} (default)", crate::plan::EDGE_FADE_SECONDS), |fade| format!("{fade}"))
}

/// Show notes are paragraphs; a chapter line wants one sentence of them.
fn summarize(description: &str) -> String {
    let mut text: String = description.split_whitespace().take(16).collect::<Vec<_>>().join(" ");
    if text.len() < description.len() {
        text.push('…');
    }
    if text.is_empty() { text } else { format!("— {text}") }
}
