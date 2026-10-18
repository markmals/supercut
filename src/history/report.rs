//! What a recovered supercut says about the editor's taste: per episode, what was cut, how it
//! was faded, and what was being said in each cut.

use serde::Serialize;

use super::Recovery;
use crate::library::{EpisodeIndex, ShowLibrary};
use crate::plan::Plan;
use crate::timecode::Time;
use crate::transcript::Transcript;

/// Words quoted at each end of a cut.
const QUOTE_WORDS: usize = 30;

#[derive(Serialize)]
pub struct Report {
    pub name: String,
    pub export: String,
    pub duration: Time,
    pub episodes: Vec<EpisodeReport>,
    pub interstitials: Vec<InterstitialReport>,
}

#[derive(Serialize)]
pub struct EpisodeReport {
    pub id: String,
    pub title: String,
    pub kept_seconds: f64,
    pub cut_seconds: f64,
    pub transcribed: bool,
    pub cuts: Vec<CutReport>,
}

#[derive(Serialize)]
pub struct CutReport {
    pub start: Time,
    /// Absent: to the end of the episode.
    pub end: Option<Time>,
    pub seconds: f64,
    pub fade_out: Option<f64>,
    pub fade_in: Option<f64>,
    /// Last words kept before the cut.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// How the cut material opens and closes, and how many words it holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opens: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<usize>,
    /// First words kept after the cut.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(Serialize)]
pub struct InterstitialReport {
    pub at: Time,
    pub seconds: f64,
    pub sources: Vec<String>,
}

pub fn build(
    show: &ShowLibrary,
    index: &EpisodeIndex,
    recovery: &Recovery,
    plan: &Plan,
) -> anyhow::Result<Report> {
    let mut episodes = Vec::new();
    for planned in &plan.episodes {
        let episode = index.get(&planned.id)?;
        let duration = crate::analysis::episode_analysis(show, episode)?.duration;
        let transcript: Option<Transcript> = {
            let path = show.transcript_path(&planned.id);
            if path.exists() { Some(crate::library::read_json(&path)?) } else { None }
        };
        let cuts: Vec<CutReport> =
            planned.cuts.iter().map(|cut| cut_report(cut, duration, transcript.as_ref())).collect();
        let cut_seconds = cuts.iter().map(|cut| cut.seconds).sum::<f64>();
        episodes.push(EpisodeReport {
            id: planned.id.clone(),
            title: episode.title.clone(),
            kept_seconds: round(duration - cut_seconds),
            cut_seconds: round(cut_seconds),
            transcribed: transcript.is_some(),
            cuts,
        });
    }
    let interstitials = recovery
        .interstitials
        .iter()
        .map(|interstitial| InterstitialReport {
            at: Time(interstitial.export_start),
            seconds: round(interstitial.export_end - interstitial.export_start),
            sources: interstitial
                .sources
                .iter()
                .map(|source| {
                    format!(
                        "{} {}-{}",
                        source.episode,
                        Time(source.source_start),
                        Time(source.source_end)
                    )
                })
                .collect(),
        })
        .collect();
    Ok(Report {
        name: recovery.name.clone(),
        export: recovery.export.clone(),
        duration: Time(recovery.duration),
        episodes,
        interstitials,
    })
}

fn cut_report(cut: &crate::plan::Cut, duration: f64, transcript: Option<&Transcript>) -> CutReport {
    let start = cut.start.seconds();
    let end = cut.end.map_or(duration, Time::seconds);
    let mut report = CutReport {
        start: cut.start,
        end: cut.end,
        seconds: round(end - start),
        fade_out: cut.fade_out,
        fade_in: cut.fade_in,
        before: None,
        opens: None,
        closes: None,
        words: None,
        after: None,
    };
    let Some(transcript) = transcript else { return report };
    let inside: Vec<&str> =
        transcript.words_between(start, end).map(|word| word.text.as_str()).collect();
    report.before = Some(last_words(
        transcript
            .words_between((start - 30.0).max(0.0), start)
            .map(|word| word.text.as_str())
            .collect(),
    ));
    report.after = Some(
        transcript
            .words_between(end, end + 30.0)
            .take(QUOTE_WORDS / 2)
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    );
    report.opens = Some(inside.iter().take(QUOTE_WORDS).copied().collect::<Vec<_>>().join(" "));
    if inside.len() > QUOTE_WORDS {
        report.closes = Some(last_words(inside.clone()));
    }
    report.words = Some(inside.len());
    report
}

fn last_words(words: Vec<&str>) -> String {
    words[words.len().saturating_sub(QUOTE_WORDS / 2)..].join(" ")
}

/// Markdown for reading the report.
pub fn markdown(report: &Report) -> String {
    let mut text = format!(
        "# {}\n\n{} long, {} episodes\n",
        report.name,
        report.duration,
        report.episodes.len()
    );
    for episode in &report.episodes {
        text.push_str(&format!(
            "\n## {} — {}\n\nkept {}, cut {} in {} cuts{}\n",
            episode.id,
            episode.title,
            Time(episode.kept_seconds),
            Time(episode.cut_seconds),
            episode.cuts.len(),
            if episode.transcribed { "" } else { " (not transcribed: no cut text)" }
        ));
        for cut in &episode.cuts {
            text.push_str(&cut_markdown(cut));
        }
    }
    text.push_str("\n## Interstitials\n\n");
    for interstitial in &report.interstitials {
        text.push_str(&format!(
            "- at {} ({:.1} s): {}\n",
            interstitial.at,
            interstitial.seconds,
            interstitial.sources.join(", ")
        ));
    }
    text
}

fn cut_markdown(cut: &CutReport) -> String {
    let end = cut.end.map_or_else(|| "end".to_owned(), |end| end.to_string());
    let fades = format!(
        "{}{}",
        cut.fade_out.map(|fade| format!(", fade-out {fade:.2} s")).unwrap_or_default(),
        cut.fade_in.map(|fade| format!(", fade-in {fade:.2} s")).unwrap_or_default()
    );
    let mut text = format!("\n- **cut {}–{}** ({:.1} s{fades})\n", cut.start, end, cut.seconds);
    let quotes = [
        ("before", &cut.before),
        ("opens", &cut.opens),
        ("closes", &cut.closes),
        ("after", &cut.after),
    ];
    for (label, quote) in quotes {
        if let Some(quote) = quote.as_deref().filter(|quote| !quote.is_empty()) {
            text.push_str(&format!("  - {label}: “{quote}”\n"));
        }
    }
    if let Some(words) = cut.words {
        text.push_str(&format!("  - {words} words cut\n"));
    }
    text
}

fn round(seconds: f64) -> f64 {
    (seconds * 1000.0).round() / 1000.0
}
