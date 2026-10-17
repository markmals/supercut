use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::Path;

use anyhow::Context as _;

use super::fetch::{agent, redacted};
use crate::library::{Episode, ShowLibrary, partial_path};

const BUFFER_BYTES: usize = 256 * 1024;
/// How much more has to arrive before the next progress line.
const PROGRESS_STEP: u64 = 8 * 1024 * 1024;

pub struct Download {
    /// File name inside the show's `audio/` folder.
    pub file: String,
    pub bytes: u64,
}

/// Fetches one episode's audio into the library. The bytes land in a `.partial` file that is
/// renamed once the download finishes, so an interrupted run leaves no half episode behind.
pub fn download(library: &ShowLibrary, episode: &Episode) -> anyhow::Result<Download> {
    let url = &episode.enclosure_url;
    let mut response = agent()
        .get(url)
        .call()
        .with_context(|| format!("fetching {}", redacted(url)))?;
    let total = response.body().content_length();
    let file = format!("{}.{}", episode.id, extension(response.body().mime_type(), url));

    let destination = library.audio_dir().join(&file);
    let parent = destination.parent().context("the library has no audio folder")?;
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    let partial = partial_path(&destination);

    eprintln!("{}: downloading {} ({})", episode.id, file, size(total));
    let mut reader = response.body_mut().with_config().limit(u64::MAX).reader();
    let bytes = write_file(&mut reader, &partial, &episode.id, total)?;
    std::fs::rename(&partial, &destination)
        .with_context(|| format!("moving {} into place", destination.display()))?;
    eprintln!("{}: saved {} ({})", episode.id, file, size(Some(bytes)));
    Ok(Download { file, bytes })
}

fn write_file(reader: &mut impl Read, path: &Path, id: &str, total: Option<u64>) -> anyhow::Result<u64> {
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    let mut buffer = vec![0u8; BUFFER_BYTES];
    let mut written = 0u64;
    let mut announced = 0u64;
    loop {
        let read = reader.read(&mut buffer).with_context(|| format!("downloading {id}"))?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).with_context(|| format!("writing {}", path.display()))?;
        written += read as u64;
        if written - announced >= PROGRESS_STEP {
            announced = written;
            eprintln!("{id}: {}", progress(written, total));
        }
    }
    writer.flush().with_context(|| format!("writing {}", path.display()))?;
    Ok(written)
}

fn progress(written: u64, total: Option<u64>) -> String {
    match total {
        Some(total) if total > 0 => format!("{} of {} ({}%)", size(Some(written)), size(Some(total)), written * 100 / total),
        _ => size(Some(written)),
    }
}

fn size(bytes: Option<u64>) -> String {
    match bytes {
        Some(bytes) => format!("{:.1} MB", bytes as f64 / 1_000_000.0),
        None => "unknown size".to_owned(),
    }
}

/// What to call the file: the served content type, else the URL's own extension, else MP3.
fn extension(mime_type: Option<&str>, url: &str) -> String {
    let mime_type = mime_type.unwrap_or_default().split(';').next().unwrap_or_default();
    let known = match mime_type.trim().to_ascii_lowercase().as_str() {
        "audio/mpeg" | "audio/mp3" | "audio/x-mpeg" => Some("mp3"),
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" | "video/mp4" => Some("m4a"),
        "audio/aac" | "audio/aacp" => Some("aac"),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/ogg" | "audio/opus" => Some("ogg"),
        "audio/aiff" | "audio/x-aiff" => Some("aiff"),
        _ => None,
    };
    known.map(str::to_owned).or_else(|| url_extension(url)).unwrap_or_else(|| "mp3".to_owned())
}

fn url_extension(url: &str) -> Option<String> {
    let path = redacted(url);
    let extension = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    (extension.len() <= 4 && extension.chars().all(|c| c.is_ascii_alphanumeric())).then_some(extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_after_the_served_type_then_the_url() {
        assert_eq!(extension(Some("audio/mpeg"), "https://example.com/stream?id=7"), "mp3");
        assert_eq!(extension(Some("application/octet-stream"), "https://example.com/ep.m4a?token=x"), "m4a");
        assert_eq!(extension(None, "https://example.com/stream"), "mp3");
        assert_eq!(extension(Some("audio/x-m4a; charset=utf-8"), "https://example.com/ep.mp3"), "m4a");
    }
}
