use serde::Serialize;

use crate::analysis::theme::{ThemeMatch, Themes};
use crate::commands::{Context, Selection};
use crate::timecode::Time;

/// Measure episodes' loudness and level envelope, and find the theme song in each opening.
///
/// Decodes each episode once to measure EBU R128 integrated loudness and loudness range, the
/// exact decoded duration, and a 10 ms level envelope used by `episodes inspect`, `plans
/// validate` and `plans render` (cached in the library; already-analyzed episodes are instant).
/// When the show configures `themes`, also reports which one plays in the episode's first five
/// minutes and where it ends: `plans draft` cuts the opening through that point, and an episode
/// with `opening: null` needs its opening checked by hand. `--season` and `--all` cover the
/// downloaded episodes and name the others on stderr; an episode named by id must be downloaded.
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
    /// Absent when the show has no `themes`; null when none of them plays in the opening.
    #[serde(skip_serializing_if = "Option::is_none")]
    opening: Option<Option<Opening>>,
}

#[derive(Serialize)]
struct Opening {
    theme: String,
    start: Time,
    end: Time,
    score: f32,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (settings, show) = context.show(&args.show)?;
    let themes = Themes::load(&settings.themes)?;
    let index = show.load_synced_index()?;
    let mut measured = Vec::new();
    let explicit = !args.episodes.ids.is_empty();
    for episode in args.episodes.resolve(&index)? {
        if !explicit && episode.audio_file.is_none() {
            eprintln!("{}: not downloaded, skipped", episode.id);
            continue;
        }
        let analysis = crate::analysis::episode_analysis(&show, episode)?;
        let opening = if settings.themes.is_empty() {
            None
        } else {
            Some(themes.find_in(&show.audio_path(episode)?)?.map(opening))
        };
        measured.push(Measured {
            id: episode.id.clone(),
            duration: analysis.duration,
            integrated_lufs: analysis.loudness.integrated_lufs,
            range_lu: analysis.loudness.range_lu,
            codec: analysis.probe.codec,
            bit_rate: analysis.probe.bit_rate,
            sample_rate: analysis.probe.sample_rate,
            channels: analysis.probe.channels,
            opening,
        });
    }
    crate::output::print_json(&measured)
}

fn opening(found: ThemeMatch) -> Opening {
    Opening {
        theme: found.theme.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        start: Time(found.start),
        end: Time(found.end),
        score: (found.score * 1000.0).round() / 1000.0,
    }
}
