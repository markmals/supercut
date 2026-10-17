//! Which speech-to-text engine transcribes episodes, and with which model.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct TranscriptionConfig {
    /// `mlx-whisper` (Apple silicon only, and the default there) or `faster-whisper` (any
    /// platform's CPU, through the `whisper-ctranslate2` command, and the default elsewhere).
    pub engine: TranscriptionEngine,
    /// mlx-whisper: a Hugging Face repo or model folder, `mlx-community/whisper-large-v3-turbo`
    /// when unset. faster-whisper: a Whisper size, `large-v3-turbo` when unset. May start with `~`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The episodes' language, e.g. `en`.
    pub language: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TranscriptionEngine {
    MlxWhisper,
    FasterWhisper,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self { engine: TranscriptionEngine::native(), model: None, language: "en".into() }
    }
}

impl TranscriptionEngine {
    /// The engine that runs best on this machine: MLX on Apple silicon, faster-whisper elsewhere.
    pub fn native() -> Self {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) { Self::MlxWhisper } else { Self::FasterWhisper }
    }

    /// The program supercut runs for this engine.
    pub fn program(self) -> &'static str {
        match self {
            Self::MlxWhisper => "mlx_whisper",
            Self::FasterWhisper => "whisper-ctranslate2",
        }
    }
}

impl TranscriptionConfig {
    /// The model the engine runs: the configured one, or Whisper large-v3-turbo as the engine
    /// names it.
    pub fn model(&self) -> &str {
        match (&self.model, self.engine) {
            (Some(model), _) => model,
            (None, TranscriptionEngine::MlxWhisper) => "mlx-community/whisper-large-v3-turbo",
            (None, TranscriptionEngine::FasterWhisper) => "large-v3-turbo",
        }
    }
}
