# Backlog

Agreed order: get The Neighborhood Listen (NHL) right end to end, then publishing, then the other shows.

## The Neighborhood Listen

1. NHL redux: rebuild every supercut from S01 to S11-so-far into `Podcasts/nhl-redux`, one per whole season (a long MP3 is fine; Logic was the limit). All 149 episodes are transcribed and labeled; all eleven season plans (`plans/the-neighborhood-listen-S01.json` … `S11.json`) are drafted and reconciled with the hand-made cuts (median edge differences 0.00–0.17 s; openings deliberately stay at the theme's end), and validate. Next: the user's answers on the open judgment calls, then render on approval. Bonus Room #36 is dropped; the CRISIS crossover closes season 10 (cuts and chapters in the show's rules).
2. A speculative cut of S11 so far (S11E01–E02, Bonus Room #37): part of the redux.
3. Hand cuts no recording places yet: the openings of S05E01 and Bonus Rooms #1, #8 and #10; the endings of S04E03, E04, E12, E13 and Bonus Rooms #1 and #3; and S03E06–E09, E11 and E13, whose credits are read aloud so the draft cuts 29 s late without warning.
4. Ad-break ends: the hand-made cuts end inside the return sting, 1–5 s before "welcome back"; the draft ends just before "welcome back". Neither `cut_end` setting reproduces the hand-made splices (tested on S06 and S10: about half the ends stay more than 1 s off either way), so every draft's break ends are moved by hand. The return sting is a recurring recording; a configured `stings` list could place these ends the way `outros` places the credits cut.
5. Missed second ads: the draft kept a real classified ad the labels were unsure of (p 0.54–0.67) in S01E04, S01E13, S02E02, S02E11, S05E02, S05E05, S06E08, S06E12, S07E02, S07E09, S09E03, S09E07 and S10E09. A low-confidence `classified` fenced by the show's phrases ("be right back" before, "welcome back" after) could be cut rather than only reported.

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
- supercut panics ("failed printing to stdout: Broken pipe") when the program reading its output exits first, as `supercut … | head` does; it should exit quietly.
- Windows is untested: the code builds and passes the suite on macOS and Linux (Tangerine), but nothing has compiled or run it on Windows. A CI matrix (macOS, Ubuntu, Windows) running `cargo test` would cover all three; it needs the repo on GitHub.
