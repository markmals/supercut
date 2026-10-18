use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::commands::Context;
use crate::config::{SCHEMA_FILE, schema};
use crate::error::CliError;
use crate::library::write_atomically;
use crate::output::print_json;

const STARTER: &str = include_str!("starter-config.jsonc");

/// Write a commented starter config file at the config path, and its JSON Schema beside it.
///
/// The starter is JSONC (JSON with comments and trailing commas) and shows every setting with a
/// comment: library, output and publish folders, the transcription engine, loudness targets,
/// the Jev model, and one example show with feeds, recordings, segments and a chapter layout.
/// Its `$schema` points at `supercut.config.schema.json`, written next to it, so editors check
/// it as you type. Edit it, then check it with `config validate`. Refuses to overwrite a config
/// that already exists unless `--force` says so.
#[derive(clap::Args)]
pub struct Args {
    /// Report where the files would go, write nothing.
    #[arg(long)]
    dry_run: bool,
    /// Overwrite the config file that is already there.
    #[arg(long)]
    force: bool,
}

#[derive(Serialize)]
struct Receipt<'a> {
    config_file: &'a Path,
    schema_file: PathBuf,
    action: Action,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum Action {
    Written,
    WouldWrite,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let path = &context.config_path;
    if path.exists() && !args.force {
        let error = CliError::conflict(format!("{} already exists", path.display()));
        return Err(error.hint("supercut config init --force").into());
    }
    let schema_file = path.with_file_name(SCHEMA_FILE);
    if args.dry_run {
        return print_json(&Receipt { config_file: path, schema_file, action: Action::WouldWrite });
    }
    write_atomically(path, STARTER.as_bytes())?;
    write_atomically(&schema_file, format!("{:#}\n", schema()).as_bytes())?;
    print_json(&Receipt { config_file: path, schema_file, action: Action::Written })
}
