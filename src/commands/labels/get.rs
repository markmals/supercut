use serde::Serialize;

use crate::commands::Context;
use crate::labels;
use crate::output::{Format, print_json};
use crate::timecode::Time;

/// Print an episode's labeled segments.
///
/// Reads `labels/<id>.json`, written by `supercut labels create`. Segments whose probability or
/// window confidence falls under 0.7 are flagged: listen to those before cutting them.
#[derive(clap::Args)]
pub struct Args {
    /// Show slug, as listed by `supercut shows list`.
    show: String,

    /// Episode id, as listed by `supercut episodes list <show>`.
    episode: String,

    /// `json` for the segment list, `text` for one line per segment.
    #[arg(long, value_enum, default_value_t)]
    output: Format,
}

#[derive(Serialize)]
struct Segments {
    episode: String,
    segments: Vec<Segment>,
}

#[derive(Serialize)]
struct Segment {
    label: String,
    start: Time,
    end: Time,
    probability: f64,
    low_confidence: bool,
}

pub fn run(context: &Context, args: Args) -> anyhow::Result<()> {
    let (_, library) = context.show(&args.show)?;
    let index = library.load_synced_index()?;
    let episode = index.get(&args.episode)?;
    let labels = labels::labels(&library, episode)?;
    let segments = Segments {
        episode: labels.episode.clone(),
        segments: labels
            .segments
            .iter()
            .map(|segment| Segment {
                label: segment.label.clone(),
                start: segment.start,
                end: segment.end,
                probability: segment.probability,
                low_confidence: labels::uncertain(segment),
            })
            .collect(),
    };
    match args.output {
        Format::Json => print_json(&segments),
        Format::Text => {
            print_segments(&segments);
            Ok(())
        }
    }
}

fn print_segments(segments: &Segments) {
    for segment in &segments.segments {
        let flag = if segment.low_confidence { "  low confidence" } else { "" };
        println!(
            "{}-{}  {}  (p={:.2}){flag}",
            segment.start, segment.end, segment.label, segment.probability
        );
    }
}
