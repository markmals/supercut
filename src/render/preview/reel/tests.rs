use super::{Kind, Reel};
use crate::plan::Cut;
use crate::plan::timeline::{Clip, Input, InputEpisode, build};
use crate::render::piece::Source;
use crate::timecode::Time;

fn episode(id: &str, cuts: Vec<Cut>) -> InputEpisode {
    InputEpisode {
        id: id.into(),
        title: String::new(),
        description: String::new(),
        audio: format!("/audio/{id}.mp3").into(),
        duration: 100.0,
        integrated_lufs: -16.0,
        cuts,
        chapters: Vec::new(),
        interstitial_after: true,
    }
}

fn cut(start: f64, end: Option<f64>) -> Cut {
    Cut {
        start: Time(start),
        end: end.map(Time),
        reason: format!("cut at {start}"),
        fade_out: None,
        fade_in: None,
    }
}

/// a: theme cut, two cuts four seconds apart, credits cut. Then a bonus episode that closes with
/// its own bumper, so no interstitial follows it. Then b: theme and credits cut.
fn supercut() -> Input {
    let a = episode(
        "a",
        vec![cut(0.0, Some(44.0)), cut(60.0, Some(62.0)), cut(66.0, Some(70.0)), cut(95.0, None)],
    );
    let bonus =
        InputEpisode { duration: 30.0, interstitial_after: false, ..episode("bonus", vec![]) };
    let b = episode("b", vec![cut(0.0, Some(10.0)), cut(90.0, None)]);
    let sting = Clip { audio: "/audio/sting.flac".into(), duration: 5.0, integrated_lufs: -16.0 };
    Input { target_lufs: -16.0, episodes: vec![a, bonus, b], interstitial: Some(sting) }
}

fn played(reel: &Reel) -> Vec<(String, f64, f64)> {
    reel.pieces
        .iter()
        .filter_map(|piece| match &piece.source {
            Source::File { path, start, end } => {
                Some((path.file_stem()?.to_string_lossy().into_owned(), *start, *end))
            }
            Source::Beep => None,
        })
        .collect()
}

#[test]
fn every_splice_is_heard_once_and_no_audio_twice() {
    let input = supercut();
    let reel = Reel::of(&input, &build(&input), 6.0);

    let kinds: Vec<&Kind> = reel.transitions.iter().map(|transition| &transition.kind).collect();
    use Kind::*;
    assert_eq!(kinds, [&Opening, &Cut, &Cut, &EpisodeBoundary, &EpisodeBoundary, &Ending]);

    let played = played(&reel);
    let episodes: Vec<&(String, f64, f64)> =
        played.iter().filter(|(name, ..)| name != "sting").collect();
    for (index, (name, start, end)) in episodes.iter().enumerate() {
        for (other, other_start, other_end) in &episodes[index + 1..] {
            let overlap = name == other && start < other_end && other_start < end;
            assert!(!overlap, "{name} {start}-{end} plays again as {other_start}-{other_end}");
        }
    }
    let stings = played.iter().filter(|(name, ..)| name == "sting").count();
    assert_eq!(stings, 2, "after a and after b, not after the bonus episode's own bumper");
}

#[test]
fn the_reel_plays_only_what_the_render_keeps_and_says_where_each_splice_falls() {
    let input = supercut();
    let reel = Reel::of(&input, &build(&input), 6.0);
    let removed = |name: &str| match name {
        "a" => vec![(0.0, 44.0), (60.0, 62.0), (66.0, 70.0), (95.0, 100.0)],
        "b" => vec![(0.0, 10.0), (90.0, 100.0)],
        _ => vec![],
    };
    for (name, start, end) in played(&reel) {
        let cut_audio = removed(&name).into_iter().any(|(from, to)| start < to && from < end);
        assert!(!cut_audio, "the reel plays cut audio {name} {start}-{end}");
    }
    // The first cut is 16 s into the render; its window opens at 10 s and follows the opening's
    // 6 s and a 0.7 s marker.
    assert!(
        (reel.transitions[1].preview_time.seconds() - 12.7).abs() < 1e-6,
        "{:?}",
        reel.transitions[1]
    );
    assert_eq!(reel.transitions[1].cut_start, Some(Time(60.0)));
}

#[test]
fn the_reel_starts_and_ends_with_the_supercuts_own_first_and_last_seconds() {
    let input = supercut();
    let played = played(&Reel::of(&input, &build(&input), 6.0));
    assert_eq!(played[0], ("a".into(), 44.0, 50.0), "the supercut starts where the theme cut ends");
    let ending = &played[played.len() - 2..];
    assert_eq!(
        ending,
        [("b".into(), 84.0, 90.0), ("sting".into(), 0.0, 5.0)],
        "and ends on the sting"
    );
}
