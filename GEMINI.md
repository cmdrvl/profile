# GEMINI.md — profile

This harness follows [`AGENTS.md`](./AGENTS.md).

Use these commands for verification:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
br ready --json
br sync --flush-only
```

When reviewing slice/pre_parse work, check both behavior and disclosure boundaries: clean CSV may contain data by design, but JSON envelopes and witness records should contain only metadata unless `--explicit` is set.
