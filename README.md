[![CI](https://github.com/Poarthy/ttype-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/Poarthy/ttype-rs/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Poarthy/ttype-rs?color=blue)](https://github.com/Poarthy/ttype-rs/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20macos%20%7C%20windows-lightgrey)](#install)

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

## Install

### Prebuilt binaries

Every tag runs the release workflow, which builds static binaries for Linux
(amd64, arm64), macOS (amd64, arm64), and Windows (amd64). Grab one from the
[releases page](https://github.com/Poarthy/ttype-rs/releases/latest):

```sh
tar -xzf ttype_<version>_linux_amd64.tar.gz   # _arm64 / _darwin_* / _windows_* also published
./ttype run
```

Each archive contains the `ttype` binary, the `ttype.1` man page, shell
completions (bash, zsh, fish, PowerShell), `README.md`, and `LICENSE`.
`checksums.txt` in the same release lists the SHA-256 of every archive. On
Windows, unpack `ttype_<version>_windows_amd64.tar.gz` and put `ttype.exe`
somewhere on your `PATH`.

### From source

Needs Rust **1.98.1** or newer:

```sh
git clone https://github.com/Poarthy/ttype-rs.git
cd ttype-rs
cargo build --release        # or: make build
./target/release/ttype run
```

### Quick start

```sh
ttype run --time 30              # 30-second test
ttype run --words 25 --mode rust # 25-word test from the Rust snippet corpus
ttype history                    # interactive replay of past runs
ttype --help                     # full command surface
```

## Modes

`words`, `sentences`, `go`, `backend`, `python`, `sql`, `shell`, `regex`,
`rust`, and custom text are available. Code modes treat spaces, tabs, braces,
angle brackets, and newlines as literal practice characters. The Rust corpus
contains 49 safe-Rust snippets covering ownership, borrowing, lifetimes,
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

CI runs this gate on every push and pull request. Pushing a `v*` tag runs the
release workflow: one build per target, then a publish job that attaches the
archives and `checksums.txt` to a GitHub release.

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

## License

MIT — see [LICENSE](LICENSE). The original project
[alirezaudev/ttype](https://github.com/alirezaudev/ttype) is also MIT,
© Alireza Mousavi; its copyright notice is preserved in this repository, and
the Rust implementation is contributed under the same license.
