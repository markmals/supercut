//! Segment labeling with Jev: ~30 s windows of transcript in, the show's segments out, and a
//! cut plan drafted from them.

mod also_cut;
mod closing;
mod draft;
mod fine;
mod jev;
mod opening;
mod parts;
mod phrases;
mod record;
mod refine;
mod run;
mod smooth;
mod state;
mod unit;
mod utterance;
mod window;

use anyhow::Context as _;

pub use also_cut::AlsoCut;
pub use draft::{Uncertain, draft, plan_path};
pub use record::{LabeledSegment, Labels};
pub use run::{estimate_episodes, label_episodes};

use crate::error::CliError;
use crate::library::{Episode, ShowLibrary};
use crate::transcript::Transcript;

/// Level under which audio counts as quiet: a wordless gap with nothing playing in it, a cut
/// edge that splices without a fade.
pub const QUIET_DB: f32 = -45.0;

/// Under this, a segment is worth listening to before acting on it.
pub const LOW_CONFIDENCE: f64 = 0.7;

/// Is this segment too uncertain to cut or chapter unseen? One shaky window inside a long
/// stretch is normal; a stretch whose label is weak throughout is not.
pub fn uncertain(segment: &LabeledSegment) -> bool {
    segment.probability < LOW_CONFIDENCE
}

/// The episode's transcript, or an error naming the command that writes it.
pub fn transcript(library: &ShowLibrary, episode: &Episode) -> anyhow::Result<Transcript> {
    let path = library.transcript_path(&episode.id);
    if !path.exists() {
        let slug = &library.slug;
        return Err(CliError::precondition(format!("`{}` is not transcribed", episode.id))
            .hint(format!("supercut transcripts create {slug} {}", episode.id))
            .detail("path", path.display().to_string())
            .into());
    }
    crate::library::read_json(&path).with_context(|| format!("loading {}", path.display()))
}

/// The episode's labels, or an error naming the command that writes them.
pub fn labels(library: &ShowLibrary, episode: &Episode) -> anyhow::Result<Labels> {
    let path = library.labels_path(&episode.id);
    if !path.exists() {
        let slug = &library.slug;
        return Err(CliError::precondition(format!("`{}` is not labeled", episode.id))
            .hint(format!("supercut labels create {slug} {}", episode.id))
            .detail("path", path.display().to_string())
            .into());
    }
    Labels::load(&path)
}
