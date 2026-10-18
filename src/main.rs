mod analysis;
mod commands;
mod config;
mod error;
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
use crate::error::{CliError, ErrorReport};

/// Build podcast supercuts: many episodes stitched into one long MP3, minus the parts you skip.
///
/// Grammar: `supercut <resource> <operation> [args]`. Discover with `supercut cli search
/// "<task>"`, inspect with `supercut schema <resource> <operation>`. Results are JSON on stdout;
/// progress goes to stderr; failures print `{"error": {code, message, retryable, hint}}` on
/// stderr with a non-zero exit code.
///
/// Workflow: episodes sync -> episodes download -> transcripts create -> labels create ->
/// plans draft -> plans validate / review / preview -> plans render. `history recover` learns
/// from supercuts made by hand; `rules get` holds the show's editorial preferences.
#[derive(Parser)]
#[command(name = "supercut", version)]
pub struct Cli {
    /// Config file to use instead of the platform default.
    #[arg(long, global = true, env = "SUPERCUT_CONFIG")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Cli(commands::cli::Args),
    Schema(commands::schema::Args),
    Config(commands::config::Args),
    Shows(commands::shows::Args),
    Rules(commands::rules::Args),
    Episodes(commands::episodes::Args),
    Transcripts(commands::transcripts::Args),
    Labels(commands::labels::Args),
    Plans(commands::plans::Args),
    History(commands::history::Args),
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if !error.use_stderr() => {
            let _ = error.print();
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            let message = error.render().to_string();
            return report(
                &CliError::invalid(message.trim())
                    .hint("supercut schema <resource> <operation>")
                    .into(),
            );
        }
    };
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => report(&error),
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let context = Context::load(cli.config)?;
    match cli.command {
        Command::Cli(args) => commands::cli::run(&context, args),
        Command::Schema(args) => commands::schema::run(&context, args),
        Command::Config(args) => commands::config::run(&context, args),
        Command::Shows(args) => commands::shows::run(&context, args),
        Command::Rules(args) => commands::rules::run(&context, args),
        Command::Episodes(args) => commands::episodes::run(&context, args),
        Command::Transcripts(args) => commands::transcripts::run(&context, args),
        Command::Labels(args) => commands::labels::run(&context, args),
        Command::Plans(args) => commands::plans::run(&context, args),
        Command::History(args) => commands::history::run(&context, args),
    }
}

/// Writes the structured error to stderr and picks the exit code from its category.
fn report(error: &anyhow::Error) -> ExitCode {
    let report = ErrorReport::from_anyhow(error);
    let exit = report.code.exit_code();
    match serde_json::to_string(&serde_json::json!({ "error": report })) {
        Ok(json) => eprintln!("{json}"),
        Err(_) => eprintln!("error: {error:#}"),
    }
    ExitCode::from(exit)
}
