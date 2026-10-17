use crate::commands::{Context, Selection};
use crate::labels;
use crate::output::print_json;

/// Label every stretch of episodes with the show's segments using the Jev decision model (paid API).
///
/// Cuts each episode's transcript into ~30 s windows and asks Jev which configured segment
/// (`intro`, `classified`, `guest`...) each one belongs to, then smooths the answers into
/// contiguous segments. Needs transcripts (`supercut transcripts create`). Writes
/// `labels/<id>.json` in the show's library folder; episodes already labeled with the show's
/// current segments are skipped unless `--force`.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as listed by `supercut shows list`.
    show: String,

    #[command(flatten)]
    selection: Selection,

    /// Label episodes that already carry labels, replacing them.
    #[arg(long)]
    force: bool,

    /// Report the windows and estimated input tokens without calling Jev or writing anything.
    #[arg(long)]
    dry_run: bool,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let index = library.load_synced_index()?;
    let episodes = args.selection.resolve(&index)?;
    if args.dry_run {
        let estimates = labels::estimate_episodes(context, &args.show, &episodes, args.force)?;
        return print_json(&estimates);
    }
    let labeled = labels::label_episodes(context, &args.show, &episodes, args.force)?;
    print_json(&labeled)
}
