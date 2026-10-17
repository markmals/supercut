//! Moving a labeled cut's edges onto the pauses that fence the segment. Labels are only as
//! precise as a window; the music stings around a segment are exact.

use crate::analysis::Envelope;
use crate::config::{CutEnd, CutStart};
use crate::labels::QUIET_DB;
use crate::transcript::Transcript;

/// Shortest pause a cut edge can land in.
const MIN_GAP_SECONDS: f64 = 0.6;
/// Audio left either side of the cut, so no word loses its tail.
const PADDING_SECONDS: f64 = 0.1;
/// How far from the speech edge a cut that keeps the music may hide, looking for the quietest
/// moment rather than clipping a sting mid-note.
const QUIETEST_WITHIN_SECONDS: f64 = 1.5;
/// Seconds of pause a second of distance from the labeled edge is worth: a long sting wins over
/// a nearer breath, a distant pause does not.
const PROXIMITY_WEIGHT: f64 = 0.3;
/// Furthest a cut edge may move from the labeled boundary. The fine pass places boundaries to
/// the utterance, so a pause further out belongs to the speech on the other side.
const MAX_EDGE_SHIFT_SECONDS: f64 = 6.0;
/// Fade for an edge that lands on audio still playing.
const LOUD_EDGE_FADE_SECONDS: f64 = 0.25;

/// A stretch with no words in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gap {
    pub start: f64,
    pub end: f64,
}

impl Gap {
    fn seconds(self) -> f64 {
        self.end - self.start
    }

    fn middle(self) -> f64 {
        (self.start + self.end) / 2.0
    }
}

/// Where each edge of a cut may move to: the windows either side of the labeled boundary.
#[derive(Clone, Copy, Debug)]
pub struct Search {
    pub around_start: (f64, f64),
    pub around_end: (f64, f64),
}

/// What the show's config says about where cuts of this segment begin and end.
#[derive(Clone, Copy, Debug, Default)]
pub struct Policy {
    pub start: CutStart,
    pub end: CutEnd,
}

/// A cut's edges once refined.
#[derive(Clone, Copy, Debug)]
pub struct Edges {
    pub start: f64,
    pub end: f64,
    pub fade_out: Option<f64>,
    pub fade_in: Option<f64>,
}

/// Every pause in the episode long enough to carry a cut edge, the run-up to the first word and
/// the run-out after the last one included.
pub fn gaps(transcript: &Transcript, duration: f64) -> Vec<Gap> {
    let mut gaps = Vec::new();
    let mut spoken_to = 0.0;
    for word in transcript.words() {
        push_gap(&mut gaps, spoken_to, word.start);
        spoken_to = spoken_to.max(word.end);
    }
    push_gap(&mut gaps, spoken_to, duration);
    gaps
}

fn push_gap(gaps: &mut Vec<Gap>, start: f64, end: f64) {
    if end - start >= MIN_GAP_SECONDS {
        gaps.push(Gap { start, end });
    }
}

/// The cut `bounds` moved onto the pauses that bracket them, each edge placed as the segment's
/// policy asks, with fades where the audio at an edge is still playing.
pub fn refine(
    bounds: (f64, f64),
    policy: Policy,
    search: Search,
    gaps: &[Gap],
    envelope: &Envelope,
) -> Edges {
    let before_end = |gap: &&Gap| gap.middle() < bounds.1;
    let after_start = |gap: &&Gap| gap.middle() > bounds.0;
    // Nothing is kept before a segment that opens the episode: cut from the top of the file.
    let start = if bounds.0 <= PADDING_SECONDS {
        0.0
    } else {
        match best_gap(gaps, bounds.0, search.around_start, before_end) {
            Some(gap) => cut_start(*gap, policy.start, envelope),
            None => bounds.0,
        }
    };
    let end = match best_gap(gaps, bounds.1, search.around_end, after_start) {
        Some(gap) => cut_end(*gap, policy.end, envelope),
        None => bounds.1,
    };
    if start >= end {
        return edges(bounds.0, bounds.1, envelope);
    }
    edges(start, end, envelope)
}

fn cut_start(gap: Gap, policy: CutStart, envelope: &Envelope) -> f64 {
    match policy {
        CutStart::AfterPreviousSpeech => (gap.start + PADDING_SECONDS).min(gap.end),
        CutStart::BeforeOwnSpeech => {
            let latest = (gap.end - PADDING_SECONDS).max(gap.start);
            quietest(envelope, (latest - QUIETEST_WITHIN_SECONDS).max(gap.start), latest)
        }
    }
}

fn cut_end(gap: Gap, policy: CutEnd, envelope: &Envelope) -> f64 {
    match policy {
        CutEnd::BeforeNextSpeech => (gap.end - PADDING_SECONDS).max(gap.start),
        CutEnd::AfterOwnSpeech => {
            let earliest = (gap.start + PADDING_SECONDS).min(gap.end);
            quietest(envelope, earliest, (earliest + QUIETEST_WITHIN_SECONDS).min(gap.end))
        }
    }
}

/// The quietest 10 ms frame in `from`..`to`: where a cut through music is least audible.
fn quietest(envelope: &Envelope, from: f64, to: f64) -> f64 {
    let levels = envelope.levels_db();
    let (first, last) = (envelope.frame_at(from), envelope.frame_at(to));
    let frame = (first..=last.max(first))
        .reduce(|best, frame| if levels[frame] < levels[best] { frame } else { best })
        .unwrap_or(first);
    Envelope::time_of(frame)
}

fn edges(start: f64, end: f64, envelope: &Envelope) -> Edges {
    Edges { start, end, fade_out: fade(start, envelope), fade_in: fade(end, envelope) }
}

/// Audio that is still playing at an edge needs a fade; a silent edge splices cleanly.
fn fade(time: f64, envelope: &Envelope) -> Option<f64> {
    (envelope.level_at(time) > QUIET_DB).then_some(LOUD_EDGE_FADE_SECONDS)
}

/// The longest pause near `boundary` inside `search`, discounted by how far it sits from it.
fn best_gap<'a>(
    gaps: &'a [Gap],
    boundary: f64,
    search: (f64, f64),
    keeps_the_cut: impl Fn(&&'a Gap) -> bool,
) -> Option<&'a Gap> {
    let score = |gap: &Gap| gap.seconds() - PROXIMITY_WEIGHT * (gap.middle() - boundary).abs();
    let (low, high) = (search.0.max(boundary - MAX_EDGE_SHIFT_SECONDS), search.1.min(boundary + MAX_EDGE_SHIFT_SECONDS));
    gaps.iter()
        .filter(|gap| gap.end > low && gap.start < high)
        .filter(keeps_the_cut)
        .reduce(|best, gap| if score(gap) > score(best) { gap } else { best })
}

#[cfg(test)]
mod tests {
    use super::{Gap, Policy, Search, gaps, refine};
    use crate::analysis::{Envelope, FRAME_SECONDS};
    use crate::config::{CutEnd, CutStart};
    use crate::transcript::{Segment, Transcript, Word};

    /// Words at two a second over `spans`, with nothing in between them.
    fn transcript(spans: &[(f64, f64)]) -> Transcript {
        let mut words = Vec::new();
        for (start, end) in spans {
            let mut clock = *start;
            while clock + 0.4 <= *end {
                words.push(Word {
                    start: clock,
                    end: clock + 0.4,
                    text: "word".into(),
                    probability: None,
                });
                clock += 0.5;
            }
        }
        Transcript {
            engine: "test".into(),
            model: "test".into(),
            language: "en".into(),
            unreliable: Vec::new(),
            segments: vec![Segment { start: 0.0, end: 0.0, text: String::new(), words }],
        }
    }

    /// Quiet everywhere but `loud`, where music plays.
    fn envelope(seconds: f64, loud: &[(f64, f64)]) -> Envelope {
        let frames = (seconds / FRAME_SECONDS) as usize;
        let squares = (0..frames).map(|frame| {
            let time = frame as f64 * FRAME_SECONDS;
            let playing = loud.iter().any(|(start, end)| time >= *start && time < *end);
            if playing { 0.25 } else { 1e-9 }
        });
        Envelope::from_mean_squares(squares.collect::<Vec<_>>())
    }

    /// Hosts to 100 s, two seconds of room tone, an ad from 102 s to 158 s, a music sting, then
    /// hosts again.
    fn ad_episode() -> (Transcript, Envelope) {
        (
            transcript(&[(0.0, 100.0), (102.0, 158.0), (160.0, 260.0)]),
            envelope(260.0, &[(158.0, 160.0)]),
        )
    }

    /// Hosts to 100 s, a sting leading into the ad, the ad from 102 s to 158 s, a sting
    /// leading out of it, hosts again from 160 s.
    fn stung_episode() -> (Transcript, Envelope) {
        (
            transcript(&[(0.0, 100.0), (102.0, 158.0), (160.0, 260.0)]),
            envelope(260.0, &[(100.0, 101.6), (158.0, 158.6)]),
        )
    }

    fn around_the_ad() -> Search {
        Search { around_start: (60.0, 130.0), around_end: (130.0, 200.0) }
    }

    #[test]
    fn cut_edges_move_onto_the_pauses_that_fence_the_ad() {
        let (transcript, envelope) = ad_episode();
        let gaps = gaps(&transcript, 260.0);
        // The labels put the ad a few seconds off on each side.
        let edges = refine((97.0, 163.0), Policy::default(), around_the_ad(), &gaps, &envelope);
        assert!((edges.start - 100.0).abs() < 1e-6, "cut starts at {}", edges.start);
        assert!((edges.end - 159.9).abs() < 1e-6, "cut ends at {}", edges.end);
    }

    #[test]
    fn a_long_pause_well_past_the_boundary_does_not_pull_the_cut_into_the_hosts() {
        // The hosts are back at 160 s; at 175 s one of them pauses for four seconds.
        let transcript = transcript(&[(0.0, 100.0), (102.0, 158.0), (160.0, 175.0), (179.0, 260.0)]);
        let envelope = envelope(260.0, &[(158.0, 160.0)]);
        let edges = refine((97.0, 160.5), Policy::default(), around_the_ad(), &gaps(&transcript, 260.0), &envelope);
        assert!((edges.end - 159.9).abs() < 1e-6, "cut ends at {}", edges.end);
    }

    #[test]
    fn a_cut_edge_on_music_fades() {
        let (transcript, envelope) = ad_episode();
        let gaps = gaps(&transcript, 260.0);
        let edges = refine((97.0, 163.0), Policy::default(), around_the_ad(), &gaps, &envelope);
        assert_eq!(edges.fade_out, None, "the ad is preceded by room tone");
        assert_eq!(edges.fade_in, Some(0.25), "the hosts come back while the sting still plays");
    }

    #[test]
    fn a_cut_that_keeps_the_lead_in_sting_starts_where_the_sting_ends() {
        let (transcript, envelope) = stung_episode();
        let gaps = gaps(&transcript, 260.0);
        let policy = Policy { start: CutStart::BeforeOwnSpeech, end: CutEnd::BeforeNextSpeech };
        let edges = refine((97.0, 163.0), policy, around_the_ad(), &gaps, &envelope);
        assert!((edges.start - 101.6).abs() < 0.02, "cut starts at {}", edges.start);
        assert_eq!(edges.fade_out, None, "the sting has finished by then");
        assert!((edges.end - 159.9).abs() < 1e-6, "the play-out sting is still cut");
    }

    #[test]
    fn a_cut_that_keeps_the_play_out_sting_ends_where_the_sting_ends() {
        let (transcript, envelope) = stung_episode();
        let gaps = gaps(&transcript, 260.0);
        let policy = Policy { start: CutStart::AfterPreviousSpeech, end: CutEnd::AfterOwnSpeech };
        let edges = refine((97.0, 163.0), policy, around_the_ad(), &gaps, &envelope);
        assert!((edges.end - 158.6).abs() < 0.02, "cut ends at {}", edges.end);
        assert_eq!(edges.fade_in, None, "the sting has finished by then");
        assert_eq!(edges.fade_out, Some(0.25), "the lead-in sting is cut mid-note");
    }

    #[test]
    fn a_boundary_with_no_pause_nearby_stays_where_the_labels_put_it() {
        let (transcript, envelope) = ad_episode();
        let gaps = gaps(&transcript, 260.0);
        let search = Search { around_start: (30.0, 60.0), around_end: (200.0, 240.0) };
        let edges = refine((45.0, 220.0), Policy::default(), search, &gaps, &envelope);
        assert_eq!((edges.start, edges.end), (45.0, 220.0));
    }

    #[test]
    fn a_cut_that_opens_the_episode_starts_at_the_file() {
        let (transcript, envelope) = ad_episode();
        let gaps = gaps(&transcript, 260.0);
        let search = Search { around_start: (0.0, 35.0), around_end: (60.0, 130.0) };
        let edges = refine((0.0, 95.0), Policy::default(), search, &gaps, &envelope);
        assert_eq!(edges.start, 0.0, "the preamble is cut from the top, not from its first pause");
    }

    #[test]
    fn gaps_are_the_stretches_without_words() {
        let (transcript, _) = ad_episode();
        let found = gaps(&transcript, 260.0);
        assert_eq!(found, [Gap { start: 99.9, end: 102.0 }, Gap { start: 157.9, end: 160.0 }]);
    }
}
