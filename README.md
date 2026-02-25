# profile

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![GitHub release](https://img.shields.io/github/v/release/cmdrvl/profile)](https://github.com/cmdrvl/profile/releases)

**Column scoping and structural metadata — defining which columns to include, key alignment, and normalization rules for report tools like `rvl`, `shape`, and `compare`.**

No AI. No inference. Pure deterministic configuration and validation.

```bash
brew install cmdrvl/tap/profile
```

</div>

---

## TL;DR

**The Problem**: Report tools like `shape` and `rvl` analyze every column by default. For a loan tape with 200 columns, you only care about 15. Teams maintain ad-hoc column lists in spreadsheets, pass fragile `--key` flags, and have no way to version, validate, or share column-scoping decisions.

**The Solution**: A profile is a versioned YAML file that declares which columns to include, what the key column is, and how to normalize values for comparison. Profile files are validated, frozen with a content hash, and consumed by all report tools via `--profile`.

### Why Use profile?

| Feature | What It Does |
|---------|--------------|
| **Column scoping** | Declare which columns matter — report tools only analyze `include_columns` |
| **Key declaration** | Specify the join/alignment key — no more guessing which column is the identifier |
| **Normalization rules** | Float precision, string trimming, order invariance — consistent across tools |
| **Versioned & frozen** | Each profile has a version and SHA-256 content hash — immutable once frozen |
| **Drafting workflow** | `profile draft init` reads a CSV header and generates a starting profile |
| **Validation** | `profile lint` catches schema drift between profile and dataset |
| **Tool-agnostic** | One profile consumed by `shape`, `rvl`, `compare`, and `lock` |

---

## Quick Example

```bash
# Step 1: Draft a profile from a dataset header
$ profile draft init loan_tape.csv --out loan_profile.yaml
```

```yaml
profile_id: "loan_tape.draft.v0"
profile_version: 0
include_columns:
  - loan_id
  - balance
  - rate
  - maturity_date
  - property_type
key: ["loan_id"]
equivalence:
  order: "order-invariant"
  float_decimals: 6
  trim_strings: true
```

```bash
# Step 2: Validate against the dataset
$ profile lint loan_profile.yaml --against loan_tape.csv
# exit 0 — all columns found, key is unique

# Step 3: Freeze the profile (immutable)
$ profile freeze loan_profile.yaml
# profile_sha256: "sha256:a1b2c3d4..."

# Step 4: Use with report tools
$ shape old.csv new.csv --profile loan_profile.yaml --json
$ rvl old.csv new.csv --profile loan_profile.yaml --json
```

One profile scopes all downstream analysis. Versioned, validated, frozen.

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
profile freeze loan_profile.yaml
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
| **No profile diffing** | Can't diff two profile versions — manual comparison only |
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

## Spec and Development

The profile specification is part of the [epistemic spine plan](https://github.com/cmdrvl/cmdrvl-context). This README covers intended behavior; implementation is in progress.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
