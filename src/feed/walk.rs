use std::path::{Path, PathBuf};

use anyhow::Context as _;

const AUDIO_EXTENSIONS: [&str; 7] = ["mp3", "m4a", "aac", "wav", "aif", "aiff", "flac"];

/// Every audio file under `dir`, sorted so two runs see the same order.
pub fn audio_files(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    collect(dir, &mut found)?;
    found.sort();
    Ok(found)
}

fn collect(dir: &Path, found: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for entry in entries {
        let path = entry.with_context(|| format!("reading {}", dir.display()))?.path();
        if hidden(&path) {
            continue;
        }
        if path.is_dir() {
            collect(&path, found)?;
        } else if is_audio(&path) {
            found.push(path);
        }
    }
    Ok(())
}

/// Dotfiles, and with them macOS `._name.mp3` resource forks, which are not audio.
fn hidden(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with('.'))
}

fn is_audio(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    AUDIO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
}
