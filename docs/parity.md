# Parity Audit

| Feature | Go reference | Rust implementation | Coverage |
| --- | --- | --- | --- |
| Net/raw WPM and accuracy | `internal/stats/stats.go:11` | `src/stats.rs` | `tests/stats_golden.rs` |
| Consistency curve | `internal/stats/stats.go:49` | `src/stats.rs` | `tests/stats_golden.rs`, property test |
| Heatmap sort | `internal/stats/stats.go:91` | `src/stats.rs` | `tests/stats_golden.rs` |
| Timing floors and WPM threshold | `internal/engine/session.go:18` | `src/session.rs` | `tests/session_golden.rs` |
| Word skip/extras/backspace | `internal/engine/session.go:371` | `src/session.rs` | session golden/property tests |
| Code literal whitespace | `internal/engine/session.go:379` | `src/session.rs` | `code_mode_counts_tabs_and_newlines_as_literal_practice_characters` |
| Per-second samples | `internal/engine/session.go:618` | `src/session.rs` | session golden tests |
| Replay event model | `internal/domain/replay.go:5` | `src/replay.rs` | replay golden test |
| Replay file codec | `internal/storage/replay.go:17` | `src/storage.rs` | `tests/replay_storage.rs` |
| Embedded practice modes | `assets/embed.go:12` | `src/assets.rs` | `tests/rust_mode.rs` |
| Rust mode | new feature | `assets/rust/snippets.txt`, domain/CLI | `tests/rust_mode.rs` |
| Config paths/defaults | `internal/storage/paths.go:1` | `src/config.rs` | CLI/config behavior |
| Shell completion | `cmd/ttype/completion.go` | `src/app.rs` | Clap generation path |
| Man page/release packaging | `Makefile:42` | `man/ttype.1`, release workflow | CI artifact definition |
| Update checksum/atomic swap | `internal/app/update.go:1` | `src/update.rs` | `tests/update.rs` |

## Known Deviations

- The Ratatui interface has the Go key paths and main test/results/help/mode/settings states, but does not reproduce Bubble Tea's pixel-identical layouts, chart glyph rendering, language picker, clipboard integration, or interactive history table. Go references: `internal/tui/*.go`.
- `history` is currently a plain table; selecting an entry to launch a replay is not wired. Go reference: `internal/tui/history_model.go`.
- Runtime update checking records its daily check but has no configured public release endpoint or tarball extractor in this source build. Checksums and atomic replacement are implemented and tested. Go reference: `internal/app/update.go`, `internal/app/autoupdate.go`.
- Downloadable language lists are deliberately not fetched in this offline source build. Go reference: `internal/text/langcache/cache.go`.
- Go JSON persistence schemas are not read as a migration format; Rust stores TOML configuration and its own compact JSON history wire format. Go references: `internal/storage/history.go`, `internal/storage/json_store.go`.
