use std::io::IsTerminal as _;

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
    println!("{text}");
    Ok(())
}
