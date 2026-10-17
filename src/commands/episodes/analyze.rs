use serde::Serialize;

use crate::analysis::jingle::{JingleMatch, Jingles};
use crate::commands::{Context, Selection};
use crate::timecode::Time;

/// Measure episodes' loudness and level envelope, and find the show's recurring recordings in each.
///
/// Decodes each episode once to measure EBU R128 integrated loudness and loudness range, the
/// exact decoded duration, and a 10 ms level envelope used by `episodes inspect`, `plans
/// validate` and `plans render` (cached in the library; already-analyzed episodes are instant).
/// Then reports where each kind of recording the show configures plays: the `themes` one in
/// the first five minutes (`plans draft` cuts the opening through its end), the `outros` one in
/// the last three (`plans draft` cuts from its start to the end), and the `bumpers` one that
/// closes the file (`plans draft` keeps it and plays no interstitial after it). A field is
/// absent when the show configures no such recordings and null when none plays; a null theme or
/// outro means that edge needs placing by hand. `--season` and `--all` cover the downloaded
/// episodes and name the others on stderr; an episode named by id must be downloaded.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    theme: Option<Option<Found>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outro: Option<Option<Found>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bumper: Option<Option<Found>>,
}

#[derive(Serialize)]
struct Found {
    recording: String,
    start: Time,
    end: Time,
    score: f32,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (settings, show) = context.show(&args.show)?;
    let themes = Jingles::load(&settings.themes)?;
    let outros = Jingles::load(&settings.outros)?;
    let bumpers = Jingles::load(&settings.bumpers)?;
    let index = show.load_synced_index()?;
    let mut measured = Vec::new();
    let explicit = !args.episodes.ids.is_empty();
    for episode in args.episodes.resolve(&index)? {
        if !explicit && episode.audio_file.is_none() {
            eprintln!("{}: not downloaded, skipped", episode.id);
            continue;
        }
        let analysis = crate::analysis::episode_analysis(&show, episode)?;
        let (audio, duration) = (show.audio_path(episode)?, analysis.duration);
        measured.push(Measured {
            id: episode.id.clone(),
            duration,
            integrated_lufs: analysis.loudness.integrated_lufs,
            range_lu: analysis.loudness.range_lu,
            codec: analysis.probe.codec,
            bit_rate: analysis.probe.bit_rate,
            sample_rate: analysis.probe.sample_rate,
            channels: analysis.probe.channels,
            theme: found(&themes, |themes| themes.opening_of(&audio))?,
            outro: found(&outros, |outros| outros.outro_of(&audio, duration))?,
            bumper: found(&bumpers, |bumpers| bumpers.ending_of(&audio, duration))?,
        });
    }
    crate::output::print_json(&measured)
}

/// Absent when the show configures none of these recordings, null when none plays.
fn found(
    jingles: &Jingles,
    find: impl FnOnce(&Jingles) -> anyhow::Result<Option<JingleMatch>>,
) -> anyhow::Result<Option<Option<Found>>> {
    if jingles.is_empty() {
        return Ok(None);
    }
    Ok(Some(find(jingles)?.map(Found::from)))
}

impl From<JingleMatch> for Found {
    fn from(found: JingleMatch) -> Self {
        Self {
            recording: found.recording.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            start: Time(found.start),
            end: Time(found.end),
            score: (found.score * 1000.0).round() / 1000.0,
        }
    }
}
