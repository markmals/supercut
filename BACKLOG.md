# Backlog

Agreed order: get The Neighborhood Listen (NHL) right end to end, then publishing, then the other shows.

## The Neighborhood Listen

1. S09 pt 2: S09E08–E13 plus Bonus Room #30, the one half season never made. Redraft (the saved plan predates the theme, outro, bumper and fade fixes), review, preview, render on approval.
2. A speculative cut of S11 pt 1.
3. Hand cuts no recording places yet: the openings of S05E01 and Bonus Rooms #1, #8 and #10, and the endings of most of S01–S04 and Bonus Rooms #1 and #3 (their closing music differs from the S05–S10 outro).

## Publishing

- A private R2 feed per show behind a long secret URL. An unfilled RSS template from September 2025 sits in `Podcasts/feeds/*.rss`.
- Replace the Castro-specific `--sideload` with a generic publish step (agreed): any number of configured folder copies, each file named after the supercut's title (Castro shows a sideload's file name as its episode title; the existing sideloads use `<Show> - <range>.mp3`), plus the R2 feed. Castro becomes one configured folder the user can drop later.

## Other shows (after NHL)

- Threedom: feed configured; needs segments, rules learned from the hand-made supercuts in `Podcasts/threedom/`, and its recurring recordings.
- That's Funny: needs its feed URL.
- The Headgum Podcast: needs its feed URL; hand-made collections 1–70 exist in `Podcasts/the-headgum-podcast/` to learn from.
- If I Were You: needs its feed URL; collection 1–10 exists in `Podcasts/if-i-were-you/`.

## Housekeeping

- Adversarial review of the commits `c3fff5a` to `a0a5b67` (fades, final segment, bumpers, final interstitial, preview reel, review line).
- Confirm the true-peak ceiling holds after MP3 encoding (the limiter runs before the encoder).
- `--output` means a format on some commands and a file on others.
- `plans draft` does not read the show's rules file; the agent applies it.
- Shows without a `chapters` layout get a chapter per labeled segment, and those go stale after hand edits (a kept low-confidence `classified` keeps its chapter, a hand cut can leave a seconds-long one) and flicker when the labels flip inside a segment. `plans validate` only reports chapters that fall wholly inside a cut; it could also flag chapters a few seconds long.
