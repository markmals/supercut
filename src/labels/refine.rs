//! Moving a labeled cut's edges onto the pauses that fence the segment. Labels are only as
//! precise as a window; the music stings around a segment are exact.

use crate::analysis::Envelope;
use crate::config::{CutEnd, CutStart};
use crate::labels::QUIET_DB;
use crate::plan::EDGE_FADE_SECONDS;
use crate::transcript::Transcript;

/// Shortest pause a cut edge can land in.
const MIN_GAP_SECONDS: f64 = 0.6;
/// Audio left either side of the cut, so no word loses its tail.
const PADDING_SECONDS: f64 = 0.1;
/// Transcripts end words early: a voice can still be trailing off this long after its last
/// word's end (0.3 s in the NHL goodbyes before the closing music).
const VOICE_TAIL_SECONDS: f64 = 0.5;
/// How far from the speech edge a cut that keeps the music may hide, looking for the quietest
/// moment rather than clipping a sting mid-note.
const QUIETEST_WITHIN_SECONDS: f64 = 1.5;
/// Seconds of pause a second of distance from the labeled edge is worth: a long sting wins over
/// a nearer breath, a distant pause does not.
const PROXIMITY_WEIGHT: f64 = 0.1;
/// How much more a wordless stretch with music in it counts than a silent pause of the same
/// length: segments are fenced by stings, conversations pause in silence.
const MUSIC_WEIGHT: f64 = 2.0;
/// Fade for an edge that lands on audio still playing.
const LOUD_EDGE_FADE_SECONDS: f64 = 0.25;
/// Sound this much louder than the audio at an edge is a voice or a note a fade must not touch.
const LOUDER_DB: f32 = 10.0;
/// A pause starts where the word before it ends; this absorbs overlapping word timings.
const ANCHOR_TOLERANCE_SECONDS: f64 = 0.05;
/// Furthest the pause that finishes an anchor phrase's sentence may be from the phrase.
const ANCHOR_REACH_SECONDS: f64 = 15.0;

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

/// Edges the transcript pins down: the end of the phrase said right before the segment and the
/// start of the phrase said right after it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Anchors {
    pub start_after: Option<f64>,
    pub end_before: Option<f64>,
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
/// policy asks, with fades where the audio at an edge is still playing. An anchored edge uses
/// the first pause after its phrase (the hosts finish their sentence) or the last pause before
/// it; the others the best-fencing pause near the label.
pub fn refine(bounds: (f64, f64), policy: Policy, anchors: Anchors, search: Search, gaps: &[Gap], envelope: &Envelope) -> Edges {
    let before_end = |gap: &&Gap| gap.middle() < bounds.1;
    let after_start = |gap: &&Gap| gap.middle() > bounds.0;
    // Nothing is kept before a segment that opens the episode: cut from the top of the file.
    let start = if bounds.0 <= PADDING_SECONDS {
        0.0
    } else if let Some(phrase_end) = anchors.start_after {
        first_pause_after(gaps, phrase_end).map_or(phrase_end + PADDING_SECONDS, |gap| cut_start(*gap, policy.start, envelope))
    } else {
        match best_gap(gaps, bounds.0, search.around_start, before_end, envelope) {
            Some(gap) => cut_start(*gap, policy.start, envelope),
            None => bounds.0,
        }
    };
    let end = if let Some(phrase_start) = anchors.end_before {
        last_pause_before(gaps, phrase_start).map_or(phrase_start - PADDING_SECONDS, |gap| cut_end(*gap, policy.end, envelope))
    } else {
        match best_gap(gaps, bounds.1, search.around_end, after_start, envelope) {
            Some(gap) => cut_end(*gap, policy.end, envelope),
            None => bounds.1,
        }
    };
    if start >= end {
        return edges(bounds.0, bounds.1, gaps, envelope);
    }
    edges(start, end, gaps, envelope)
}

/// The first pause once a phrase has been said, within reach of it.
fn first_pause_after(gaps: &[Gap], phrase_end: f64) -> Option<&Gap> {
    gaps.iter()
        .find(|gap| gap.start >= phrase_end - ANCHOR_TOLERANCE_SECONDS)
        .filter(|gap| gap.start - phrase_end <= ANCHOR_REACH_SECONDS)
}

/// The last pause before a phrase is said, within reach of it.
fn last_pause_before(gaps: &[Gap], phrase_start: f64) -> Option<&Gap> {
    gaps.iter()
        .rev()
        .find(|gap| gap.end <= phrase_start + ANCHOR_TOLERANCE_SECONDS)
        .filter(|gap| phrase_start - gap.end <= ANCHOR_REACH_SECONDS)
}

fn cut_start(gap: Gap, policy: CutStart, envelope: &Envelope) -> f64 {
    match policy {
        CutStart::AfterPreviousSpeech => {
            let earliest = (gap.start + PADDING_SECONDS).min(gap.end);
            let latest = (earliest + VOICE_TAIL_SECONDS).min(gap.end - PADDING_SECONDS).max(earliest);
            quietest(envelope, earliest, latest)
        }
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

/// The edges with their fades. A fade's room ends at the nearest word, or nearer, at the nearest
/// sound clearly louder than the edge: transcripts end words before the voice stops.
fn edges(start: f64, end: f64, gaps: &[Gap], envelope: &Envelope) -> Edges {
    let louder = |time: f64| envelope.level_at(time) + LOUDER_DB;
    let room_before = gap_around(gaps, start).map(|gap| {
        let voice_ends = envelope.last_above(gap.start, start, louder(start)).unwrap_or(gap.start);
        start - voice_ends.max(gap.start)
    });
    let room_after = gap_around(gaps, end).map(|gap| {
        let voice_starts = envelope.first_above(end, gap.end, louder(end)).unwrap_or(gap.end);
        voice_starts.min(gap.end) - end
    });
    Edges { start, end, fade_out: fade(start, room_before, envelope), fade_in: fade(end, room_after, envelope) }
}

fn gap_around(gaps: &[Gap], time: f64) -> Option<&Gap> {
    gaps.iter().find(|gap| gap.start <= time && time <= gap.end)
}

/// Audio that is still playing at an edge needs a fade, one no longer than the `room` before
/// the nearest voice so it never fades one; a silent edge splices cleanly.
fn fade(time: f64, room: Option<f64>, envelope: &Envelope) -> Option<f64> {
    let seconds = room.map_or(LOUD_EDGE_FADE_SECONDS, |room| room.clamp(EDGE_FADE_SECONDS, LOUD_EDGE_FADE_SECONDS));
    (envelope.level_at(time) > QUIET_DB).then_some(seconds)
}

/// The pause near `boundary` inside `search` most likely to fence the segment: long, carrying
/// music, and close to where the labels put the edge.
fn best_gap<'a>(
    gaps: &'a [Gap],
    boundary: f64,
    search: (f64, f64),
    keeps_the_cut: impl Fn(&&'a Gap) -> bool,
    envelope: &Envelope,
) -> Option<&'a Gap> {
    let score = |gap: &Gap| {
        let music = if envelope.mean_db(gap.start, gap.end) > QUIET_DB { MUSIC_WEIGHT } else { 1.0 };
        gap.seconds() * music - PROXIMITY_WEIGHT * (gap.middle() - boundary).abs()
    };
    gaps.iter()
        .filter(|gap| gap.end > search.0 && gap.start < search.1)
        .filter(keeps_the_cut)
        .reduce(|best, gap| if score(gap) > score(best) { gap } else { best })
}

#[cfg(test)]
mod tests {
    use super::{Anchors, Gap, Policy, Search, gaps, refine};
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
        let edges = refine((97.0, 163.0), Policy::default(), Anchors::default(), around_the_ad(), &gaps, &envelope);
        assert!((edges.start - 100.0).abs() < 1e-6, "cut starts at {}", edges.start);
        assert!((edges.end - 159.9).abs() < 1e-6, "cut ends at {}", edges.end);
    }

    #[test]
    fn a_long_pause_well_past_the_boundary_does_not_pull_the_cut_into_the_hosts() {
        // The hosts are back at 160 s; at 175 s one of them pauses for four seconds.
        let transcript = transcript(&[(0.0, 100.0), (102.0, 158.0), (160.0, 175.0), (179.0, 260.0)]);
        let envelope = envelope(260.0, &[(158.0, 160.0)]);
        let gaps = gaps(&transcript, 260.0);
        let edges = refine((97.0, 160.5), Policy::default(), Anchors::default(), around_the_ad(), &gaps, &envelope);
        assert!((edges.end - 159.9).abs() < 1e-6, "cut ends at {}", edges.end);
    }

    #[test]
    fn an_anchor_phrase_overrides_a_late_label_and_a_tempting_pause() {
        // Labels ran 20 s past the ad, past a long silent pause; the hosts' "welcome back" starts
        // at 160 s, right after the ad's play-out sting.
        let transcript = transcript(&[(0.0, 100.0), (102.0, 158.0), (160.0, 175.0), (179.0, 260.0)]);
        let envelope = envelope(260.0, &[(158.0, 160.0)]);
        let gaps = gaps(&transcript, 260.0);
        let anchors = Anchors { start_after: Some(100.0), end_before: Some(160.0) };
        let edges = refine((80.0, 180.0), Policy::default(), anchors, around_the_ad(), &gaps, &envelope);
        assert!((edges.start - 100.1).abs() < 1e-6, "cut starts at {}", edges.start);
        assert!((edges.end - 159.9).abs() < 1e-6, "cut ends at {}", edges.end);
    }

    #[test]
    fn a_cut_edge_on_music_fades_but_never_into_a_word() {
        let (transcript, envelope) = ad_episode();
        let gaps = gaps(&transcript, 260.0);
        let edges = refine((97.0, 163.0), Policy::default(), Anchors::default(), around_the_ad(), &gaps, &envelope);
        assert_eq!(edges.fade_out, None, "the ad is preceded by room tone");
        // The sting plays up to the hosts' first word at 160 s; the cut ends 0.1 s before it.
        let fade_in = edges.fade_in.unwrap();
        assert!((fade_in - 0.1).abs() < 1e-6, "fades in over {fade_in} s, into the hosts' first word");
    }

    #[test]
    fn a_cut_after_the_last_word_waits_for_the_voice_to_die_away() {
        // The transcript ends the hosts' "bye" at 99.9 s, but the voice trails on to 100.25 s;
        // the closing music hits at 100.3 s.
        let transcript = transcript(&[(0.0, 100.0), (110.0, 160.0)]);
        let envelope = envelope(160.0, &[(99.5, 100.25), (100.3, 101.0)]);
        let gaps = gaps(&transcript, 160.0);
        let search = Search { around_start: (95.0, 105.0), around_end: (105.0, 115.0) };
        let edges = refine((99.0, 112.0), Policy::default(), Anchors::default(), search, &gaps, &envelope);
        assert!((edges.start - 100.25).abs() < 0.011, "cut starts at {}", edges.start);
        assert_eq!(edges.fade_out, None, "the edge is in the hush before the music, so nothing fades");
    }

    #[test]
    fn a_cut_that_keeps_the_lead_in_sting_starts_where_the_sting_ends() {
        let (transcript, envelope) = stung_episode();
        let gaps = gaps(&transcript, 260.0);
        let policy = Policy { start: CutStart::BeforeOwnSpeech, end: CutEnd::BeforeNextSpeech };
        let edges = refine((97.0, 163.0), policy, Anchors::default(), around_the_ad(), &gaps, &envelope);
        assert!((edges.start - 101.6).abs() < 0.02, "cut starts at {}", edges.start);
        assert_eq!(edges.fade_out, None, "the sting has finished by then");
        assert!((edges.end - 159.9).abs() < 1e-6, "the play-out sting is still cut");
    }

    #[test]
    fn a_cut_that_keeps_the_play_out_sting_ends_where_the_sting_ends() {
        let (transcript, envelope) = stung_episode();
        let gaps = gaps(&transcript, 260.0);
        let policy = Policy { start: CutStart::AfterPreviousSpeech, end: CutEnd::AfterOwnSpeech };
        let edges = refine((97.0, 163.0), policy, Anchors::default(), around_the_ad(), &gaps, &envelope);
        assert!((edges.end - 158.6).abs() < 0.02, "cut ends at {}", edges.end);
        assert_eq!(edges.fade_in, None, "the sting has finished by then");
        let fade_out = edges.fade_out.unwrap();
        assert!((fade_out - 0.1).abs() < 1e-6, "the lead-in sting is cut mid-note, faded back to the last word only: {fade_out}");
    }

    #[test]
    fn a_boundary_with_no_pause_nearby_stays_where_the_labels_put_it() {
        let (transcript, envelope) = ad_episode();
        let gaps = gaps(&transcript, 260.0);
        let search = Search { around_start: (30.0, 60.0), around_end: (200.0, 240.0) };
        let edges = refine((45.0, 220.0), Policy::default(), Anchors::default(), search, &gaps, &envelope);
        assert_eq!((edges.start, edges.end), (45.0, 220.0));
    }

    #[test]
    fn a_cut_that_opens_the_episode_starts_at_the_file() {
        let (transcript, envelope) = ad_episode();
        let gaps = gaps(&transcript, 260.0);
        let search = Search { around_start: (0.0, 35.0), around_end: (60.0, 130.0) };
        let edges = refine((0.0, 95.0), Policy::default(), Anchors::default(), search, &gaps, &envelope);
        assert_eq!(edges.start, 0.0, "the preamble is cut from the top, not from its first pause");
    }

    #[test]
    fn gaps_are_the_stretches_without_words() {
        let (transcript, _) = ad_episode();
        let found = gaps(&transcript, 260.0);
        assert_eq!(found, [Gap { start: 99.9, end: 102.0 }, Gap { start: 157.9, end: 160.0 }]);
    }
}
