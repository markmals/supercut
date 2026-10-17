use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::commands::Context;
use crate::config::{Config, LoudnessConfig, SegmentKind, ShowConfig, TranscriptionConfig};
use crate::output::print_json;

/// Show the effective configuration and where supercut keeps its files, secrets redacted.
///
/// Start here: it names the config file, the library, the output and sideload folders, the
/// rules folder and every configured show. Feed URLs and API keys are never printed.
#[derive(clap::Args)]
pub struct Args {}

#[derive(Serialize)]
struct Report<'a> {
    config_file: &'a Path,
    /// False until `config init` or you write the file yourself.
    exists: bool,
    library_dir: &'a Path,
    output_dir: Option<&'a Path>,
    sideload_dir: Option<&'a Path>,
    /// Holds `<show>.md`, the editorial rules `rules get` prints.
    rules_dir: PathBuf,
    /// Every configured show and the settings rendering uses (feed URLs left out).
    shows: Vec<Show<'a>>,
    transcription: &'a TranscriptionConfig,
    loudness: &'a LoudnessConfig,
    jev: Jev<'a>,
}

/// The Jev settings worth showing: the key itself stays where it is.
#[derive(Serialize)]
struct Jev<'a> {
    endpoint: &'a str,
    model: &'a str,
    concurrency: usize,
    /// The program that prints the API key, e.g. `op`.
    api_key_program: Option<&'a str>,
}

#[derive(Serialize)]
struct Show<'a> {
    show: &'a str,
    title: &'a str,
    feeds: usize,
    artwork: Option<&'a Path>,
    /// Played between episodes in every supercut of the show.
    interstitial: Option<&'a Path>,
    /// Theme songs and jingles whose end marks where each episode's opening cut ends.
    themes: &'a [std::path::PathBuf],
    /// The first notes of the music leading into the credits; each episode is cut from there.
    outros: &'a [std::path::PathBuf],
    /// Closing jingles: an episode ending with one keeps it and no interstitial follows it.
    bumpers: &'a [std::path::PathBuf],
    /// The segments `labels create` sorts episodes into; `cut` ones `plans draft` removes.
    segments: &'a [SegmentKind],
}

fn show<'a>(slug: &'a str, settings: &'a ShowConfig) -> Show<'a> {
    Show {
        show: slug,
        title: &settings.title,
        feeds: settings.feeds.len(),
        artwork: settings.artwork.as_deref(),
        interstitial: settings.interstitial.as_deref(),
        themes: &settings.themes,
        outros: &settings.outros,
        bumpers: &settings.bumpers,
        segments: &settings.segments,
    }
}

pub fn run(context: &Context, _args: Args) -> anyhow::Result<()> {
    let config = &context.config;
    print_json(&Report {
        config_file: &context.config_path,
        exists: context.config_path.exists(),
        library_dir: &context.library_dir,
        output_dir: config.output_dir.as_deref(),
        sideload_dir: config.sideload_dir.as_deref(),
        rules_dir: rules_dir(context),
        shows: config.shows.iter().map(|(slug, settings)| show(slug, settings)).collect(),
        transcription: &config.transcription,
        loudness: &config.loudness,
        jev: jev(config),
    })
}

/// Where `rules get` looks, taken from the path it builds for a show.
fn rules_dir(context: &Context) -> PathBuf {
    context.rules_path("show").parent().map(Path::to_path_buf).unwrap_or_default()
}

fn jev(config: &Config) -> Jev<'_> {
    Jev {
        endpoint: &config.jev.endpoint,
        model: &config.jev.model,
        concurrency: config.jev.concurrency,
        api_key_program: config.jev.api_key_command.first().map(String::as_str),
    }
}
