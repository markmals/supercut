use std::io::{IsTerminal as _, Write as _};

use serde::Serialize;

/// Presentation for commands whose result also reads well as prose (transcripts, segments,
/// reviews). JSON stays the default everywhere.
#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    #[default]
    Json,
    Text,
}

/// Prints a command's result as JSON on stdout: indented for a terminal, one line otherwise.
pub fn print_json(value: &impl Serialize) -> anyhow::Result<()> {
    let text = if std::io::stdout().is_terminal() {
        serde_json::to_string_pretty(value)?
    } else {
        serde_json::to_string(value)?
    };
    print_text(&format!("{text}\n"))
}

/// Prints a command's result on stdout as it is.
pub fn print_text(text: &str) -> anyhow::Result<()> {
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(text.as_bytes()).and_then(|()| stdout.flush()).map_err(|error| {
        if error.kind() == std::io::ErrorKind::BrokenPipe {
            anyhow::Error::new(ReaderGone)
        } else {
            anyhow::Error::new(error).context("printing to stdout")
        }
    })
}

/// Whoever read stdout stopped before the end, as `supercut … | head` does. Nobody is left to
/// tell, so the command ends quietly.
#[derive(Debug)]
pub struct ReaderGone;

impl std::fmt::Display for ReaderGone {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("stdout was closed before the result was printed")
    }
}

impl std::error::Error for ReaderGone {}
