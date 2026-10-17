use serde::Serialize;

use crate::commands::{Context, Selection};

/// Measure episodes' loudness and level envelope (cached; other commands run it on demand).
///
/// Decodes each episode once to measure EBU R128 integrated loudness and loudness range, the
/// exact decoded duration, and a 10 ms level envelope used by `episodes inspect`, `plans
/// validate` and `plans render`. Cached in the library; already-analyzed episodes are instant.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug.
    show: String,
    #[command(flatten)]
    episodes: Selection,
}

#[derive(Serialize)]
struct Measured {
    id: String,
    duration: f64,
    integrated_lufs: f64,
    range_lu: f64,
    codec: String,
    bit_rate: Option<u64>,
    sample_rate: u32,
    channels: u32,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, show) = context.show(&args.show)?;
    let index = show.load_synced_index()?;
    let mut measured = Vec::new();
    for episode in args.episodes.resolve(&index)? {
        let analysis = crate::analysis::episode_analysis(&show, episode)?;
        measured.push(Measured {
            id: episode.id.clone(),
            duration: analysis.duration,
            integrated_lufs: analysis.loudness.integrated_lufs,
            range_lu: analysis.loudness.range_lu,
            codec: analysis.probe.codec,
            bit_rate: analysis.probe.bit_rate,
            sample_rate: analysis.probe.sample_rate,
            channels: analysis.probe.channels,
        });
    }
    crate::output::print_json(&measured)
}
