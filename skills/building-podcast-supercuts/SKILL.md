---
name: building-podcast-supercuts
description: Use when asked to make, plan, re-cut or publish a podcast supercut (many episodes stitched into one long MP3), to cut ads, classifieds, plugs or guest segments out of podcast episodes, or to learn from supercuts the user previously edited by hand in Logic Pro.
---

# Building podcast supercuts

## Overview

The `supercut` CLI does all audio work; you make the editorial calls from transcripts, segment labels and the show's rules, and the user approves before anything is published. Never guess commands or flags: `supercut cli search "<task>"` finds operations, `supercut schema <resource> <operation>` describes one exactly.

## Workflow

1. Orient: `supercut shows list`, then `supercut rules get <show>`. The rules record the user's taste (grouping, what to cut, which bonus episodes to include). `supercut history list <show>` and `history get` show how past supercuts were cut.
2. Choose episodes per the rules; ask the user when the rules say ask. `episodes sync`, `episodes list`, `episodes download`.
3. `transcripts create` (about 5 minutes per episode hour; run in the background) and `labels create` (paid; `--dry-run` first to see the request count).
4. `plans draft <show> <ids…> --title … --output …` (add `--also-cut <id>=<label>` for per-episode removals the user asked for).
5. Check the draft: `labels get <show> <id> --output text` for low-confidence segments, `transcripts get --from --to --output text` to read around them, `episodes inspect --at` for doubtful cut edges. Edit the plan JSON to fix what is wrong.
6. `plans validate <plan>` must report `ok: true`. When a comparable hand-made supercut exists, `plans compare <plan> <history plan>` shows where the draft disagrees with past choices.
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
