# ttype-rs

`ttype` is a terminal typing trainer with prose and code modes, replay, live
WPM charts, consistency scores, and character-error heatmaps.

This project is a **Rust rewrite** of
[alirezaudev/ttype](https://github.com/alirezaudev/ttype) — the original Go
implementation of the same tool. The goal is behavior parity with upstream,
reimplemented from scratch in safe Rust; the upstream repository and its
authors are unmodified and unaffiliated with this port.

The final application uses a single-threaded `ratatui` and `crossterm` event
loop, stores user configuration as TOML, and embeds shipped practice lists so
normal runs work offline. Network access is limited to the optional update
check.

## Modes

`words`, `sentences`, `go`, `backend`, `python`, `sql`, `shell`, `regex`,
`rust`, and custom text are available. Code modes treat spaces, tabs, braces,
angle brackets, and newlines as literal practice characters. The Rust corpus
contains 55 safe-Rust snippets covering ownership, borrowing, lifetimes,
matching, `Option`/`Result`, `?`, traits, generics, iterators, structs, enums,
modules, async/await, derives, errors, and Cargo practice.

## Data and Privacy

Settings use `$XDG_CONFIG_HOME/ttype/config.toml` (or the platform config
directory); history, replays, language data, and update state use
`$XDG_DATA_HOME/ttype`. The app sends no telemetry. Built-in practice text is
compiled into the executable. Only `ttype update` can initiate a network call.
`ttype languages` lists English plus compatible cached lists already present in
`$XDG_DATA_HOME/ttype/languages`; `languages download` remains unavailable so
normal practice never fetches a catalog.

## Development Gate

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

The release workflow produces static musl artifacts, checksums, and includes
the generated command surface and `man/ttype.1`.

## Commands

`ttype run`, `config`, `history`, `stats`, `languages`, `clear`, `doctor`,
`update`, `completion`, `version`, and `uninstall` are available. `history`
opens an interactive selector on a terminal; press Enter to replay a retained
run. Replay sidecars use the Go-compatible `TTRP` v1 format and retain the
newest 50 files. `ttype update` is the only network-enabled operation: it
checks the GitHub release redirect daily, verifies SHA-256 checksums, runs the
downloaded binary with `--version`, and atomically swaps it only when the
installed copy is an owned release build. Interactive runs honor `update =
"auto"`, `"notify"`, or `"off"` (and `TTYPE_UPDATE` overrides it); JSON and
CI runs remain quiet and offline.
