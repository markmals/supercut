use std::path::Path;

use anyhow::Context as _;
use id3::frame::{Chapter, Content, Picture, PictureType, TableOfContents};
use id3::{Frame, Tag, TagLike, Version};

use crate::error::CliError;
use crate::plan::timeline::EpisodeSpan;

/// ID3 says offsets it cannot state are all ones; ours are times, not byte positions.
const UNKNOWN_OFFSET: u32 = 0xffff_ffff;

pub struct Tags<'a> {
    pub title: &'a str,
    pub show_title: &'a str,
    pub artwork: Option<&'a Path>,
    pub episodes: &'a [EpisodeSpan],
}

/// Replaces the file's tags: titles, cover art and the chapters.
///
/// ID3v2.3, not 2.4, because of the frames nested inside CHAP and CTOC: ffmpeg — and every
/// player built on it — reads their lengths as plain 32-bit integers, so a 2.4 tag loses any
/// chapter whose title or description is 128 bytes or longer.
pub fn write(path: &Path, tags: &Tags) -> anyhow::Result<usize> {
    let mut tag = Tag::new();
    tag.set_text("TIT2", tags.title);
    tag.set_text("TPE1", tags.show_title);
    tag.set_text("TALB", tags.show_title);
    if let Some(artwork) = tags.artwork {
        tag.add_frame(picture(artwork)?);
    }
    let chapters = add_chapters(&mut tag, tags.episodes);
    tag.write_to_path(path, Version::Id3v23).with_context(|| format!("tagging {}", path.display()))?;
    Ok(chapters)
}

/// Every chapter in one ordered, top-level table of contents, and no other table: Castro reads
/// only the first table it meets, so a table per episode showed just the first episode's
/// chapters. Titles are the plan's, which name their episode; an episode's first chapter carries
/// its show notes. The chapter frames come before the table because ffmpeg stops collecting
/// chapters at the first table of contents it meets. Returns how many chapters were written.
fn add_chapters(tag: &mut Tag, episodes: &[EpisodeSpan]) -> usize {
    let mut chapters = Vec::new();
    let mut entries = Vec::new();
    for (index, span) in episodes.iter().enumerate() {
        let id = format!("ep{:02}", index + 1);
        if span.chapters.is_empty() {
            chapters.push(chapter(&id, span.start, span.end, &span.title, Some(&span.description)));
            entries.push(id);
            continue;
        }
        for (position, segment) in span.chapters.iter().enumerate() {
            let child = format!("{id}c{:02}", position + 1);
            chapters.push(chapter(&child, segment.start, segment.end, &segment.title, segment.description.as_deref()));
            entries.push(child);
        }
    }
    let written = chapters.len();
    for frame in chapters {
        tag.add_frame(frame);
    }
    tag.add_frame(contents("toc", entries));
    written
}

fn chapter(id: &str, start: f64, end: f64, title: &str, description: Option<&str>) -> Frame {
    let mut frames = vec![text("TIT2", title)];
    if let Some(description) = description.filter(|description| !description.is_empty()) {
        frames.push(text("TIT3", description));
    }
    let chapter = Chapter {
        element_id: id.to_string(),
        start_time: milliseconds(start),
        end_time: milliseconds(end),
        start_offset: UNKNOWN_OFFSET,
        end_offset: UNKNOWN_OFFSET,
        frames,
    };
    Frame::with_content("CHAP", Content::Chapter(chapter))
}

fn contents(id: &str, elements: Vec<String>) -> Frame {
    let contents = TableOfContents { element_id: id.to_string(), top_level: true, ordered: true, elements, frames: Vec::new() };
    Frame::with_content("CTOC", Content::TableOfContents(contents))
}

fn text(id: &str, value: &str) -> Frame {
    Frame::text(id, value)
}

fn milliseconds(seconds: f64) -> u32 {
    (seconds * 1000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32
}

fn picture(path: &Path) -> anyhow::Result<Frame> {
    let data = std::fs::read(path).with_context(|| format!("reading the artwork {}", path.display()))?;
    let extension = path.extension().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
    let mime_type = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        other => {
            let unusable = CliError::invalid(format!("the artwork {} is a `{other}` file", path.display()))
                .hint("point `artwork` at a JPEG or PNG");
            return Err(unusable.into());
        }
    };
    let picture = Picture {
        mime_type: mime_type.to_string(),
        picture_type: PictureType::CoverFront,
        description: String::new(),
        data,
    };
    Ok(Frame::with_content("APIC", Content::Picture(picture)))
}

#[cfg(test)]
mod tests;
