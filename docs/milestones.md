# Rust Port Milestones

| Milestone | Delivered in | Verification |
| --- | --- | --- |
| M0 scaffold | `742a241` | fmt, clippy and 3 smoke tests passed |
| M1 engine/stats | `0492cd4` | Go golden vectors, property tests, fmt/clippy/test gate passed |
| M2 terminal UI/results/replay | `536554f`, `e4c9e68` | rendering, blind/zen, replay and history browser tests passed |
| M3 CLI/config/custom/history persistence | `9280b98`, `b4c2e52`, `fa7e5ba` | config, custom-text, result-file, storage and replay codec tests passed |
| M4 embedded modes and Rust corpus | `ce08116` | Rust registration/literal-character corpus tests passed |
| M5 themes/completion/doctor/update/uninstall | `df4e6f7`, `2910b46` | completion, doctor and verified-update tests passed |
| M6 docs and release automation | `d616e0a`, `796fb1a` | generated-man comparison and release artifact contract checked |

Final validation used the constrained one-job Cargo configuration in
`.cargo/config.toml`:

```text
cargo fmt --all --check                         PASS
cargo clippy --all-targets -j 1 -- -D warnings PASS
cargo test --all-targets -j 1                  PASS (45 tests)
```
