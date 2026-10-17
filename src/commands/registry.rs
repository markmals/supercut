//! What clap cannot say about each operation: its side effects, what it prints, an example,
//! and words people use for it. `schema` and `cli search` combine this with clap's own
//! argument metadata; a test keeps the two in step.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Effect {
    /// Changes nothing.
    ReadOnly,
    /// Writes indexes, audio, transcripts, labels, analysis caches or plans in the library.
    WritesLibrary,
    /// Writes the config file.
    WritesConfig,
    /// Writes finished files outside the library (output and sideload folders).
    WritesOutput,
    /// Reads RSS feeds or downloads audio.
    Network,
    /// Calls the Jev API, which bills per input token.
    PaidApi,
    /// Minutes of local CPU/GPU work per episode.
    HeavyCompute,
}

pub struct Operation {
    pub path: &'static str,
    pub effects: &'static [Effect],
    /// What stdout carries on success.
    pub output: &'static str,
    pub example: &'static str,
    /// Other words for what the operation does, for `cli search`.
    pub keywords: &'static [&'static str],
}

use Effect::*;

pub const OPERATIONS: &[Operation] = &[
    Operation {
        path: "cli search",
        effects: &[ReadOnly],
        output: "ranked commands: [{command, summary, score, example}]",
        example: "supercut cli search \"cut the ads out of an episode\"",
        keywords: &["find", "discover", "help", "which command"],
    },
    Operation {
        path: "schema",
        effects: &[ReadOnly],
        output: "the operation's arguments, effects, output and example; every operation when no path is given",
        example: "supercut schema plans render",
        keywords: &["describe", "arguments", "flags", "usage", "options"],
    },
    Operation {
        path: "config get",
        effects: &[ReadOnly],
        output: "{config_file, exists, library_dir, output_dir, sideload_dir, rules_dir, shows: [{show, title, feeds, artwork, interstitial, themes, outros, bumpers, segments}], transcription, loudness, jev}",
        example: "supercut config get",
        keywords: &["context", "settings", "paths", "where", "library", "interstitial", "artwork", "segments", "show settings"],
    },
    Operation {
        path: "config init",
        effects: &[WritesConfig],
        output: "{config_file, action}",
        example: "supercut config init --dry-run",
        keywords: &["setup", "create config", "starter"],
    },
    Operation {
        path: "config validate",
        effects: &[ReadOnly],
        output: "{ok, problems: [...], tools: {ffmpeg, mlx_whisper, ...}}",
        example: "supercut config validate",
        keywords: &["doctor", "check setup", "installed", "tools"],
    },
    Operation {
        path: "shows list",
        effects: &[ReadOnly],
        output: "[{show, title, episodes, downloaded, transcribed, labeled}]",
        example: "supercut shows list",
        keywords: &["podcasts", "configured"],
    },
    Operation {
        path: "rules get",
        effects: &[ReadOnly],
        output: "{show, path, rules (Markdown)}",
        example: "supercut rules get the-neighborhood-listen",
        keywords: &["preferences", "taste", "what to cut", "editorial"],
    },
    Operation {
        path: "episodes sync",
        effects: &[Network, WritesLibrary],
        output: "{show, episodes, added: [ids], updated}",
        example: "supercut episodes sync the-neighborhood-listen",
        keywords: &["refresh", "feed", "rss", "update", "new episodes"],
    },
    Operation {
        path: "episodes list",
        effects: &[ReadOnly],
        output: "[{id, title, published, season, number, kind, duration_seconds, downloaded, transcribed, labeled}]",
        example: "supercut episodes list the-neighborhood-listen --season 10",
        keywords: &["show episodes", "ids", "season", "index"],
    },
    Operation {
        path: "episodes get",
        effects: &[ReadOnly],
        output: "the episode's feed metadata, plus {files: {audio, transcript, labels, analysis}} with each path and whether it exists",
        example: "supercut episodes get the-neighborhood-listen s10e01",
        keywords: &["details", "description", "metadata"],
    },
    Operation {
        path: "episodes download",
        effects: &[Network, WritesLibrary],
        output: "[{id, title, file, bytes, status}]",
        example: "supercut episodes download the-neighborhood-listen --season 10 --dry-run",
        keywords: &["fetch", "audio", "mp3"],
    },
    Operation {
        path: "episodes adopt",
        effects: &[WritesLibrary],
        output: "{adopted: [{id, file, method}], unmatched_files, ambiguous: [{file, ids}], rejected: [{file, id, reason}]}",
        example: "supercut episodes adopt the-neighborhood-listen ~/Music/Podcasts --dry-run",
        keywords: &["import", "existing files", "link"],
    },
    Operation {
        path: "episodes analyze",
        effects: &[WritesLibrary],
        output: "[{id, duration, integrated_lufs, range_lu, bit_rate, sample_rate, channels, theme, outro, bumper}]; each of theme/outro/bumper is {recording, start, end, score}, null when none plays, absent when the show configures none",
        example: "supercut episodes analyze the-neighborhood-listen s10e01",
        keywords: &["loudness", "levels", "measure", "lufs", "theme", "bumper", "jingle", "opening", "ending"],
    },
    Operation {
        path: "episodes inspect",
        effects: &[ReadOnly],
        output: "{from, to, quiet_threshold_db, transcribed, levels: [{at, db}], words: [{start, end, text}], non_speech: [{start, end, seconds, kind, mean_db}], quiet_spans: [{start, end}]}; --output text for one line per step",
        example: "supercut episodes inspect the-neighborhood-listen s10e01 --at 29:20 --window 20",
        keywords: &["waveform", "silence", "music", "cut point", "where to cut", "fade", "spectrogram"],
    },
    Operation {
        path: "episodes clip",
        effects: &[WritesOutput],
        output: "{episode, from, to, seconds, output, start_db, end_db, warnings, dry_run}; a warning names where sound cut off at an edge starts or dies away",
        example: "supercut episodes clip the-neighborhood-listen s10e01 --from 29:17.83 --to 29:22.66 --out sting.flac --dry-run",
        keywords: &["extract", "interstitial", "theme", "jingle", "sting", "export audio", "save a piece"],
    },
    Operation {
        path: "transcripts create",
        effects: &[WritesLibrary, HeavyCompute],
        output: "[{id, seconds, words, unreliable_spans, status: transcribed|skipped|would-transcribe}]",
        example: "supercut transcripts create the-neighborhood-listen s10e01",
        keywords: &["transcribe", "speech to text", "whisper"],
    },
    Operation {
        path: "transcripts get",
        effects: &[ReadOnly],
        output: "{episode, from, to, utterances: [{start, end, text}], unreliable: [{start, end, reason}]}; --output text for `[m:ss.s] line` text",
        example: "supercut transcripts get the-neighborhood-listen s10e01 --from 28:00 --to 31:00 --output text",
        keywords: &["read", "what was said", "words", "text"],
    },
    Operation {
        path: "labels create",
        effects: &[PaidApi, WritesLibrary],
        output: "[{id, windows, units, segments: [{label, start, end, probability}], input_tokens}]; --dry-run: [{id, windows, utterances, assumed_boundaries, requests, estimated_input_tokens}]",
        example: "supercut labels create the-neighborhood-listen --season 10 --dry-run",
        keywords: &["classify", "segments", "jev", "find ads", "detect", "advertisements"],
    },
    Operation {
        path: "labels get",
        effects: &[ReadOnly],
        output: "{episode, segments: [{label, start, end, probability, low_confidence}]}",
        example: "supercut labels get the-neighborhood-listen s10e01 --output text",
        keywords: &["segments", "structure", "sections"],
    },
    Operation {
        path: "plans draft",
        effects: &[WritesLibrary],
        output: "{plan, episodes, cuts, low_confidence: [{episode, label, start, end, probability, action}]}",
        example: "supercut plans draft the-neighborhood-listen s10e01 s10e02 --title \"NHL S10 pt 1\" --output nhl-s10-pt-1.mp3",
        keywords: &["create plan", "cut list", "edit decision list", "propose cuts", "remove ads", "skip segments"],
    },
    Operation {
        path: "plans validate",
        effects: &[ReadOnly, WritesLibrary],
        output: "{ok, errors: [...], warnings: [...], output_duration, episodes: [{id, kept_seconds, cuts}]}",
        example: "supercut plans validate plan.json",
        keywords: &["check", "lint", "verify plan"],
    },
    Operation {
        path: "plans review",
        effects: &[ReadOnly],
        output: "{title, show, episodes: [{id, title, duration, kept, plays_at, interstitial_after, cuts: [{index, start, end, length, \
                 reason, fade_out, fade_in, context}], chapters}], totals}; --output text for Markdown",
        example: "supercut plans review plan.json --output text",
        keywords: &["summary", "approve", "human review"],
    },
    Operation {
        path: "plans compare",
        effects: &[ReadOnly],
        output: "{episodes: [{id, matched: [{a, b, start_delta, end_delta, shared_seconds}], only_in_a, only_in_b}], only_in_a, only_in_b, summary}; compares cuts of episodes both plans contain, so the plans must share episodes",
        example: "supercut plans compare draft.json history.plan.json",
        keywords: &["diff", "difference", "agree", "match past cuts", "evaluate draft", "versus"],
    },
    Operation {
        path: "plans preview",
        effects: &[WritesLibrary],
        output: "{preview, duration, transitions: [{index, preview_time, kind: opening|cut|episode_boundary|ending, episode, next, cut_start, cut_end, reason}]}; preview_time is where the splice falls in the reel",
        example: "supercut plans preview plan.json",
        keywords: &["listen", "transitions", "audition"],
    },
    Operation {
        path: "plans render",
        effects: &[WritesOutput, HeavyCompute],
        output: "{output, duration, bytes, bit_rate, chapters, sideload, dry_run}",
        example: "supercut plans render plan.json --dry-run",
        keywords: &["export", "build", "make supercut", "mp3", "encode"],
    },
    Operation {
        path: "history recover",
        effects: &[WritesLibrary],
        output: "{name, export, duration, episodes, pieces, gaps, interstitials, plan}",
        example: "supercut history recover the-neighborhood-listen ~/supercuts/nhl-S10-pt-1.mp3 --season 10",
        keywords: &["learn", "past edits", "logic", "align", "match export", "what did I cut"],
    },
    Operation {
        path: "history get",
        effects: &[ReadOnly],
        output: "{name, export, duration, episodes: [{id, title, kept_seconds, cut_seconds, transcribed, cuts: [{start, end, seconds, fade_out, fade_in, before, opens, closes, words, after}]}], interstitials}; the quote fields need the episode's transcript; --output text for Markdown",
        example: "supercut history get the-neighborhood-listen the-neighborhood-listen-S10-pt-1 --output text",
        keywords: &["report", "past cuts", "examples"],
    },
    Operation {
        path: "history list",
        effects: &[ReadOnly],
        output: "[{name, export, episodes, cuts}]",
        example: "supercut history list the-neighborhood-listen",
        keywords: &["recovered", "past supercuts"],
    },
];

pub fn find(path: &str) -> Option<&'static Operation> {
    OPERATIONS.iter().find(|operation| operation.path == path)
}
