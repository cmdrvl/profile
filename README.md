# profile

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![GitHub release](https://img.shields.io/github/v/release/cmdrvl/profile)](https://github.com/cmdrvl/profile/releases)

**Stop passing column lists on the command line. Freeze your domain knowledge into a versioned, validated, tamper-evident config.**

```bash
brew install cmdrvl/tap/profile
```

</div>

---

Your dataset has 42 columns. 15 of them matter for this analysis. The key is `loan_id`. Float precision is 6 decimal places. Order doesn't matter. Where does this knowledge live? In a Slack thread? In someone's head? In a `--exclude` flag you'll forget next month?

**profile captures all of it in a versioned YAML file that every report tool consumes.** Draft one from a real CSV header, iterate until it's right, then freeze it — immutable, SHA-256 hashed, recorded in every lockfile and report that uses it. Change a column? New version. Full audit trail.

### What makes this different

- **Draft → freeze lifecycle** — `profile draft init` reads a CSV header and generates a starting profile. Edit it. Lint it against real data. When it's right, `profile freeze` makes it immutable and content-addressed.
- **Key intelligence** — `profile suggest-key` ranks candidate key columns by uniqueness, null rate, and type. No guessing.
- **One file, all tools** — the same frozen profile is consumed by `shape`, `rvl`, `compare`, and `lock`. Declare your scoping once.
- **Schema drift detection** — `profile lint --against data.csv` catches columns that disappeared, keys that aren't unique, and types that shifted.

---

## 5-Minute Quickstart (Copy/Paste)

```bash
# 1) Create a draft profile from a real dataset
profile draft init loan_tape.csv --out loan_tape.draft.yaml

# 2) Validate schema and lint against the dataset
profile validate loan_tape.draft.yaml
profile lint loan_tape.draft.yaml --against loan_tape.csv

# 3) Freeze to an immutable profile
profile freeze loan_tape.draft.yaml \
  --family csv.loan_tape.core \
  --version 0 \
  --out profiles/csv.loan_tape.core.v0.yaml

# 4) Use the frozen profile in report tools
shape old.csv new.csv --profile profiles/csv.loan_tape.core.v0.yaml --json
rvl old.csv new.csv --profile profiles/csv.loan_tape.core.v0.yaml --json
compare old.csv new.csv --profile profiles/csv.loan_tape.core.v0.yaml --json
```

---

## Draft → Frozen Workflow (With Expected Output)

```bash
profile draft init loan_tape.csv --out loan_tape.draft.yaml
# writes: loan_tape.draft.yaml

profile lint loan_tape.draft.yaml --against loan_tape.csv
# exit 0 (or exit 1 with deterministic lint issues)

profile freeze loan_tape.draft.yaml \
  --family csv.loan_tape.core \
  --version 0 \
  --out profiles/csv.loan_tape.core.v0.yaml
# writes: profiles/csv.loan_tape.core.v0.yaml
# frozen profile includes: profile_id, profile_family, profile_version, profile_sha256
```

A draft is cheap to iterate. A frozen profile is immutable and hashable for reproducible downstream analysis.

---

## Where profile Fits

`profile` is a **metadata tool** that configures how report tools operate.

```
                        ┌── shape ──┐
vacuum → hash → lock    │           │
                        ├── rvl ────┤ ← --profile
                        │           │
                        └── compare ┘
         profile ───────────────────┘
```

Profile doesn't sit in the stream pipeline (vacuum → hash → lock). Instead, it produces configuration files that report tools consume via `--profile`. Lock records which profiles were active in its `profiles` array.

---

## What profile Is Not

| If you need... | Use |
|----------------|-----|
| Enumerate files in a directory | [`vacuum`](https://github.com/cmdrvl/vacuum) |
| Compute content hashes | [`hash`](https://github.com/cmdrvl/hash) |
| Match files against templates | [`fingerprint`](https://github.com/cmdrvl/fingerprint) |
| Pin artifacts into a lockfile | [`lock`](https://github.com/cmdrvl/lock) |
| Check structural comparability | [`shape`](https://github.com/cmdrvl/shape) |
| Explain numeric changes | [`rvl`](https://github.com/cmdrvl/rvl) |

`profile` only answers: **which columns matter, what's the key, and how should values be compared?**

---

## Profile Format

A profile is a YAML file with a defined schema:

```yaml
profile_id: "csv.loan_tape.core.v0"
profile_version: 0
include_columns:
  - loan_id
  - current_balance
  - note_rate
  - maturity_date
  - property_type
  - occupancy
key: ["loan_id"]
equivalence:
  order: "order-invariant"
  float_decimals: 6
  trim_strings: true
```

| Field | Type | Description |
|-------|------|-------------|
| `profile_id` | string | Unique identifier with version suffix |
| `profile_version` | integer | Monotonically increasing version number |
| `include_columns` | string[] | Columns to include in analysis (others ignored) |
| `key` | string[] | Column(s) used for row alignment/joining |
| `equivalence.order` | string | `"order-invariant"` or `"order-sensitive"` |
| `equivalence.float_decimals` | integer | Decimal places for float comparison |
| `equivalence.trim_strings` | boolean | Trim whitespace before string comparison |

### Frozen Profiles

Once frozen, a profile is immutable:

```yaml
profile_id: "csv.loan_tape.core.v0"
profile_version: 0
profile_sha256: "sha256:a1b2c3d4e5f6..."
frozen: true
# ... rest of profile
```

Any semantic change requires a new `profile_version` and a new `profile_id`.

---

## Subcommands

### `profile draft init`

Generate a draft profile from a CSV header:

```bash
profile draft init loan_tape.csv --out loan_profile.yaml
```

Auto-populates `include_columns` from the header. You edit the draft to remove unwanted columns and set the key.

### `profile suggest-key`

Rank candidate key columns by uniqueness, null rate, and deterministic order:

```bash
profile suggest-key loan_tape.csv
# loan_id: unique=100%, nulls=0%, type=string ← recommended
# property_id: unique=85%, nulls=0%, type=string
```

### `profile lint`

Validate a profile against a dataset:

```bash
profile lint loan_profile.yaml --against loan_tape.csv
```

Catches: missing columns, non-unique keys, type mismatches, schema drift.

### `profile stats`

Surface structural statistics about a dataset:

```bash
profile stats loan_tape.csv
# rows: 1,247 | columns: 42 | nulls: 3.2% | key candidates: loan_id, property_id
```

### `profile freeze`

Validate and mark a profile immutable with SHA-256 content hash:

```bash
profile freeze loan_profile.yaml \
  --family csv.loan_tape.core \
  --version 0 \
  --out profiles/csv.loan_tape.core.v0.yaml
```

---

## How profile Compares

| Capability | profile | Manual column lists | Config files | SQL views |
|------------|---------|--------------------|--------------|-----------|
| Versioned and frozen | Yes | No | No | No |
| Content hash (tamper-evident) | Yes | No | No | No |
| Validated against dataset | Yes (`lint`) | No | No | At query time |
| Key declaration | Yes | Ad-hoc | Ad-hoc | Yes |
| Normalization rules | Yes | No | No | No |
| Cross-tool (shape/rvl/compare) | Yes | No | No | No |
| Draft from header | Yes (`draft init`) | Manual | Manual | Manual |

---

## Installation

### Homebrew (Recommended)

```bash
brew install cmdrvl/tap/profile
```

### Shell Script

```bash
curl -fsSL https://raw.githubusercontent.com/cmdrvl/profile/main/scripts/install.sh | bash
```

### From Source

```bash
cargo build --release
./target/release/profile --help
```

---

## Integration with Report Tools

All report tools accept `--profile`:

```bash
# shape — only check overlap on profile columns
shape old.csv new.csv --profile loan_profile.yaml --json

# rvl — only explain changes in profile columns
rvl old.csv new.csv --profile loan_profile.yaml --json

# compare — only diff profile columns
compare old.csv new.csv --profile loan_profile.yaml --json
```

### Lock Integration

`lock` records which profiles were active:

```json
{
  "profiles": [
    {
      "profile_id": "csv.loan_tape.core.v0",
      "profile_version": 0,
      "profile_sha256": "sha256:a1b2c3d4..."
    }
  ]
}
```

---

## Operational Contract

### Global flags

| Flag | Behavior |
|------|----------|
| `--describe` | Print `operator.json` and exit `0` before normal input validation |
| `--schema` | Print profile JSON Schema and exit `0` before normal input validation (deferred in v0.1) |
| `--version` | Print `profile <semver>` and exit `0` |
| `--no-witness` | Suppress witness ledger recording |

### Exit codes

| Exit | Meaning | When |
|------|---------|------|
| `0` | `SUCCESS` | Operation completed with no issues |
| `1` | `ISSUES_FOUND` | Lint/diff found issues or differences |
| `2` | `REFUSAL` | Invalid input, schema violation, parse/IO refusal, or CLI error |

### Refusal codes

`E_INVALID_SCHEMA`, `E_MISSING_FIELD`, `E_BAD_VERSION`, `E_ALREADY_FROZEN`, `E_IO`, `E_CSV_PARSE`, `E_EMPTY`, `E_COLUMN_NOT_FOUND`

With `--json`, refusals are emitted in the unified output envelope (`outcome=REFUSAL`, refusal detail in `result`). Without `--json`, refusals are human-readable errors on stderr with the refusal code.

### Witness behavior

- Witness append is enabled for: `freeze`, `validate`, `lint`, `stats`, `suggest-key`
- Witness append is skipped for: `draft new`, `draft init`, `list`, `show`, `diff`, `push`, `pull`
- `--no-witness` disables witness writes without changing domain outcome or exit semantics
- Ledger path: `~/.epistemic/witness.jsonl`
- Witness append failures warn on stderr and do not change primary command outcome/exit code

---

## Troubleshooting

### "Column not found" when using profile with shape/rvl

A column in `include_columns` doesn't exist in the dataset. Run `profile lint` to diagnose:

```bash
profile lint loan_profile.yaml --against new_tape.csv
# ERROR: Column 'occupancy' in profile not found in dataset
```

### Key column isn't unique

The column(s) declared in `key` have duplicate values. Use `profile suggest-key` to find better candidates:

```bash
profile suggest-key loan_tape.csv
```

### Profile version confusion

Frozen profiles are immutable. If you need to change columns, create a new version:

```yaml
# Old: csv.loan_tape.core.v0 (frozen)
# New: csv.loan_tape.core.v1 (add new columns, re-freeze)
profile_id: "csv.loan_tape.core.v1"
profile_version: 1
```

### float_decimals too aggressive

If `rvl` reports spurious changes, your `float_decimals` may be too high. Try reducing precision:

```yaml
equivalence:
  float_decimals: 2  # was 6, reduced to match business precision
```

### Profile works locally but fails in CI

Ensure the profile file is committed and the path is correct. Profiles are plain YAML — no environment dependencies.

---

## Limitations

| Limitation | Detail |
|------------|--------|
| **CSV only** | v0 profiles scope CSV/TSV columns; XLSX sheet/range scoping is deferred |
| **Single key type** | Composite keys supported, but only column-based — no expression keys |
| **No auto-update** | Profile doesn't auto-detect schema changes — use `lint` to catch drift |
| **No profile registry** | Profiles are local files — centralized registry is deferred |
| **Network publish deferred** | `push`/`pull` data-fabric wrappers are deferred in v0.1 |
| **Pre-release** | Implementation in progress — spec is complete in the epistemic spine plan |

---

## FAQ

### Why not just pass column names as CLI flags?

Flags don't compose. With 15 columns, a key, and normalization rules, the command line becomes unmanageable. A profile captures all scoping decisions in a versioned, validated, shareable file.

### Why freeze profiles?

Immutability. Once a profile is frozen and referenced by a lockfile, you can prove that the exact same column scoping was used. Any change requires a new version, creating an audit trail.

### Can I use the same profile across datasets?

Yes — as long as the datasets have the same schema. Use `profile lint --against` to verify compatibility before use.

### What if my dataset has columns not in the profile?

They're ignored. Report tools only analyze columns in `include_columns`. This is the whole point — focus on what matters.

### How does profile relate to fingerprint?

Fingerprint identifies *what kind* of file something is (template recognition). Profile declares *which columns* to analyze in report tools. They solve different problems and can be used together.

### Why is key declaration important?

Without a key, report tools like `rvl` can't align rows between two datasets. The key column(s) define how rows map from old to new. `profile suggest-key` helps identify the best candidate.

### Can profiles be generated programmatically?

Yes. `profile draft init` generates a starting profile from a CSV header. You can also write YAML directly or generate it from any tool.

---

## Agent Integration

For the full toolchain guide, see the [Agent Operator Guide](https://github.com/cmdrvl/.github/blob/main/profile/AGENT_PROMPT.md). Run `profile --describe` for this tool's machine-readable contract.

---

## Spec and Development

The profile specification is part of the [epistemic spine plan](https://github.com/cmdrvl/cmdrvl-context). This README covers intended behavior; implementation is in progress.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
