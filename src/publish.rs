//! Publishing a rendered supercut: a copy in each configured folder, named after the supercut's
//! title, since podcast apps that take sideloaded files (Castro) show the file name as the
//! episode's title.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Serialize;

use crate::commands::Context;
use crate::error::CliError;
use crate::library::partial_path;
use crate::plan::Plan;
use crate::render::supercut::refuse_to_overwrite;

/// What `plans publish` copied, or would copy on a dry run.
#[derive(Debug, Serialize)]
pub struct Published {
    pub source: PathBuf,
    pub copies: Vec<PathBuf>,
    pub dry_run: bool,
}

/// Copies the rendered `source` of `plan` into every configured publish folder.
pub fn publish(
    context: &Context,
    plan: &Plan,
    source: PathBuf,
    force: bool,
    dry_run: bool,
) -> anyhow::Result<Published> {
    let folders = &context.config.publish_folders;
    if folders.is_empty() {
        let missing = CliError::precondition("`publish_folders` is not set in the config file");
        return Err(missing.hint("supercut config get").into());
    }
    if !source.is_file() {
        return Err(CliError::precondition(format!("{} is not rendered", source.display()))
            .hint("supercut plans render <plan>")
            .detail("path", source.display().to_string())
            .into());
    }
    let extension = source.extension().unwrap_or_default().to_string_lossy();
    let name = file_name(&plan.title, &extension);
    let copies: Vec<PathBuf> = folders.iter().map(|folder| folder.join(&name)).collect();
    for copy in &copies {
        refuse_to_overwrite(copy, force)?;
    }
    if !dry_run {
        for copy in &copies {
            copy_atomically(&source, copy)?;
        }
    }
    Ok(Published { source, copies, dry_run })
}

/// `title` as a file name every platform accepts: "Show: Part 2" becomes "Show - Part 2", and
/// other characters Windows or macOS reserve become hyphens.
fn file_name(title: &str, extension: &str) -> String {
    let readable = title.replace(": ", " - ");
    let safe: String = readable
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            character if character.is_control() => '-',
            character => character,
        })
        .collect();
    let stem = safe.trim().trim_end_matches('.');
    if extension.is_empty() { stem.to_owned() } else { format!("{stem}.{extension}") }
}

fn copy_atomically(source: &Path, destination: &Path) -> anyhow::Result<()> {
    let folder = destination.parent().expect("a copy has a folder");
    std::fs::create_dir_all(folder).with_context(|| format!("creating {}", folder.display()))?;
    let partial = partial_path(destination);
    std::fs::copy(source, &partial)
        .with_context(|| format!("copying {} to {}", source.display(), partial.display()))?;
    std::fs::rename(&partial, destination)
        .with_context(|| format!("moving {} into place", destination.display()))
}

#[cfg(test)]
mod tests {
    use super::file_name;

    #[test]
    fn a_title_becomes_a_file_name_any_platform_accepts() {
        assert_eq!(
            file_name("The Neighborhood Listen: Season 9, Part 2", "mp3"),
            "The Neighborhood Listen - Season 9, Part 2.mp3"
        );
        assert_eq!(file_name("AC/DC: Live at Donington?", "mp3"), "AC-DC - Live at Donington-.mp3");
        assert_eq!(file_name("Trailing dots...", "mp3"), "Trailing dots.mp3");
    }
}
