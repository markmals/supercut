//! An episode's opening: everything up to the end of its theme song goes. A supercut's first
//! episode goes on to the hosts' first words; the others start where the theme ends, right
//! after the interstitial, unless dead air follows the theme.

use crate::analysis::Envelope;
use crate::analysis::jingle::JingleMatch;
use crate::plan::Cut;
use crate::timecode::Time;

/// Above this level the hosts have started talking.
const SPEECH_DB: f32 = -35.0;
/// How long after the theme the hosts may take to start.
const SPEECH_WITHIN_SECONDS: f64 = 10.0;
/// Left before the first syllable so its soft start survives.
const PRE_ROLL_SECONDS: f64 = 0.05;
/// The pause The Neighborhood Listen leaves between theme and talk (median of 160 episodes:
/// 0.44 s, nine in ten under 0.67 s).
const USUAL_PAUSE_SECONDS: f64 = 0.45;
/// A longer pause after the theme is dead air, cut down to the usual one.
const LONGEST_PAUSE_SECONDS: f64 = 1.0;

/// Where the opening cut ends.
pub fn opening_end(theme: &JingleMatch, first_in_supercut: bool, envelope: &Envelope) -> f64 {
    let Some(talk) = envelope.first_above(theme.end, theme.end + SPEECH_WITHIN_SECONDS, SPEECH_DB) else {
        return theme.end;
    };
    if first_in_supercut {
        return (talk - PRE_ROLL_SECONDS).max(theme.end);
    }
    if talk - theme.end > LONGEST_PAUSE_SECONDS { talk - USUAL_PAUSE_SECONDS } else { theme.end }
}

/// `cuts` led by the opening cut. Cuts that began inside the theme give way to it: they are the
/// labels' reading of the introduction, and the theme says exactly where it ends.
pub fn with_opening(cuts: Vec<Cut>, theme: &JingleMatch, end: f64) -> Vec<Cut> {
    let name = theme.recording.file_stem().unwrap_or_default().to_string_lossy();
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

    fn theme(end: f64) -> JingleMatch {
        JingleMatch { recording: "/themes/show.flac".into(), start: end - 27.0, end, score: 0.99 }
    }

    /// Theme music to 43.8 s, `pause` seconds of near-silence, then the hosts talking.
    fn envelope(pause: f64) -> Envelope {
        let frames = (60.0 / FRAME_SECONDS) as usize;
        Envelope::from_mean_squares((0..frames).map(|frame| {
            let time = frame as f64 * FRAME_SECONDS;
            if (43.8..43.8 + pause).contains(&time) { 1e-9 } else { 0.05 }
        }))
    }

    #[test]
    fn later_episodes_start_where_the_theme_ends_and_the_first_on_the_hosts_first_words() {
        assert_eq!(opening_end(&theme(43.8), false, &envelope(0.4)), 43.8);
        let first = opening_end(&theme(43.8), true, &envelope(0.4));
        assert!((first - 44.15).abs() < 1e-6, "{first}");
    }

    #[test]
    fn dead_air_after_the_theme_shrinks_to_the_usual_pause() {
        let later = opening_end(&theme(43.8), false, &envelope(5.4));
        assert!((later - 48.75).abs() < 1e-6, "{later}");
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
