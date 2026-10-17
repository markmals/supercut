use std::path::Path;
use std::process::Command;

use id3::{Tag, TagLike};

use super::{Tags, write};
use crate::plan::timeline::{EpisodeSpan, SegmentChapter};

fn silent_mp3(path: &Path) {
    let status = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error", "-f", "lavfi", "-i", "anullsrc=r=44100:cl=stereo"])
        .args(["-t", "2", "-c:a", "libmp3lame", "-b:a", "64k", "-y"])
        .arg(path)
        .status()
        .expect("running ffmpeg");
    assert!(status.success(), "ffmpeg could not write the test file");
}

fn chapter(start: f64, end: f64, title: &str) -> SegmentChapter {
    SegmentChapter { start, end, title: title.into(), description: None }
}

fn span(id: &str, start: f64, end: f64, chapters: Vec<SegmentChapter>) -> EpisodeSpan {
    EpisodeSpan {
        id: id.into(),
        title: format!("{id} title"),
        description: format!("{id} notes"),
        start,
        end,
        kept_seconds: end - start,
        cuts: 0,
        chapters,
    }
}

#[test]
fn chapters_round_trip_through_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("supercut.mp3");
    silent_mp3(&path);

    let episodes = vec![
        span("a", 0.0, 60.0, vec![chapter(0.0, 30.0, "Intro"), chapter(30.0, 60.0, "Guest")]),
        span("b", 60.0, 90.0, Vec::new()),
    ];
    let tags = Tags { title: "NHL S10 part 1", show_title: "The Neighborhood Listen", artwork: None, episodes: &episodes };
    assert_eq!(write(&path, &tags).unwrap(), 3);

    let tag = Tag::read_from_path(&path).unwrap();
    assert_eq!(tag.title(), Some("NHL S10 part 1"));
    assert_eq!(tag.album(), Some("The Neighborhood Listen"));

    let top = tag.tables_of_contents().find(|contents| contents.top_level).unwrap();
    assert!(top.ordered);
    assert_eq!(top.elements, vec!["ep01".to_string(), "ep02".to_string()]);

    let episode = tag.tables_of_contents().find(|contents| contents.element_id == "ep01").unwrap();
    assert_eq!(episode.elements, vec!["ep01c01".to_string(), "ep01c02".to_string()]);
    assert_eq!(title_of(&episode.frames), Some("a title"));

    let chapters: Vec<(&str, u32, u32, Option<&str>)> = tag
        .chapters()
        .map(|chapter| {
            (chapter.element_id.as_str(), chapter.start_time, chapter.end_time, title_of(&chapter.frames))
        })
        .collect();
    assert_eq!(
        chapters,
        vec![
            ("ep01c01", 0, 30_000, Some("a title - Intro")),
            ("ep01c02", 30_000, 60_000, Some("a title - Guest")),
            ("ep02", 60_000, 90_000, Some("b title")),
        ]
    );
}

fn title_of(frames: &[id3::Frame]) -> Option<&str> {
    frames.iter().find(|frame| frame.id() == "TIT2").and_then(|frame| frame.content().text())
}
