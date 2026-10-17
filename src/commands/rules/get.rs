use std::path::PathBuf;

use anyhow::Context as _;
use serde::Serialize;

use crate::commands::Context;
use crate::error::CliError;
use crate::output::print_json;

/// Print a show's editorial rules and the path of the rules file.
///
/// The rules are plain Markdown you write by hand: the show's segments, what to cut, how
/// transitions should sound. Read them before drafting a plan. The file lives in
/// `rules/<show>.md` next to the config file; when it is missing this fails and names the path.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as it appears in the config file.
    show: String,
}

#[derive(Serialize)]
struct Rules<'a> {
    show: &'a str,
    path: PathBuf,
    /// The file, as Markdown.
    rules: String,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    context.config.show(&args.show)?;
    let path = context.rules_path(&args.show);
    if !path.exists() {
        let error = CliError::not_found(format!("no editorial rules for `{}`", args.show))
            .detail("path", path.display().to_string())
            .hint(format!("write them in {}", path.display()));
        return Err(error.into());
    }
    let rules = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    print_json(&Rules { show: &args.show, path, rules })
}
