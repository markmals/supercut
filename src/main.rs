mod analysis;
mod commands;
mod config;
mod feed;
mod history;
mod labels;
mod library;
mod media;
mod output;
mod plan;
mod render;
mod timecode;
mod transcribe;
mod transcript;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::commands::Context;

/// Build podcast supercuts: many episodes stitched into one long MP3, minus the parts you skip.
///
/// Workflow: `sync` a show's feeds, `download` and `transcribe` episodes, write a cut plan
/// (JSON) from the transcripts and the show's rules, `plan check` it, `preview` the transitions,
/// then `render`. `history` recovers the cuts of supercuts you made by hand, as examples.
///
/// Data commands print JSON on stdout; reading commands (transcript, inspect, plan review)
/// print text. Progress goes to stderr.
#[derive(Parser)]
#[command(name = "supercut", version)]
struct Cli {
    /// Config file to use instead of the platform default.
    #[arg(long, global = true, env = "SUPERCUT_CONFIG")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Config(commands::config::Args),
    Rules(commands::rules::Args),
    Sync(commands::sync::Args),
    Episodes(commands::episodes::Args),
    Download(commands::download::Args),
    Adopt(commands::adopt::Args),
    Transcribe(commands::transcribe::Args),
    Transcript(commands::transcript::Args),
    Analyze(commands::analyze::Args),
    Inspect(commands::inspect::Args),
    Label(commands::label::Args),
    Segments(commands::segments::Args),
    Draft(commands::draft::Args),
    Plan(commands::plan::Args),
    Preview(commands::preview::Args),
    Render(commands::render::Args),
    History(commands::history::Args),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let context = Context::load(cli.config)?;
    match cli.command {
        Command::Config(args) => commands::config::run(&context, args),
        Command::Rules(args) => commands::rules::run(&context, args),
        Command::Sync(args) => commands::sync::run(&context, args),
        Command::Episodes(args) => commands::episodes::run(&context, args),
        Command::Download(args) => commands::download::run(&context, args),
        Command::Adopt(args) => commands::adopt::run(&context, args),
        Command::Transcribe(args) => commands::transcribe::run(&context, args),
        Command::Transcript(args) => commands::transcript::run(&context, args),
        Command::Analyze(args) => commands::analyze::run(&context, args),
        Command::Inspect(args) => commands::inspect::run(&context, args),
        Command::Label(args) => commands::label::run(&context, args),
        Command::Segments(args) => commands::segments::run(&context, args),
        Command::Draft(args) => commands::draft::run(&context, args),
        Command::Plan(args) => commands::plan::run(&context, args),
        Command::Preview(args) => commands::preview::run(&context, args),
        Command::Render(args) => commands::render::run(&context, args),
        Command::History(args) => commands::history::run(&context, args),
    }
}
