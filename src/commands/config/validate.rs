use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::commands::Context;
use crate::config::{Config, TranscriptionEngine};
use crate::output::print_json;

/// Check the config file, the paths it names and the external tools supercut needs.
///
/// Reports every setting that points at something missing and where each external program was
/// found, so a failing run can be explained before it starts. Changes nothing.
#[derive(clap::Args)]
pub struct Args {}

#[derive(Serialize)]
struct Report {
    /// True when nothing below needs fixing.
    ok: bool,
    /// One sentence per thing to fix, naming the setting and the path.
    problems: Vec<String>,
    /// Where each program was found on `PATH`, or null when it is missing.
    tools: BTreeMap<String, Option<PathBuf>>,
}

pub fn run(context: &Context, _args: Args) -> anyhow::Result<()> {
    let config = &context.config;
    let tools = tool_paths(config);
    let mut problems = Vec::new();
    if !context.config_path.exists() {
        problems.push(format!("no config file at {}; `supercut config init` writes one", context.config_path.display()));
    }
    problems.extend(path_problems(config));
    problems.extend(tool_problems(config, &tools));
    print_json(&Report { ok: problems.is_empty(), problems, tools })
}

fn path_problems(config: &Config) -> Vec<String> {
    let mut problems = Vec::new();
    problems.extend(missing("output_dir", config.output_dir.as_deref()));
    problems.extend(missing("sideload_dir", config.sideload_dir.as_deref()));
    for (slug, show) in &config.shows {
        problems.extend(missing(&format!("shows.{slug}.artwork"), show.artwork.as_deref()));
        problems.extend(missing(&format!("shows.{slug}.interstitial"), show.interstitial.as_deref()));
        if show.feeds.is_empty() {
            problems.push(format!("shows.{slug}.feeds: no feeds to sync from"));
        }
    }
    problems
}

fn missing(setting: &str, path: Option<&Path>) -> Option<String> {
    let path = path.filter(|path| !path.exists())?;
    Some(format!("{setting}: {} does not exist", path.display()))
}

/// Everything supercut shells out to, whether or not this config needs it.
fn tool_paths(config: &Config) -> BTreeMap<String, Option<PathBuf>> {
    let mut programs = vec!["ffmpeg".to_owned(), "ffprobe".to_owned(), "mlx_whisper".to_owned(), "whisper-cli".to_owned()];
    programs.extend(config.jev.api_key_command.first().cloned());
    programs.into_iter().map(|program| (program.clone(), locate(&program))).collect()
}

fn tool_problems(config: &Config, tools: &BTreeMap<String, Option<PathBuf>>) -> Vec<String> {
    let transcriber = match config.transcription.engine {
        TranscriptionEngine::MlxWhisper => "mlx_whisper",
        TranscriptionEngine::WhisperCpp => "whisper-cli",
    };
    let mut needed = vec!["ffmpeg", "ffprobe", transcriber];
    needed.extend(config.jev.api_key_command.first().map(String::as_str));
    needed
        .into_iter()
        .filter(|program| tools.get(*program).is_none_or(Option::is_none))
        .map(|program| format!("{program} is not on PATH"))
        .collect()
}

fn locate(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|folder| folder.join(program)).find(|candidate| executable(candidate))
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|data| data.is_file() && data.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable(path: &Path) -> bool {
    path.is_file()
}
