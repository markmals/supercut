use std::path::Path;

use anyhow::Context as _;

use crate::analysis;
use crate::commands::Context as AppContext;
use crate::config::ShowConfig;
use crate::library::{Episode, ShowLibrary};
use crate::media::Probe;
use crate::plan::timeline::{self, Clip, InputEpisode};
use crate::plan::{InterstitialSetting, Plan};
use crate::render::format::OutputFormat;
use crate::transcript::Transcript;

/// A plan with everything it refers to loaded and measured: ready to build a timeline from.
pub struct Resolved<'a> {
    pub show: &'a ShowConfig,
    pub library: ShowLibrary,
    /// The index entries of the plan's episodes, in the plan's order.
    pub episodes: Vec<Episode>,
    pub input: timeline::Input,
    pub format: OutputFormat,
}

impl Resolved<'_> {
    /// The episode's transcript, when it has been transcribed.
    pub fn transcript(&self, id: &str) -> anyhow::Result<Option<Transcript>> {
        let path = self.library.transcript_path(id);
        if !path.exists() {
            return Ok(None);
        }
        crate::library::read_json(&path).map(Some)
    }
}

pub fn resolve<'a>(context: &'a AppContext, plan: &Plan) -> anyhow::Result<Resolved<'a>> {
    let (show, library) = context.show(&plan.show)?;
    anyhow::ensure!(!plan.episodes.is_empty(), "the plan lists no episodes");
    let index = library.load_synced_index()?;

    let mut episodes = Vec::new();
    let mut inputs = Vec::new();
    let mut probes: Vec<Probe> = Vec::new();
    for planned in &plan.episodes {
        let episode = index.get(&planned.id)?;
        let audio = library.audio_path(episode)?;
        let analysis = analysis::episode_analysis(&library, episode)?;
        inputs.push(InputEpisode {
            id: episode.id.clone(),
            title: episode.title.clone(),
            description: episode.description.clone(),
            audio,
            duration: analysis.duration,
            integrated_lufs: analysis.loudness.integrated_lufs,
            cuts: planned.cuts.clone(),
            chapters: planned.chapters.clone(),
            interstitial_after: planned.interstitial_after,
        });
        probes.push(analysis.probe);
        episodes.push(episode.clone());
    }

    let interstitial = interstitial_clip(interstitial_path(show, plan))?;
    let target_lufs = context.config.loudness.target_lufs;
    Ok(Resolved {
        show,
        library,
        episodes,
        format: OutputFormat::from_probes(&probes)?,
        input: timeline::Input { target_lufs, episodes: inputs, interstitial },
    })
}

/// The clip played between episodes: the plan overrides the show's setting.
pub fn interstitial_path<'a>(show: &'a ShowConfig, plan: &'a Plan) -> Option<&'a Path> {
    match &plan.interstitial {
        Some(InterstitialSetting::Enabled(false)) => None,
        Some(InterstitialSetting::File(path)) => Some(path),
        _ => show.interstitial.as_deref(),
    }
}

fn interstitial_clip(path: Option<&Path>) -> anyhow::Result<Option<Clip>> {
    let Some(path) = path else { return Ok(None) };
    let (analysis, _) = analysis::analyze_file(path)
        .with_context(|| format!("measuring the interstitial {}", path.display()))?;
    Ok(Some(Clip {
        audio: path.to_path_buf(),
        duration: analysis.duration,
        integrated_lufs: analysis.loudness.integrated_lufs,
    }))
}
