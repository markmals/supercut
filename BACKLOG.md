# Backlog

Agreed order: get The Neighborhood Listen (NHL) right end to end, then publishing, then the other shows.

## The Neighborhood Listen

1. NHL redux: every season from S01 to S11-so-far is rendered into `Podcasts/nhl-redux`, one file per season (about 177 hours; S11 so far is Bonus Room #37 and S11E01–E02), from the plans `plans/the-neighborhood-listen-S01.json` … `S11.json`. The plans were reconciled with the hand-made cuts (median edge differences 0.00–0.17 s; openings deliberately stay at the theme's end) and carry the user's answers on the judgment calls, which are in the show's rules. Bonus Room #36 is dropped; the CRISIS crossover closes season 10. Next: the user listens and reports anything that sounds wrong.
2. Break ends lose the hosts' chatter: classified cuts now end right after the ad (`cut_end: after-own-speech`, keeping the return sting), but in the last pause before the closing phrase. In about one hand-made break in seven the cut ends in an earlier pause, mostly to keep talk between the ad and "welcome back" ("Doug, do you want to bring us back?", S01–S04); the draft cuts that talk. There is no single return sting to place ends by: covering the 153 hand-made break ends takes 21 recordings, and the ends vary by about ±1.5 s against the same one.
3. Ads no phrase fences: the draft still keeps the unsure second ads of S01E04 and S07E02 (no closing phrase after them) and the first of S05E02 (announced "we're going to take a break. When we come back…"; adding "take a break" pulled other break starts early).
4. Words Whisper stretches over music hide pauses: S02E11's "welcome" spans the 5 s return sting, so the break's cut ends at the sting's start and anchors on the wrong time. `refine::gaps` trusts every word's timing.

## Publishing

- A private R2 feed per show behind a long secret URL, as a second target of `plans publish` beside its `publish_folders` copies. An unfilled RSS template from September 2025 sits in `Podcasts/feeds/*.rss`. Needs the bucket, its public URL and credentials.

## Other shows (after NHL)

- Threedom: feed configured; needs segments, rules learned from the hand-made supercuts in `Podcasts/threedom/`, and its recurring recordings.
- That's Funny: needs its feed URL.
- The Headgum Podcast: needs its feed URL; hand-made collections 1–70 exist in `Podcasts/the-headgum-podcast/` to learn from.
- If I Were You: needs its feed URL; collection 1–10 exists in `Podcasts/if-i-were-you/`.
- supercut must stay generic: each show's specifics belong in its config and rules, not the code. Some constants are tuned on NHL alone: the theme-to-talk pause (`labels/opening.rs`), the 15 s lead that marks credits read aloud (`labels/closing.rs`), and the edge-search weights (`labels/refine.rs`). Check them against the second show's hand-made supercuts before trusting them, and make one a setting only where that show needs a different value.

## Housekeeping

- The true-peak ceiling does not hold after encoding. `alimiter` limits sample peaks, and inter-sample peaks and the MP3 encoder overshoot it. The redux renders, which barely touch the limiter, measure -1.48 dBTP (S11) and -2.20 (S01) against the -1.5 ceiling. Ten minutes of S11E01 raised 8 dB measure -0.58 dBTP before encoding and -0.27 after. Limiting a 4× oversampled signal holds it before encoding (-1.43) but not after (-0.26), so a fix needs both oversampled limiting and a check after encoding: put the measured true peak in the render receipt, and re-render or warn when it is over.
- `history recover` files any unmatched tail of an export as an interstitial, so an export that ends in a few seconds of silence gets a recovered plan that plays the show's interstitial after its last episode (`history/recover.rs`, `interstitial_follows`; from the adversarial review of `c3fff5a`..`a0a5b67`, not reproduced).
- `plans draft` does not read the show's rules file; the agent applies it.
- Windows is untested: the code builds and passes the suite on macOS and Linux (Tangerine), but nothing has compiled or run it on Windows. A CI matrix (macOS, Ubuntu, Windows) running `cargo test` would cover all three; it needs the repo on GitHub.
