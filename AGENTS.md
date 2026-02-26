# AGENTS.md — profile

> Guidelines for AI coding agents working in this Rust codebase.

---

## profile — What This Project Does

`profile` creates, validates, and freezes column-scoping profiles that tell report tools which columns to analyze. It is a **configuration authoring tool** whose output (profile YAML files) is consumed by downstream report tools:

```
              profile
                │
                ▼
        profile YAML files
        ┌───────┼───────┐
        ▼       ▼       ▼
      shape   compare   rvl
```

### Quick Reference

```bash
# Create a draft profile from a real dataset
profile draft init tape.csv --out loan_tape.draft.yaml

# Validate and lint against a dataset
profile validate loan_tape.draft.yaml
profile lint loan_tape.draft.yaml --against tape.csv

# Freeze when ready (immutable, hashable)
profile freeze loan_tape.draft.yaml \
  --family csv.loan_tape.core --version 0 \
  --out profiles/csv.loan_tape.core.v0.yaml

# Quality gate
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

### Source of Truth

- **Spec:** [`docs/PLAN.md`](./docs/PLAN.md) — all behavior must follow this document
- Do not invent behavior not present in the plan

### Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entry + exit code mapping |
| `src/lib.rs` | Full dispatch (calls handler + output layer) |
| `src/cli/` | Argument parsing (clap derive), exit codes |
| `src/schema/` | Profile struct, schema validation, canonicalization |
| `src/draft/` | `draft new` (blank template) and `draft init` (from CSV) |
| `src/freeze/` | Draft → frozen transition, SHA256 |
| `src/lint/` | Profile vs dataset column checking |
| `src/stats/` | Column stats + suggest-key ranking |
| `src/resolve/` | Profile resolution, list, show |
| `src/diff/` | Structural profile diff |
| `src/output/` | Unified output envelope (JSON) + human formatting |
| `src/refusal/` | Refusal codes, detail payloads |
| `src/witness/` | Witness record append, ledger, query subcommands |
| `operator.json` | Machine-readable operator contract |

---

## Output Contract (Critical)

`profile` is a **mixed-mode tool** — some subcommands produce artifacts (YAML files), others produce reports.

- **`--json`** wraps all output in the unified envelope: `{ version, outcome, exit_code, subcommand, result, profile_ref, witness_id }`
- Without `--json`, output is free-form and human-optimized
- Refusals emit structured error details, not ad-hoc text

| Exit | Meaning | When |
|------|---------|------|
| `0` | `SUCCESS` | Operation completed, no issues |
| `1` | `ISSUES_FOUND` | `lint` found issues, `diff` found differences |
| `2` | `REFUSAL` | Invalid input, schema violation, CLI error |

### Decoupled Output Architecture

Subcommand handlers return `Result<serde_json::Value, RefusalPayload>` — they do NOT format output. The dispatch in `lib.rs` passes the result to the output layer (`output::json` for `--json`, `output::human` otherwise).

**This means:** subcommand beads only implement business logic in their own `.rs` files. No subcommand bead touches `lib.rs`, any `mod.rs`, or any file in `src/output/`.

---

## Core Invariants (Do Not Break)

### 1. Canonical form determinism

Frozen profile SHA256 must be computed from a deterministic canonical form:
1. Field order: `schema_version`, `profile_id`, `profile_version`, `profile_family`, `status`, `format`, `hashing`, `equivalence`, `key`, `include_columns`
2. Block-style YAML only (no flow sequences/mappings)
3. Exactly one trailing `\n`, no comments, no blank lines, no document markers
4. `profile_sha256` excluded from the canonical form (it's computed from it)

Any change to canonical field order, YAML style, or byte serialization is a **breaking change**.

### 2. Draft → Frozen lifecycle

- Drafts are exploratory and mutable. Frozen profiles are immutable.
- `freeze` rejects empty `include_columns` and already-frozen profiles.
- Any change after freeze = new version, no exceptions.

### 3. Refusal codes

All 8 refusal codes must match the plan contract: `E_INVALID_SCHEMA`, `E_MISSING_FIELD`, `E_BAD_VERSION`, `E_ALREADY_FROZEN`, `E_IO`, `E_CSV_PARSE`, `E_EMPTY`, `E_COLUMN_NOT_FOUND`.

### 4. Witness parity

Ambient witness semantics must match spine conventions:
- Append for `freeze`, `validate`, `lint`, `stats`, `suggest-key` only
- `--no-witness` opt-out
- Witness failures do not mutate domain outcome or exit code

### 5. Family name format

Must match `^[a-z][a-z0-9]*(\.[a-z][a-z0-9_]*)*$` — lowercase dot-separated segments.

---

## Toolchain

- **Language:** Rust, Cargo only
- **Edition:** 2024 (or `rust-toolchain.toml` when present)
- **Unsafe code:** forbidden (`#![forbid(unsafe_code)]`)
- **Dependencies:** explicit versions, small and pinned

Release profile:

```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

---

## Quality Gate

Run after any substantive change:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### Test Coverage Areas

- Draft new/init template generation and header-driven drafting
- Validate/lint schema and column checking paths
- Stats/suggest-key deterministic output and ranking
- Freeze canonicalization, default-fill, SHA256 round-trip
- List/show resolution from `~/.epistemic/profiles/`
- Diff semantic-fields-only comparison
- Refusal envelope shape and detail payloads for all 8 codes
- Witness append/no-witness/query behavior
- Output envelope: every `--json` subcommand returns a valid envelope
- Golden file tests: known CSV → exact expected draft

---

## Git and Release

- **Primary branch:** `main`
- **`master`** exists for legacy URL compatibility — keep synced: `git push origin main:master`
- Bump `Cargo.toml` semver appropriately on release
- Sync `Cargo.lock` before release workflows that use `--locked`

---

## RULE 0

If the user gives a direct instruction, follow it even if it conflicts with defaults in this file.

---

## RULE 1: No File Deletion

**You are never allowed to delete a file without express permission.** Always ask and receive clear, written permission before deleting any file or folder.

---

## Editing Rules

- **No destructive git commands** (`reset --hard`, `clean -fd`, `rm -rf`, force push) without explicit authorization
- **No scripted mass edits** — make intentional, reviewable changes
- **No file proliferation** — edit existing files; new files for genuinely new functionality only
- **No surprise behavior** — do not invent behavior not in `docs/PLAN.md`
- **No backwards-compatibility shims** — fix the code directly

---

## Beads (`br`) Workflow

Use Beads as source of truth for task state. Issues are stored in `.beads/` and tracked in git.

**Important:** `br` is non-invasive — it NEVER executes git commands. After `br sync --flush-only`, you must manually run `git add .beads/ && git commit`.

```bash
br ready              # Show unblocked ready work
br list --status=open # All open issues
br show <id>          # Full issue details
br update <id> --status=in_progress
br close <id> --reason "Completed"
br sync --flush-only  # Export to JSONL (no git ops)
```

Pick unblocked beads. Mark in-progress before coding. Close with evidence when done.

### Phase Labels

Beads are labeled `phase-0` through `phase-3` indicating when they can start:

| Phase | When | What |
|-------|------|------|
| `phase-0` | Immediately | Scaffold, schema, refusal, fixtures, CI, docs |
| `phase-1` | After Phase 0 completes | All subcommands + output envelope (parallel) |
| `phase-2` | After specific Phase 1 beads | Dependent subcommands + first test suites |
| `phase-3` | After Phase 2 | Remaining test suites |

Use `br list --label phase-N` to see beads in each phase.

---

## Agent Mail (Multi-Agent Sessions)

When Agent Mail is available:

- Register identity in this project
- **Reserve only the specific file(s) you are editing — never entire directories or broad globs**
- Each bead's comments document the exact files to reserve (look for `RESERVATIONS:`)
- Send start/finish updates per bead using bead ID as `thread_id`
- Poll inbox at moderate cadence (2-5 minutes)
- Acknowledge `ack_required` messages promptly
- Release reservations when done

### File Reservation Rules

The scaffold (bd-qzc) pre-creates all `mod.rs` files and placeholder stubs. This means:

1. **No bead except scaffold touches `lib.rs` or any `mod.rs`** — your dispatch and re-exports are already in place
2. **You only edit your own `.rs` file(s)** — the stubs have `todo!()` that you replace with real implementation
3. **Reserve only the files you are writing** — not the module directory, not mod.rs

Example: if working on `stats` (bd-1x2), reserve only `src/stats/stats.rs`.

---

## Multi-Agent Coordination

When working alongside other agents:

- **Never stash, revert, or overwrite other agents' work**
- Treat unexpected changes in the working tree as if you made them
- If you see changes you didn't make in `git status`, those are from other agents working concurrently — commit them together with your changes
- This is normal and happens frequently in multi-agent environments

**Do NOT** stop working to ask about unexpected changes. **Do** continue working as normal and include those changes when you commit.

---

## Session Completion

Before ending a session:

1. Run quality gate (`fmt` + `clippy` + `test`)
2. Confirm docs/spec alignment for behavior changes
3. Update bead status (`br close <id>` or update progress)
4. Sync beads: `br sync --flush-only`
5. Commit with precise message:
   ```bash
   git add .beads/ <other files>
   git commit -m "..."
   git push
   ```
6. Verify: `git status` shows "up to date with origin"
7. Summarize: what changed, what was validated, remaining risks

**Work is NOT complete until `git push` succeeds.** Never stop before pushing.
