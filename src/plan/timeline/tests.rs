use super::{Clip, Input, InputEpisode, build};
use crate::plan::{Chapter, Cut};
use crate::render::piece::Source;
use crate::timecode::Time;

fn cut(start: f64, end: f64) -> Cut {
    Cut {
        start: Time(start),
        end: Some(Time(end)),
        reason: "advert".into(),
        fade_out: None,
        fade_in: None,
    }
}

fn chapter(start: f64, title: &str) -> Chapter {
    Chapter { start: Time(start), title: title.into(), description: None }
}

fn episode(id: &str, duration: f64, cuts: Vec<Cut>, chapters: Vec<Chapter>) -> InputEpisode {
    InputEpisode {
        id: id.into(),
        title: format!("{id} title"),
        description: format!("{id} notes"),
        audio: format!("/audio/{id}.mp3").into(),
        duration,
        integrated_lufs: -18.0,
        cuts,
        chapters,
        interstitial_after: true,
    }
}

fn input(episodes: Vec<InputEpisode>, interstitial: Option<Clip>) -> Input {
    Input { target_lufs: -16.0, episodes, interstitial }
}

fn ranges(timeline: &super::Timeline) -> Vec<(String, f64, f64, f64)> {
    timeline
        .pieces
        .iter()
        .map(|piece| match &piece.source {
            Source::File { path, start, end } => (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                *start,
                *end,
                piece.output_start,
            ),
            Source::Beep => ("beep".to_string(), 0.0, 0.0, piece.output_start),
        })
        .collect()
}

#[test]
fn keeps_the_audio_between_cuts_and_stacks_it_without_gaps() {
    let timeline = build(&input(
        vec![episode("a", 100.0, vec![cut(20.0, 30.0), cut(60.0, 70.0)], vec![])],
        None,
    ));

    assert_eq!(
        ranges(&timeline),
        vec![
            ("a.mp3".into(), 0.0, 20.0, 0.0),
            ("a.mp3".into(), 30.0, 60.0, 20.0),
            ("a.mp3".into(), 70.0, 100.0, 50.0),
        ]
    );
    assert_eq!(timeline.duration(), 80.0);
    assert_eq!(timeline.episodes[0].kept_seconds, 80.0);
}

#[test]
fn only_cut_edges_fade() {
    let timeline = build(&input(vec![episode("a", 100.0, vec![cut(20.0, 30.0)], vec![])], None));

    let fades: Vec<(f64, f64)> =
        timeline.pieces.iter().map(|piece| (piece.fade_in, piece.fade_out)).collect();
    assert_eq!(
        fades,
        vec![(0.0, crate::plan::EDGE_FADE_SECONDS), (crate::plan::EDGE_FADE_SECONDS, 0.0)]
    );
}

#[test]
fn a_cut_without_an_end_runs_to_the_end_of_the_episode() {
    let timeline = build(&input(
        vec![episode(
            "a",
            100.0,
            vec![Cut {
                start: Time(90.0),
                end: None,
                reason: "outro".into(),
                fade_out: Some(2.0),
                fade_in: None,
            }],
            vec![],
        )],
        None,
    ));

    assert_eq!(ranges(&timeline), vec![("a.mp3".into(), 0.0, 90.0, 0.0)]);
    assert_eq!(timeline.pieces[0].fade_out, 2.0);
}

#[test]
fn the_interstitial_follows_every_episode_but_one_that_closes_with_its_own_bumper() {
    let clip = Clip { audio: "/audio/sting.mp3".into(), duration: 10.0, integrated_lufs: -20.0 };
    let bonus =
        InputEpisode { interstitial_after: false, ..episode("bonus", 30.0, vec![], vec![]) };
    let timeline = build(&input(
        vec![episode("a", 100.0, vec![], vec![]), bonus, episode("b", 50.0, vec![], vec![])],
        Some(clip),
    ));

    assert_eq!(
        ranges(&timeline),
        vec![
            ("a.mp3".into(), 0.0, 100.0, 0.0),
            ("sting.mp3".into(), 0.0, 10.0, 100.0),
            ("bonus.mp3".into(), 0.0, 30.0, 110.0),
            ("b.mp3".into(), 0.0, 50.0, 140.0),
            ("sting.mp3".into(), 0.0, 10.0, 190.0),
        ]
    );
    assert_eq!(timeline.pieces[1].gain_db, 4.0);
    assert_eq!(timeline.duration(), 200.0);
}

#[test]
fn an_episode_span_ends_at_the_next_episode_so_it_owns_the_interstitial() {
    let clip = Clip { audio: "/audio/sting.mp3".into(), duration: 10.0, integrated_lufs: -16.0 };
    let timeline = build(&input(
        vec![episode("a", 100.0, vec![], vec![]), episode("b", 50.0, vec![], vec![])],
        Some(clip),
    ));

    assert_eq!((timeline.episodes[0].start, timeline.episodes[0].end), (0.0, 110.0));
    assert_eq!((timeline.episodes[1].start, timeline.episodes[1].end), (110.0, 170.0));
}

#[test]
fn chapters_move_with_the_audio_they_label() {
    let chapters = vec![chapter(0.0, "Intro"), chapter(50.0, "Guest")];
    let timeline = build(&input(vec![episode("a", 100.0, vec![cut(20.0, 30.0)], chapters)], None));

    let spans: Vec<(f64, f64, &str)> =
        timeline.episodes[0].chapters.iter().map(|c| (c.start, c.end, c.title.as_str())).collect();
    assert_eq!(spans, vec![(0.0, 40.0, "Intro"), (40.0, 90.0, "Guest")]);
    assert!(timeline.warnings.is_empty());
}

#[test]
fn a_chapter_inside_a_cut_starts_where_the_audio_resumes() {
    let chapters = vec![chapter(0.0, "Intro"), chapter(25.0, "Guest")];
    let timeline = build(&input(vec![episode("a", 100.0, vec![cut(20.0, 30.0)], chapters)], None));

    let starts: Vec<f64> = timeline.episodes[0].chapters.iter().map(|c| c.start).collect();
    assert_eq!(starts, vec![0.0, 20.0]);
}

#[test]
fn a_chapter_with_no_audio_left_after_it_is_dropped_with_a_warning() {
    let chapters = vec![chapter(0.0, "Intro"), chapter(95.0, "Outro")];
    let timeline = build(&input(vec![episode("a", 100.0, vec![cut(90.0, 100.0)], chapters)], None));

    let titles: Vec<&str> =
        timeline.episodes[0].chapters.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, vec!["Intro"]);
    assert_eq!(timeline.warnings.len(), 1);
    assert!(timeline.warnings[0].contains("Outro"), "{}", timeline.warnings[0]);
}

#[test]
fn a_chapter_a_cut_leaves_seconds_long_gets_a_warning() {
    // The plug runs 50-60 s; a hand cut through 58 s leaves 2 s of it before the credits.
    let chapters = vec![chapter(0.0, "Hosts"), chapter(50.0, "Plug"), chapter(60.0, "Credits")];
    let timeline = build(&input(vec![episode("a", 100.0, vec![cut(45.0, 58.0)], chapters)], None));

    assert_eq!(timeline.warnings.len(), 1, "{:?}", timeline.warnings);
    assert!(timeline.warnings[0].contains("`Plug` plays only 2.0 s"), "{}", timeline.warnings[0]);
}

#[test]
fn a_long_lead_in_is_labelled_with_the_episode() {
    let timeline =
        build(&input(vec![episode("a", 100.0, vec![], vec![chapter(45.0, "Guest")])], None));

    let spans: Vec<(f64, f64, &str)> =
        timeline.episodes[0].chapters.iter().map(|c| (c.start, c.end, c.title.as_str())).collect();
    assert_eq!(spans, vec![(0.0, 45.0, "a title"), (45.0, 100.0, "Guest")]);
}

#[test]
fn a_short_lead_in_joins_the_first_chapter_which_carries_the_show_notes() {
    let timeline =
        build(&input(vec![episode("a", 100.0, vec![], vec![chapter(10.0, "Guest")])], None));

    let chapters = &timeline.episodes[0].chapters;
    let spans: Vec<(f64, f64, &str)> =
        chapters.iter().map(|c| (c.start, c.end, c.title.as_str())).collect();
    assert_eq!(spans, vec![(0.0, 100.0, "Guest")]);
    assert!(
        chapters[0].description.is_some(),
        "the episode's first chapter carries its description"
    );
}
