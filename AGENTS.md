# supercut

A Rust CLI that stitches podcast episodes into one long MP3 with chosen segments cut out. [README.md](README.md) covers what it does and how to use it. [skills/building-podcast-supercuts/SKILL.md](skills/building-podcast-supercuts/SKILL.md) is the skill for operating it to make a supercut; this file is for changing the code.

## Commands

```sh
cargo build
cargo run -- <resource> <operation> [args]
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

Before declaring work done, `cargo fmt --check`, the clippy command above and `cargo test` must all pass. There is no CI; these checks are the gate.

`rust-toolchain.toml` pins the compiler. A `RUSTUP_TOOLCHAIN` environment variable overrides it; `rustup show active-toolchain` tells you which one is in use.

Tests in `src/render/tags/tests.rs`, `src/analysis/mod.rs` and `src/analysis/jingle.rs` run `ffmpeg` and `ffprobe`. No test reaches the network or the Jev API.

## Real data

The default config (`supercut config get` prints its path) is the user's own, and its library holds their downloaded episodes, transcripts, labels, plans and recovered history. Treat it as production data.

- To try a command that writes, point it at a scratch config: `--config /tmp/scratch.jsonc` or `SUPERCUT_CONFIG`. `supercut config init --config /tmp/scratch.jsonc` writes one.
- `labels create` costs money. Run it only with `--dry-run` unless the user asks for the real thing.
- `transcripts create` and `plans render` take minutes per episode; `episodes sync` and `episodes download` use the network.
- Never pass `--force` against the user's library or output folder without asking. It replaces transcripts, labels, plans, recovered history or rendered files.
- The per-show rules files (`rules/<show>.md` next to the config) are the user's writing. Do not edit them unasked.

## Layout

| Path | Contents |
| --- | --- |
| `src/main.rs` | clap entry point, dispatch to resources, error reporting |
| `src/commands/` | One folder per resource, one file per operation; `registry.rs` describes every operation |
| `src/config.rs`, `src/config/` | `supercut.config.jsonc`: types, JSONC parsing, chapter layouts, transcription settings |
| `src/feed/` | RSS parsing, the episode index, downloading and adopting audio |
| `src/library/` | The per-show library folder layout and atomic file writes |
| `src/transcribe/`, `src/transcript.rs` | Whisper engines (mlx-whisper, faster-whisper) and the word-timed transcript |
| `src/labels/` | Jev labeling, and `plans draft`: openings, closings, phrases, cuts and chapters |
| `src/analysis/` | Loudness, level envelopes, and finding theme, outro and bumper recordings |
| `src/plan/` | The plan format, validation, lint, review, compare and the rendered timeline |
| `src/render/` | ffmpeg filter graphs, MP3 encoding, preview reels, ID3 tags and chapters |
| `src/history/` | Chromaprint matching of hand-made supercuts back to their episodes |
| `src/media/` | ffprobe and PCM decoding through ffmpeg |
| `src/timecode.rs` | `Time`, written `h:mm:ss.sss` |
| `src/error.rs` | `CliError` and its stable error codes |

## Conventions

### Operations

- The grammar is `supercut <resource> <operation>`. An operation lives in `src/commands/<resource>/<operation>.rs` as a clap `Args` struct and a `run(context, args)` function, and the resource's `mod.rs` dispatches to it.
- The doc comment on `Args` is the operation's help: its first paragraph is the summary and the rest the description, both shown by `supercut schema`. Doc comments on fields are the argument help.
- Every operation needs an entry in `src/commands/registry.rs` with its effects, output shape, example and search keywords. The `registry_and_cli_describe_the_same_operations` test fails when the registry and clap disagree.
- `--dry-run` exists on the operations that download, adopt, transcribe, label, clip, render or write the config. Give a new operation with comparable cost or reach one too.
- An operation that would replace a file the user made or may have edited (the config, a plan, a recovered history, a clip, a rendered supercut) refuses with `CONFLICT` unless given `--force`. `transcripts create` and `labels create` skip finished episodes instead, and `--force` redoes them.
- When a command, flag or output field changes, update the README and the skill to match.

### Output

- Results go to stdout through `output::print_json`. Progress goes to stderr. Commands whose result reads well as prose take `--output text` through `output::Format`.
- Failures are `CliError`s (`src/error.rs`) with a stable `ErrorCode`, a `hint` naming the next command to run, and `detail`s where useful. `main` finds the `CliError` anywhere in the `anyhow` context chain; an error without one is reported as `INTERNAL`. Agents branch on the codes, so a code's meaning and exit status do not change.
- Files are written with `library::write_atomically` or `write_json_atomically`: to `<name>.partial` beside the destination, then renamed into place.

### Config

- The config types in `src/config.rs` deny unknown fields, and their doc comments become the JSON Schema that `config schema` prints and `config init` writes.
- A new setting also goes into `src/commands/config/starter-config.jsonc` with a comment. `the_starter_config_parses` checks that the starter still parses.
- Only a segment's `label` and `describe` reach the Jev classifier. Changing either one means existing labels are stale; changing any other segment field does not.

### Plans

- Times in plans are positions in the episode's own audio file, not in the supercut.
- Plans are user-editable JSON. Keep them readable: optional fields stay absent when unset (`skip_serializing_if`), and `reason` says why each cut exists.

### Style

- `rustfmt.toml` sets 100 columns. Stable rustfmt sorts imports but cannot group them, so write `std`, external crates, then `crate` imports as blank-line-separated groups by hand.
- Clippy runs at its default lint level. Silence a lint with `#[expect(clippy::…, reason = "…")]` on the item, not `#[allow]`.
- Tests sit beside the code: an inline `#[cfg(test)] mod tests`, or a `tests.rs` file in the module's folder. Fixture files go in a `testdata/` folder next to the tests that read them.

## Commits

Scoped Commits: `<scope>: <imperative description>`, where the scope is the area changed: a module (`plans`, `labels`, `transcribe`, `feed`, `render`, `history`, `analysis`, `config`, `cli`), or `skills`, `backlog`, `docs`, `build`, `style` or `license`.

## Backlog

`BACKLOG.md` holds the agreed order of work. When you finish an item or find a new gap, record it there in its own `backlog:` commit.
