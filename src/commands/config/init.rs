use serde::Serialize;

use crate::commands::Context;
use crate::error::CliError;
use crate::library::write_atomically;
use crate::output::print_json;

const STARTER: &str = include_str!("../starter-config.toml");

/// Write a commented starter config file at the config path.
///
/// The starter shows every setting with a comment: library, output and sideload folders, the
/// transcription engine, loudness targets, the Jev model, and one example show with feeds,
/// artwork, interstitial and segments. Edit it, then check it with `config validate`. Refuses
/// to overwrite a config that already exists unless `--force` says so.
#[derive(clap::Args)]
pub struct Args {
    /// Report where the file would go, write nothing.
    #[arg(long)]
    dry_run: bool,
    /// Overwrite the config file that is already there.
    #[arg(long)]
    force: bool,
}

#[derive(Serialize)]
struct Receipt<'a> {
    config_file: &'a std::path::Path,
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
    if args.dry_run {
        return print_json(&Receipt { config_file: path, action: Action::WouldWrite });
    }
    write_atomically(path, STARTER.as_bytes())?;
    print_json(&Receipt { config_file: path, action: Action::Written })
}
