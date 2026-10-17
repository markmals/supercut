use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::commands::Context;
use crate::config::{Config, LoudnessConfig, TranscriptionConfig};
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
    /// Slugs every command takes.
    shows: Vec<&'a str>,
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

pub fn run(context: &Context, _args: Args) -> anyhow::Result<()> {
    let config = &context.config;
    print_json(&Report {
        config_file: &context.config_path,
        exists: context.config_path.exists(),
        library_dir: &context.library_dir,
        output_dir: config.output_dir.as_deref(),
        sideload_dir: config.sideload_dir.as_deref(),
        rules_dir: rules_dir(context),
        shows: config.shows.keys().map(String::as_str).collect(),
        transcription: &config.transcription,
        loudness: &config.loudness,
        jev: jev(config),
    })
}

/// Where `rules get` looks, taken from the path it builds for a show.
pub fn rules_dir(context: &Context) -> PathBuf {
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
