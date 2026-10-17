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
    problems.extend(transcription_problems(config));
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
        for recording in show.themes.iter().chain(&show.outros).chain(&show.bumpers) {
            problems.extend(missing(&format!("shows.{slug}.themes/outros/bumpers"), Some(recording.as_path())));
        }
        if show.feeds.is_empty() {
            problems.push(format!("shows.{slug}.feeds: no feeds to sync from"));
        }
    }
    problems
}

/// An engine this machine cannot run.
fn transcription_problems(config: &Config) -> Vec<String> {
    let engine = config.transcription.engine;
    if engine == TranscriptionEngine::MlxWhisper && TranscriptionEngine::native() != TranscriptionEngine::MlxWhisper {
        return vec!["transcription.engine: mlx-whisper runs only on Apple silicon; use \"faster-whisper\"".into()];
    }
    Vec::new()
}

fn missing(setting: &str, path: Option<&Path>) -> Option<String> {
    let path = path.filter(|path| !path.exists())?;
    Some(format!("{setting}: {} does not exist", path.display()))
}

/// Everything supercut shells out to, whether or not this config needs it.
fn tool_paths(config: &Config) -> BTreeMap<String, Option<PathBuf>> {
    let mut programs: Vec<String> =
        ["ffmpeg", "ffprobe", "mlx_whisper", "whisper-ctranslate2"].into_iter().map(str::to_owned).collect();
    programs.extend(config.jev.api_key_command.first().cloned());
    programs.into_iter().map(|program| (program.clone(), locate(&program))).collect()
}

fn tool_problems(config: &Config, tools: &BTreeMap<String, Option<PathBuf>>) -> Vec<String> {
    let transcriber = config.transcription.engine.program();
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
    std::env::split_paths(&path).flat_map(|folder| candidates(&folder, program)).find(|candidate| executable(candidate))
}

#[cfg(unix)]
fn candidates(folder: &Path, program: &str) -> Vec<PathBuf> {
    vec![folder.join(program)]
}

/// Windows finds `ffmpeg` as `ffmpeg.exe`: a program is named without the extensions PATHEXT lists.
#[cfg(not(unix))]
fn candidates(folder: &Path, program: &str) -> Vec<PathBuf> {
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT".into());
    let named = extensions.split(';').filter(|extension| !extension.is_empty());
    std::iter::once(folder.join(program)).chain(named.map(|extension| folder.join(format!("{program}{extension}")))).collect()
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
