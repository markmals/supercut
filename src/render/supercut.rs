use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Serialize;

use crate::commands::Context;
use crate::error::CliError;
use crate::library::partial_path;
use crate::plan::resolve::Resolved;
use crate::plan::timeline::Timeline;
use crate::plan::{Plan, resolve, timeline};
use crate::render::tags::Tags;
use crate::render::{encode, graph, tags};
use crate::timecode::Time;

pub struct Request<'a> {
    pub plan: &'a Plan,
    /// Where to write instead of `<output_dir>/<show>/<the plan's output name>`.
    pub out: Option<PathBuf>,
    pub force: bool,
    pub dry_run: bool,
}

/// The receipt of a render: what was written, how long it is and what is in it.
#[derive(Debug, Serialize)]
pub struct Rendered {
    pub output: PathBuf,
    pub duration: Time,
    /// Absent on a dry run, which encodes nothing.
    pub bytes: Option<u64>,
    pub bit_rate: u64,
    pub chapters: usize,
    pub dry_run: bool,
}

pub fn render(context: &Context, request: Request) -> anyhow::Result<Rendered> {
    let resolved = resolve::resolve(context, request.plan)?;
    crate::plan::check::require_valid_cuts(&resolved)?;
    let timeline = timeline::build(&resolved.input);
    for warning in &timeline.warnings {
        eprintln!("warning: {warning}");
    }

    let output = match &request.out {
        Some(path) => path.clone(),
        None => default_output(context, request.plan)?,
    };
    refuse_to_overwrite(&output, request.force)?;

    // An episode without segment chapters is still one chapter of the file.
    let chapters = timeline.episodes.iter().map(|span| span.chapters.len().max(1)).sum();
    let planned = Rendered {
        output,
        duration: Time(timeline.duration()),
        bytes: None,
        bit_rate: resolved.format.bit_rate,
        chapters,
        dry_run: request.dry_run,
    };
    if request.dry_run {
        return Ok(planned);
    }
    write_file(context, &resolved, &timeline, &request.plan.title, planned)
}

fn write_file(
    context: &Context,
    resolved: &Resolved,
    timeline: &Timeline,
    title: &str,
    planned: Rendered,
) -> anyhow::Result<Rendered> {
    let partial = partial_path(&planned.output);
    let parent = planned.output.parent().expect("an output file has a folder");
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    let limit_db = context.config.loudness.true_peak_db;
    let args = graph::encode_args(&timeline.pieces, &resolved.format, limit_db, &partial);
    encode::run(&args, timeline.duration(), "rendering")?;

    let tags = Tags {
        title,
        show_title: &resolved.show.title,
        artwork: resolved.show.artwork.as_deref(),
        episodes: &timeline.episodes,
    };
    let chapters = tags::write(&partial, &tags)?;
    std::fs::rename(&partial, &planned.output)
        .with_context(|| format!("moving {} into place", planned.output.display()))?;

    let probe = crate::media::probe(&planned.output)?;
    let bytes = std::fs::metadata(&planned.output)
        .with_context(|| format!("reading {}", planned.output.display()))?
        .len();
    Ok(Rendered {
        duration: Time(probe.duration),
        bytes: Some(bytes),
        bit_rate: probe.bit_rate.unwrap_or(planned.bit_rate),
        chapters,
        ..planned
    })
}

/// Where `plans render` writes a plan's supercut unless told otherwise:
/// `<output_dir>/<show>/<the plan's output name>`.
pub fn default_output(context: &Context, plan: &Plan) -> anyhow::Result<PathBuf> {
    Ok(context.config.output_dir()?.join(&plan.show).join(&plan.output))
}

/// Refuses to replace a finished file unless `force` says so.
pub fn refuse_to_overwrite(path: &Path, force: bool) -> anyhow::Result<()> {
    if force || !path.exists() {
        return Ok(());
    }
    let conflict = CliError::conflict(format!("{} already exists", path.display()))
        .detail("path", path.display().to_string())
        .hint("pass --force to overwrite it");
    Err(conflict.into())
}
