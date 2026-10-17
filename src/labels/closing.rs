//! An episode's ending: from the start of its outro (the music that leads into the credits)
//! everything goes, but a bumper that closes the episode stays, standing in for the interstitial
//! between it and the next episode.

use crate::analysis::jingle::JingleMatch;
use crate::plan::Cut;
use crate::timecode::Time;

/// `cuts` ending in one cut from the outro to the end of the file. Cuts reaching into the outro
/// give way to it: they are the labels' reading of the credits, and the outro says exactly where
/// they begin.
pub fn with_outro(cuts: Vec<Cut>, outro: &JingleMatch) -> Vec<Cut> {
    let name = outro.recording.file_stem().unwrap_or_default().to_string_lossy();
    let ending = Cut {
        start: Time(outro.start),
        end: None,
        reason: format!("outro {name} and the credits after it"),
        fade_out: None,
        fade_in: None,
    };
    let before = cuts.into_iter().filter(|cut| cut.end.is_some_and(|end| end.seconds() <= outro.start));
    before.chain(std::iter::once(ending)).collect()
}

/// `cuts` with the bumper left in: a cut reaching into it now ends where it begins, and one
/// inside it goes.
pub fn keeping_bumper(cuts: Vec<Cut>, bumper: &JingleMatch) -> Vec<Cut> {
    cuts.into_iter()
        .filter(|cut| cut.start.seconds() < bumper.start)
        .map(|mut cut| {
            if cut.end.is_none_or(|end| end.seconds() > bumper.start) {
                cut.end = Some(Time(bumper.start));
                cut.fade_in = None;
            }
            cut
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut(start: f64, end: Option<f64>) -> Cut {
        Cut { start: Time(start), end: end.map(Time), reason: String::new(), fade_out: Some(0.25), fade_in: Some(0.25) }
    }

    fn spans(cuts: &[Cut]) -> Vec<(f64, Option<f64>)> {
        cuts.iter().map(|cut| (cut.start.seconds(), cut.end.map(Time::seconds))).collect()
    }

    #[test]
    fn the_outro_replaces_the_labels_reading_of_the_credits() {
        let outro = JingleMatch { recording: "/outros/show.flac".into(), start: 5532.3, end: 5534.4, score: 0.99 };
        // An ad, then credits the labels started inside the goodbye and stopped short of the end.
        let ending = with_outro(vec![cut(1760.0, Some(1830.0)), cut(5530.5, Some(5557.0))], &outro);
        assert_eq!(spans(&ending), vec![(1760.0, Some(1830.0)), (5532.3, None)]);
        assert_eq!((ending[1].fade_out, ending[1].fade_in), (None, None), "the outro starts in the hush after the goodbye");
    }

    #[test]
    fn cuts_give_way_to_the_bumper_that_closes_the_episode() {
        let bumper = JingleMatch { recording: "/bumpers/show.flac".into(), start: 4230.8, end: 4233.5, score: 1.0 };
        // An ad mid-episode, the closing labeled as credits and cut to the end of the file.
        let kept = keeping_bumper(vec![cut(1200.0, Some(1260.0)), cut(4220.0, None)], &bumper);
        assert_eq!(spans(&kept), vec![(1200.0, Some(1260.0)), (4220.0, Some(4230.8))]);
        assert_eq!(kept[1].fade_in, None, "the bumper starts from its own first note");
    }
}
