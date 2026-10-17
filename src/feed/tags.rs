use std::ffi::OsStr;
use std::path::Path;

use anyhow::Context as _;
use serde::Deserialize;

use crate::media;

/// The title an audio file claims for itself, read from its own tags.
pub fn title(path: &Path) -> anyhow::Result<Option<String>> {
    let title = if is_mp3(path) { id3_title(path)? } else { ffprobe_title(path)? };
    Ok(title.map(|title| title.trim().to_owned()).filter(|title| !title.is_empty()))
}

fn is_mp3(path: &Path) -> bool {
    path.extension().and_then(OsStr::to_str).is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"))
}

fn id3_title(path: &Path) -> anyhow::Result<Option<String>> {
    match id3::Tag::read_from_path(path) {
        Ok(tag) => Ok(id3::TagLike::title(&tag).map(str::to_owned)),
        Err(error) if matches!(error.kind, id3::ErrorKind::NoTag) => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading the ID3 tags of {}", path.display())),
    }
}

fn ffprobe_title(path: &Path) -> anyhow::Result<Option<String>> {
    #[derive(Deserialize)]
    struct Report {
        format: Format,
    }
    #[derive(Deserialize)]
    struct Format {
        #[serde(default)]
        tags: Tags,
    }
    #[derive(Default, Deserialize)]
    struct Tags {
        title: Option<String>,
    }

    let output = media::run(
        "ffprobe",
        [
            OsStr::new("-v"),
            OsStr::new("error"),
            OsStr::new("-show_entries"),
            OsStr::new("format_tags=title"),
            OsStr::new("-of"),
            OsStr::new("json"),
            path.as_os_str(),
        ],
    )?;
    let report: Report = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("reading the tags of {}", path.display()))?;
    Ok(report.format.tags.title)
}
