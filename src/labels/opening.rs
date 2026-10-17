//! An episode's opening: everything up to the end of its theme song goes. A supercut's first
//! episode goes on to the hosts' first words; the others start where the theme ends, right
//! after the interstitial.

use crate::analysis::Envelope;
use crate::analysis::theme::ThemeMatch;
use crate::plan::Cut;
use crate::timecode::Time;

/// Above this level the hosts have started talking.
const SPEECH_DB: f32 = -35.0;
/// How long after the theme the hosts may take to start.
const SPEECH_WITHIN_SECONDS: f64 = 10.0;
/// Left before the first syllable so its attack survives.
const PRE_ROLL_SECONDS: f64 = 0.02;

/// Where the opening cut ends.
pub fn opening_end(theme: &ThemeMatch, first_in_supercut: bool, envelope: &Envelope) -> f64 {
    if !first_in_supercut {
        return theme.end;
    }
    envelope
        .first_above(theme.end, theme.end + SPEECH_WITHIN_SECONDS, SPEECH_DB)
        .map_or(theme.end, |onset| (onset - PRE_ROLL_SECONDS).max(theme.end))
}

/// `cuts` led by the opening cut. Cuts that began inside the theme give way to it: they are the
/// labels' reading of the introduction, and the theme says exactly where it ends.
pub fn with_opening(cuts: Vec<Cut>, theme: &ThemeMatch, end: f64) -> Vec<Cut> {
    let name = theme.theme.file_stem().unwrap_or_default().to_string_lossy();
    let opening = Cut {
        start: Time(0.0),
        end: Some(Time(end)),
        reason: format!("opening, through the theme {name} ending at {}", Time(theme.end)),
        fade_out: None,
        fade_in: None,
    };
    std::iter::once(opening).chain(cuts.into_iter().filter(|cut| cut.start.seconds() >= theme.end)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::FRAME_SECONDS;

    fn theme(end: f64) -> ThemeMatch {
        ThemeMatch { theme: "/themes/show.flac".into(), start: end - 27.0, end, score: 0.99 }
    }

    /// Theme music to 43.8 s, 0.4 s of near-silence, then the hosts talking.
    fn envelope() -> Envelope {
        let frames = (60.0 / FRAME_SECONDS) as usize;
        Envelope::from_mean_squares((0..frames).map(|frame| {
            let time = frame as f64 * FRAME_SECONDS;
            if (43.8..44.2).contains(&time) { 1e-9 } else { 0.05 }
        }))
    }

    #[test]
    fn later_episodes_start_where_the_theme_ends_and_the_first_on_the_hosts_first_words() {
        assert_eq!(opening_end(&theme(43.8), false, &envelope()), 43.8);
        let first = opening_end(&theme(43.8), true, &envelope());
        assert!((first - 44.18).abs() < 1e-6, "{first}");
    }

    #[test]
    fn the_opening_replaces_label_cuts_that_began_inside_the_theme() {
        let cut = |start: f64, end: f64| Cut { start: Time(start), end: Some(Time(end)), reason: String::new(), fade_out: None, fade_in: None };
        // The labels' preamble ran past the theme into the hosts' welcome; a classified follows.
        let cuts = with_opening(vec![cut(0.0, 58.0), cut(1500.0, 1560.0)], &theme(43.8), 43.8);
        let spans: Vec<(f64, f64)> = cuts.iter().map(|cut| (cut.start.seconds(), cut.end.unwrap().seconds())).collect();
        assert_eq!(spans, vec![(0.0, 43.8), (1500.0, 1560.0)]);
    }
}
