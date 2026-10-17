# Backlog

Agreed order: get The Neighborhood Listen (NHL) right end to end, then publishing, then the other shows.

## The Neighborhood Listen

1. Recognize each episode's closing music by sound so the credits and closing music are cut without relying on the labels (in progress).
2. S09 pt 2: S09E08–E13 plus Bonus Room #30, the one half season never made. Redraft (the saved plan predates the theme, bumper and fade fixes), review, preview, render on approval.
3. Decide the end-of-episode plug rule: the hand-made supercuts cut from the Maximus/bonus-content pitch to the end when only a goodbye follows it (S10E01, S10E03) and kept it when the hosts riffed after it (S10E02, S10E06); `plans draft` currently always keeps it.
4. A speculative cut of S11 pt 1.
5. Openings no theme recording matches still need a hand cut: S05E01 and Bonus Rooms #1, #8 and #10.

## Publishing

- A private R2 feed per show behind a long secret URL. An unfilled RSS template from September 2025 sits in `Podcasts/feeds/*.rss`.
- Replace the Castro-specific `--sideload` with a generic publish step: folder copies named after the supercut's title (Castro shows a sideload's file name as its episode title; the existing sideloads use `<Show> - <range>.mp3`) and the R2 feed.

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
