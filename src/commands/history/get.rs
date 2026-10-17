use crate::commands::Context;
use crate::error::CliError;
use crate::history;
use crate::output::Format;

/// Print a recovered supercut: its episodes, what was cut from each with the fades used, what was said in each cut, and the interstitials.
///
/// Cut text needs the episode's transcript (`supercut transcripts create`); without it the cut
/// is reported by time only.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug.
    show: String,
    /// Recovered supercut name (the export's file stem), as listed by `supercut history list`.
    name: String,
    /// `json` (default) or `text` (Markdown).
    #[arg(long, value_enum, default_value_t)]
    output: Format,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, show) = context.show(&args.show)?;
    if !history::recovery_path(&show, &args.name).exists() {
        let error = CliError::not_found(format!("no recovered supercut `{}`", args.name));
        return Err(error.hint(format!("supercut history list {}", args.show)).into());
    }
    let index = show.load_synced_index()?;
    let (recovery, plan) = history::load(&show, &args.name)?;
    let report = history::report::build(&show, &index, &recovery, &plan)?;
    match args.output {
        Format::Json => crate::output::print_json(&report),
        Format::Text => {
            print!("{}", history::report::markdown(&report));
            Ok(())
        }
    }
}
