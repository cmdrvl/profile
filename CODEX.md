# CODEX.md — profile

Codex agents inherit [`AGENTS.md`](./AGENTS.md). Read it first, then check [`docs/PLAN.md`](./docs/PLAN.md) for behavior.

Quality gate:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

For focused command work, run the matching integration test first, for example `cargo test --test slice` or `cargo test doctor`.

Keep `profile slice` output safe for agents: default JSON and witness records must not include data rows; use `--out` or non-JSON stdout for clean CSV, and `--emit-manifest` only when captured preamble/unit rows are explicitly needed.

Before landing, sync Beads, run UBS on staged files, commit, push `main`, and push `origin main:master`.
