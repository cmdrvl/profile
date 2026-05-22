# profile

![profile: freeze operator knowledge. A painterly dashboard showing Connections.csv with its four-row preamble dimmed and the header at row 4 highlighted. A profile config draft in the middle holds fields like header_at, key_column, trim_whitespace, decimals, slug. A snowflake FREEZE action produces a profile.lock file with sha256:9a2c…f1d8. Outputs include a clean CSV table and a slicing manifest. A profile version history table shows v1 draft and v2 frozen.](docs/images/profile.webp)

> *What you know about the file becomes a hash. The freeze is the moment of trust.*

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

**profile captures all of it in a versioned YAML file that downstream report tools can consume.** Draft one from a real CSV header, iterate until it's right, then freeze it — immutable, SHA-256 hashed, recorded in every lockfile and report that uses it. Change a column? New version. Full audit trail.

### What makes this different

- **Draft → freeze lifecycle** — `profile draft init` reads a CSV header and generates a starting profile. Edit it. Lint it against real data. When it's right, `profile freeze` makes it immutable and content-addressed.
- **Key intelligence** — `profile suggest-key` ranks candidate key columns by uniqueness, null rate, and type. No guessing.
- **Registry-backed header canonicalization** — optional `column_registry` lets the same profile survive heterogeneous raw headers by resolving them to canonical column IDs before scoping.
- **Witnessed pre-parse slicing** — optional `pre_parse` directives describe how to remove export preambles, merge multi-row headers, and capture units rows before downstream tools read the CSV.
- **One file, reusable scoping** — `rvl` consumes frozen profiles today, and the same artifact is the intended scoping surface for `shape`, `compare`, and `lock` as those integrations settle. Declare your domain choices once.
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

# 4) Use the frozen profile with the current live downstream surface
rvl old.csv new.csv --profile profiles/csv.loan_tape.core.v0.yaml --json
```

`rvl` is profile-aware today. Check `shape` / `compare` operator contracts before assuming equivalent `--profile` behavior in the current release line.

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
vacuum → hashbytes → lock
           ↑
        profile → rvl
        profile → shape / compare (planned or release-line dependent)
```

Profile doesn't sit in the stream pipeline (vacuum → hashbytes → lock). Instead, it produces configuration files that downstream tools can consume where the current runtime contract supports `--profile`. `rvl` is the live consumer today; other integrations are converging by tool/release line. Lock records which profiles were active in its `profiles` array.

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

It can also perform a narrow lineage-preserving CSV slice when a profile declares `pre_parse` directives. That slice is not a general transform pipeline: it only removes pre-data rows, normalizes headers, optionally captures units/preamble metadata in a manifest, and emits clean CSV for the existing profile workflow.

---

## Profile Format

A profile is a YAML file with a defined schema:

```yaml
profile_id: "csv.loan_tape.core.v0"
profile_version: 0
column_registry: "registries/annex-columns-v0"
pre_parse:
  slice:
    mode: preamble_skip
    skip_rows: 3
    header_at_row: 4
    data_starts_at: 5
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
| `column_registry` | string | Optional canon registry path used to normalize raw headers to canonical column IDs before scoping |
| `fingerprint_ref` | string | Optional upstream fingerprint ID used as pre-parse lineage |
| `pre_parse` | object | Optional CSV slicing directives (`preamble_skip`, `multi_row_header`, `preamble_with_units`) |
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
When headers vary across providers, add `--column-registry registries/annex-columns-v0` to write canonical column IDs into the draft instead of raw header spellings.
For messy exports, seed pre-parse directives from `fingerprint peek --suggest`:

```bash
fingerprint peek vendor_export.csv --json --suggest > peek.json
profile draft init vendor_export.csv --from-peek peek.json --out vendor_profile.yaml
```

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

### `profile slice`

Apply profile-driven or ad-hoc pre-parse directives to emit clean CSV:

```bash
profile slice vendor_export.csv --profile-path vendor_profile.yaml --out clean.csv --emit-manifest slice.manifest.json
profile slice vendor_export.csv --mode multi_row_header --header-rows 2,3 --data-starts-at 4
```

Without `--out` in human mode, `slice` writes the clean CSV to stdout. With `--json`, it emits the `profile.v0` envelope with row counts, columns, output hash, and lineage metadata; raw data rows are omitted unless `--explicit` is set. The optional manifest is explicit opt-in and may contain captured preamble/unit rows. When a profile is provided and slice flags override profile directives, `slice` emits explicit warnings.

### `profile emit-discovery`

Emit a deterministic `profile.discovery.v0` candidate template from an already-sliced CSV and a chosen preamble skip offset:

```bash
profile emit-discovery linkedin_sliced.csv \
  --source-file Connections.csv \
  --skip-rows 3 \
  --source-kind linkedin_export --json
```

The result payload is designed for downstream `fingerprint template promote` without re-reading the source file.

### `profile stats`

Surface structural statistics about a dataset. Per-column example values are redacted by
default; JSON output only includes `example` fields when the global `--explicit` flag is
set:

```bash
profile stats loan_tape.csv
# rows: 1,247 | columns: 42 | nulls: 3.2% | key candidates: loan_id, property_id

profile --explicit --json stats loan_tape.csv
# includes per-column example values
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

Current runtime support:

- `rvl` accepts `--profile` today for key derivation and column scoping
- `shape` exposes `--profile` / `--profile-id` flags, but its current operator contract still marks the check-scoping behavior as reserved/deferred
- `compare` remains the deferred exhaustive diff tool in the broader spine roadmap

```bash
# rvl — only explain changes in profile columns
rvl old.csv new.csv --profile loan_profile.yaml --json
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
| `--schema` | Print JSON Schema and exit `0` before normal input validation (`profile --schema` prints profile YAML schema; `profile emit-discovery --schema` prints `profile.discovery.v0` schema) |
| `--version` | Print `profile <semver>` and exit `0` |
| `--no-witness` | Suppress witness ledger recording |

### Exit codes

| Exit | Meaning | When |
|------|---------|------|
| `0` | `SUCCESS` | Operation completed with no issues |
| `1` | `ISSUES_FOUND` | Lint/diff found issues or differences |
| `2` | `REFUSAL` | Invalid input, schema violation, parse/IO refusal, or CLI error |

### Doctor

`profile doctor` is a read-only diagnostic surface for headless agents. It does
not read profile files, datasets, column registries, stdin, witness ledgers, or
network endpoints. It does not write profile YAML, witness records, `.doctor/`
artifacts, or remote data.

```bash
profile doctor health --json
profile doctor capabilities --json
profile doctor --robot-triage
profile doctor robot-docs
```

`doctor --json` responses use the same `profile.v0` output envelope as the rest
of the CLI. There is no `doctor --fix` mode.

### Refusal codes

`E_INVALID_SCHEMA`, `E_MISSING_FIELD`, `E_BAD_VERSION`, `E_ALREADY_FROZEN`, `E_IO`, `E_CSV_PARSE`, `E_EMPTY`, `E_COLUMN_NOT_FOUND`

With `--json`, refusals are emitted in the unified output envelope (`outcome=REFUSAL`, refusal detail in `result`). Without `--json`, refusals are human-readable errors on stderr with the refusal code.

### Managed paths

- Frozen profile lookup: `~/.cmdrvl/config/profile/profiles/`; legacy `~/.epistemic/profiles/` is copied on first default use.
- Fabric config for `push`/`pull`: `$EPISTEMIC_FABRIC_URL` or `~/.cmdrvl/config/profile/config.toml`; legacy `~/.epistemic/config.toml` is copied on first default use.

### Witness behavior

- Witness append is enabled for: `freeze`, `validate`, `lint`, `slice`, `stats`, `suggest-key`
- Witness append is skipped for: `draft new`, `draft init`, `emit-discovery`, `list`, `show`, `diff`, `push`, `pull`
- `--no-witness` disables witness writes without changing domain outcome or exit semantics
- Ledger path: `$EPISTEMIC_WITNESS` or `~/.cmdrvl/state/witness/witness.jsonl`; legacy `~/.epistemic/witness.jsonl` is copied on first default use.
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
| **Registry paths are local** | Profiles can reference local column registries, but registry distribution/resolution is still path-based in v0 |
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

Fingerprint identifies *what kind* of file something is (template recognition). `fingerprint peek` can also provide row-shape metadata for messy CSV exports. Profile consumes that shape metadata through `draft init --from-peek` and `pre_parse`, then declares which cleaned columns report tools should analyze.

### Why is key declaration important?

Without a key, report tools like `rvl` can't align rows between two datasets. The key column(s) define how rows map from old to new. `profile suggest-key` helps identify the best candidate.

### Can profiles be generated programmatically?

Yes. `profile draft init` generates a starting profile from a CSV header, and `--from-peek` can seed `pre_parse` from `fingerprint peek --suggest`. You can also write YAML directly or generate it from any tool.

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

---

*`profile` is part of the open-source toolchain from the [CMD+RVL](https://cmdrvl.com) lineage and AI enablement practice. MIT-licensed. Contributions welcome from any practice or stack.*
