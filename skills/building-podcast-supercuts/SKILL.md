---
name: building-podcast-supercuts
description: Use when asked to make, plan, re-cut or publish a podcast supercut (many episodes stitched into one long MP3), to cut ads, classifieds, plugs or guest segments out of podcast episodes, or to learn from supercuts the user previously edited by hand in Logic Pro.
---

# Building podcast supercuts

## Overview

The `supercut` CLI does all audio work; you make the editorial calls from transcripts, segment labels and the show's rules, and the user approves before anything is published. Never guess commands or flags: `supercut cli search "<task>"` finds operations, `supercut schema <resource> <operation>` describes one exactly.

## Workflow

1. Orient: `supercut shows list`, then `supercut rules get <show>`. The rules record the user's taste (grouping, what to cut, which bonus episodes to include). `supercut config get` shows each show's interstitial, its theme, outro and bumper recordings, and its segment kinds. `supercut history list <show>` and `history get` show how past supercuts were cut.
2. Choose episodes per the rules; ask the user when the rules say ask. `episodes sync`, `episodes list`, `episodes download`.
3. `transcripts create` (about 5 minutes per episode hour; run in the background) and `labels create` (paid; `--dry-run` first to see the request count).
4. `plans draft <show> <ids…> --title … --output …` (add `--also-cut <id>=<label>` for per-episode removals the user asked for). Openings come from the show's `themes` and the episode order: the first episode is cut to the hosts' first words, the rest to their theme's end. Each episode is cut from its `outros` recording to the end. An episode that ends with one of the show's `bumpers` keeps it and gets `"interstitial_after": false`; every other episode, the last included, is followed by the interstitial. A show with a `chapters` layout gets one chapter per stretch between breaks, titled from the episode title; otherwise a chapter per labeled segment. Redraft after reordering episodes, and fix by hand each opening, ending or chapter layout the draft's `warnings` name.
5. Check every cut, not only the flagged ones: read `plans review <plan> --output text` and confirm each cut's "heard before"/"heard after" lines land where the rules say (e.g. the hosts' "welcome back" must be kept). `plans draft`'s `low_confidence` lists segments it was unsure about, kept or cut. Read around doubtful spots with `transcripts get --from --to --output text`; `episodes inspect --at` gives word timings and levels for placing an edge. Edit the plan JSON to fix what is wrong, chapters included: a chapter that starts where a cut ends moves with that cut's end when you move it. With chapters per labeled segment (no `chapters` layout), a segment you keep as content (a `classified` that is really a community post) and a segment a hand cut leaves only seconds of (a `Plug` reduced to "Thank you for listening") lose their chapter entries, so the chapter before them runs on.
6. `plans validate <plan>` must report `ok: true`; its warnings point at edges on sounding audio. For a supercut you remade, `plans compare <plan> <history plan>` shows where the draft disagrees with the hand-made one (it compares shared episodes only); for a new one, read `history get` of the previous part instead.
7. Ask for approval: give the user `plans review <plan> --output text` and the path from `plans preview <plan>`.
8. After approval: `plans render <plan> --dry-run`, then `plans render <plan>`; add `--sideload` only when asked. Report the receipt (path, duration, chapters).

## Rules of engagement

| Situation | Do |
|---|---|
| A command fails | Read the JSON error's `code` and run its `hint`; `RATE_LIMITED`/`UNAVAILABLE` are retryable |
| `UNAUTHENTICATED` from `labels create` | Ask the user to unlock 1Password, then retry |
| Output file already exists | Ask before passing `--force` |
| Rules and the user's request disagree | Follow the request; offer to update the rules file (`rules get` prints its path) |
| Times in plans | Positions in the episode's own audio file, `h:mm:ss.sss` |
| A show needs an interstitial, theme, outro or bumper recording | Find it with `episodes inspect` and cut it with `episodes clip`. An interstitial or bumper needs silence on both sides and a theme must end where its music does; an outro is its first second or so from the hush before it, so its end landing on sound is expected. Add recordings to the config, then check `episodes analyze` finds them in every episode they belong to |
