# Parity Audit

The Go tree at `/root/ttype` is the behavior oracle. The table names the
specific implementation and test that protects each compatible behavior.

| Feature | Go reference | Rust implementation | Test coverage |
| --- | --- | --- | --- |
| Net WPM, raw WPM, accuracy and two-decimal rounding | `internal/stats/stats.go:11-36,114-115` | `src/stats.rs` | `tests/stats_golden.rs` |
| Consistency mean/COV curve and clamp | `internal/stats/stats.go:49-84` | `src/stats.rs` | `tests/stats_golden.rs`, `tests/session_proptest.rs` |
| Character heatmap count/order/limit | `internal/stats/stats.go:91-112` | `src/stats.rs`, `src/tui.rs` | `tests/stats_golden.rs`, `tests/tui_render.rs` |
| One-second rating floor, 500 ms sample floor, five-second min-WPM grace, 20 extras | `internal/engine/session.go:18-29,450-464,618-674` | `src/session.rs` | `tests/session_golden.rs` |
| First accepted key starts time; word skips, extras, backspace and delete-word state | `internal/engine/session.go:371-447,466-604` | `src/session.rs` | `tests/session_golden.rs`, `tests/session_proptest.rs` |
| Code-mode tabs/newlines/spaces are literal characters | `internal/engine/session_code_test.go:1-156` | `src/domain.rs`, `src/session.rs` | `tests/session_golden.rs`, `tests/rust_mode.rs` |
| Deterministic seeded target generation | `internal/text/provider.go:90-151` | `src/text.rs` | `tests/text_generation.rs` |
| Cached language word lists and display names | `internal/text/langcache/cache.go:61-150,267-302` | `src/langcache.rs`, `src/app.rs`, `src/tui.rs` | `tests/language_cache.rs` |
| Built-in assets and new safe Rust corpus | `assets/embed.go:12-52` | `src/assets.rs`, `assets/rust/snippets.txt` | `tests/rust_mode.rs` |
| Custom pipe/file/text selection, 1 MiB cap and normalization | `internal/app/custom_text.go:16-116` | `src/custom_text.rs`, `src/app.rs` | `tests/custom_text_golden.rs` |
| Custom-text config precedence and whole-text clamp | `internal/app/custom_text.go:157-178` | `src/custom_text.rs` | `tests/custom_text_golden.rs` |
| Flat history JSON, newest-first list, 1,000-result retention, missed words and concurrent-save lock | `internal/storage/history.go:16-105,157-204`; `internal/storage/lock.go:1-29` | `src/storage.rs`, `src/lock.rs` | `tests/history_golden.rs` |
| Result JSON status, source hash, duration and timestamps | `internal/app/result_file.go:16-99` | `src/result_file.rs`, `src/session.rs` | `tests/result_file_golden.rs`, `tests/result_json_golden.rs` |
| Replay v1 binary sidecars and 50-file retention | `internal/storage/replay.go:17-198` | `src/storage.rs`, `src/replay.rs` | `tests/replay_storage.rs` |
| Replay fake clock, 50 ms tick, exact event offsets, pause/speed/restart | `internal/tui/replay_model.go:24-146` | `src/tui.rs` | `tests/tui_render.rs`, `tests/replay_storage.rs` |
| Results, heatmap, blind/zen, help, picker, settings and history/replay screens | `internal/tui/{test_model,result_render,history_model,keys}.go` | `src/tui.rs` | `tests/tui_render.rs` |
| Save the result and replay as the session finishes, before the result screen is dismissed | `internal/tui/app_model.go:466-528` | `src/tui.rs`, `src/session.rs` | `src/tui.rs` unit tests, `tests/session_golden.rs` |
| Braille chart interpolation, error markers and ASCII fallback | `internal/tui/chart.go:1-220`, `internal/tui/sparkline.go:1-47` | `src/tui.rs` | `tests/tui_render.rs` |
| TOML config and XDG data/config directories | `internal/storage/paths.go:1-98`, `internal/app/config.go:1-241` | `src/config.rs` | `tests/cli_flags.rs`, `tests/history_golden.rs` |
| `run`, config/history/stats/clear/doctor/update/completion/version/uninstall command surface | `cmd/ttype/main.go:40-465` | `src/cli.rs`, `src/app.rs` | `tests/cli_surface.rs`, `tests/cli_flags.rs`, `tests/stats_screen.rs` |
| Doctor terminal/colour/locale/clipboard/data checks | `internal/app/doctor.go:1-139` | `src/doctor.rs` | `tests/doctor_golden.rs` |
| Safe uninstall confirmation, managed-install protection and optional purge | `internal/app/uninstall.go:13-134` | `src/uninstall.rs` | `tests/uninstall_golden.rs` |
| Version comparison, daily state, automatic-mode precedence, checksum, archive verification, sibling atomic replacement and install-only exit wait | `internal/app/update.go:25-438`, `internal/app/autoupdate.go:16-143` | `src/update.rs`, `src/app.rs`, `src/tui.rs` | `tests/update.rs`, `src/tui.rs` unit tests |
| Derived Bash/Zsh/Fish/PowerShell completion and man page | `cmd/ttype/completion.go:1-49` | `src/completion.rs`, `src/bin/generate_artifacts.rs`, `man/ttype.1` | `tests/completion_golden.rs` |
| Static-musl archives, checksums and updater-compatible asset names | `.goreleaser.yaml:1-61` | `.github/workflows/release.yml`, `Makefile` | release workflow review |

## Formula and Threshold Sources

No score formula or threshold is inferred in this port. `src/stats.rs` ports
the five-characters-per-word WPM/raw-WPM equations, zero-input accuracy of
100, `round(v*100)/100`, and the consistency `tanh` curve from
`internal/stats/stats.go:11-36,49-84,114-115`. `src/session.rs` uses the
reference's 500 ms partial bucket floor, one-second rating floor, five-second
min-WPM grace and 20-extra limit from `internal/engine/session.go:18-29`; it
records buckets and samples as prescribed at `internal/engine/session.go:618-674`
and compares `int(WPM)` to the minimum at `internal/engine/session.go:450-464`.
Replay offsets are unsigned ULEB millisecond deltas exactly as specified in
`internal/storage/replay.go:108-198`, and its TUI playback uses the reference
50 ms frame interval from `internal/tui/replay_model.go:24-146`.

## Known Deviations

- The Rust UI deliberately preserves keyboard flows, state and metrics but is
  not pixel-identical to Bubble Tea/Lipgloss: it uses Ratatui layout and does
  not reproduce Go's big-digit renderer. The braille chart, including its
  thresholds and fallback, is now ported. Reference:
  `internal/tui/bigdigits.go:1-170`.
- Downloading Monkeytype language catalogs is intentionally unavailable. The
  user-imposed runtime-network rule permits only update traffic, whereas Go's
  `languages download` fetches GitHub resources at
  `internal/app/languages.go:14-72` and `internal/text/langcache/cache.go:151-232`.
  English, every shipped asset, and any already-cached compatible language
  lists remain available offline.
- Update downloads have the same 10-second check timeout, 64 MiB bound,
  15-minute total limit, SHA-256 verification and atomic swap. Ureq's receive
  timeout is a bounded body-read timeout rather than Go's reset-on-each-byte
  `progressReader` watchdog (`internal/app/update.go:252-300`), so a very slow
  but continuously-progressing transfer can time out earlier than Go.
