//! Which speech-to-text engine transcribes episodes, and with which model.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::CliError;

/// mlx-whisper's model when the config names none.
const MLX_MODEL: &str = "mlx-community/whisper-large-v3-turbo";

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct TranscriptionConfig {
    /// `mlx-whisper` (Apple silicon only, and the default there) or `whisper-cpp` (any platform,
    /// and the default elsewhere).
    pub engine: TranscriptionEngine,
    /// mlx-whisper: a Hugging Face repo or model folder, `mlx-community/whisper-large-v3-turbo`
    /// when unset. whisper.cpp: a ggml model file such as `ggml-large-v3-turbo-q8_0.bin`, which
    /// it needs. May start with `~`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The episodes' language, e.g. `en`.
    pub language: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TranscriptionEngine {
    MlxWhisper,
    WhisperCpp,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self { engine: TranscriptionEngine::native(), model: None, language: "en".into() }
    }
}

impl TranscriptionEngine {
    /// The engine that runs on this machine: MLX on Apple silicon, whisper.cpp anywhere else.
    pub fn native() -> Self {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) { Self::MlxWhisper } else { Self::WhisperCpp }
    }
}

impl TranscriptionConfig {
    /// The model the engine runs: the configured one, or mlx-whisper's default. whisper.cpp has
    /// no default, so a config without a model file is refused.
    pub fn model(&self) -> anyhow::Result<&str> {
        match (&self.model, self.engine) {
            (Some(model), _) => Ok(model),
            (None, TranscriptionEngine::MlxWhisper) => Ok(MLX_MODEL),
            (None, TranscriptionEngine::WhisperCpp) => Err(CliError::precondition(
                "whisper.cpp needs a model file: set transcription.model to a ggml model, e.g. ggml-large-v3-turbo-q8_0.bin",
            )
            .hint("supercut config validate")
            .into()),
        }
    }
}
