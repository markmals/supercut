//! An episode that ends with one of the show's bumpers keeps it: the bumper stands in for the
//! interstitial between it and the next episode.

use crate::analysis::jingle::JingleMatch;
use crate::plan::Cut;
use crate::timecode::Time;

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
        Cut { start: Time(start), end: end.map(Time), reason: String::new(), fade_out: None, fade_in: Some(0.25) }
    }

    #[test]
    fn cuts_give_way_to_the_bumper_that_closes_the_episode() {
        let bumper = JingleMatch { recording: "/bumpers/show.flac".into(), start: 4230.8, end: 4233.5, score: 1.0 };
        // An ad mid-episode, the closing labeled as credits and cut to the end of the file.
        let kept = keeping_bumper(vec![cut(1200.0, Some(1260.0)), cut(4220.0, None)], &bumper);
        let spans: Vec<(f64, Option<f64>)> =
            kept.iter().map(|cut| (cut.start.seconds(), cut.end.map(Time::seconds))).collect();
        assert_eq!(spans, vec![(1200.0, Some(1260.0)), (4220.0, Some(4230.8))]);
        assert_eq!(kept[1].fade_in, None, "the bumper starts from its own first note");
    }
}
