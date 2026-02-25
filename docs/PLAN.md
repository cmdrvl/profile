# profile — Column Scoping for Report Tools

## One-line promise

**Create, validate, and freeze column-scoping profiles that tell report tools which columns to analyze.**

Profiles are versioned YAML configuration — simple, iterable, and deterministic. If the profile is frozen, its hash is recorded for reproducibility.

Second promise: **Explore with drafts, commit with frozen. The profile is cheap to iterate and expensive to change once frozen — exactly the right incentive structure.**

---

## Problem (clearly understood)

Report tools (`rvl`, `compare`, `shape`) need to know which columns to analyze. Today this means:

- Analyzing all columns by default, including noisy or irrelevant ones
- Manually specifying columns via CLI flags on every run
- No record of which columns were used for a given analysis
- No versioned, reusable column-scoping configuration
- No way to distinguish "exploratory scoping" from "production scoping"
- No schema gating before downstream ingestion

`profile` replaces that with **versioned, hashable column-scoping configuration** that report tools consume, agents iterate on, and evidence packs preserve.

---

## Non-goals (explicit)

`profile` is NOT:

- A template recognizer (that's `fingerprint`)
- A diff tool (that's `rvl` / `compare`)
- A comparability gate (that's `shape`)
- A general-purpose extraction/transform pipeline (it only reads enough dataset structure for profile authoring checks)
- A fingerprint (profiles are YAML config, not Rust assertion crates)

It does not tell you *what the data means*.
It tells report tools *which columns to look at* and *how to normalize values* for comparison.

**Clarification: profiles vs fingerprints.** Both are versioned and reviewable, but they serve different purposes:

| Concept | Tool | What it is | Who authors |
|---------|------|-----------|-------------|
| **Profile** | `rvl`, `compare`, `shape` | YAML config — "analyze only these columns" | Agents, analysts |
| **Fingerprint** | `fingerprint` | Rust crate — "does this match this template?" | Engineers, DSL |

The `fingerprint` tool does not use profiles. Profiles are consumed by report tools only.

---

## Relationship to the pipeline

`profile` is **not a stream pipeline tool**. It does not sit in the `vacuum | hash | fingerprint | lock` pipeline. Instead, it is a **configuration authoring tool** whose output (profile YAML files) is consumed by report tools:

```bash
# Create a draft profile from a real dataset
profile draft init tape.csv --out loan_tape.draft.yaml

# Iterate with a draft (cheap, exploratory)
rvl nov.csv dec.csv --key loan_id --profile loan_tape.draft.yaml --json

# Freeze when the answer is useful (immutable, hashable)
profile freeze loan_tape.draft.yaml \
  --family csv.loan_tape.core --version 0 \
  --out profiles/csv.loan_tape.core.v0.yaml

# Use frozen profile for production
rvl nov.csv dec.csv --key loan_id --profile profiles/csv.loan_tape.core.v0.yaml --json
```

Profile outputs flow to report tools, not to the stream pipeline:

```
                    profile
                      │
                      ▼
              profile YAML files
              ┌───────┼───────┐
              ▼       ▼       ▼
            shape   compare   rvl
```

---

## Draft → Frozen Lifecycle

Profiles have two states:

**Draft** — a proposed profile, often agent-generated. Useful for iteration. Accepted by all report tools for exploratory analysis. Profile hash is `null` in output.

**Frozen** — validated, canonicalized, hashed, and immutable. When you want to record the exact scoping used for an analysis, freeze the profile. The frozen profile's hash becomes part of the audit trail.

The transition from draft to frozen is an explicit act (`profile freeze`). This is a boundary crossing — it converts a working configuration into an immutable reference.

### The explore/commit boundary

| Stage | Profile state | What you're doing |
|-------|--------------|-------------------|
| **Explore** | Draft | Iterating — figuring out which columns matter, testing scoping, refining the question |
| **Commit** | Frozen | Recording — the profile hash is recorded in reports and packs for reproducibility |

Report tools accept both. The difference is whether the profile hash is recorded (frozen) or `null` (draft).

**This separation lets agents iterate freely with drafts, then freeze only when the profile produces useful results.**

---

## CLI (v0.1 target)

```
profile <SUBCOMMAND> [OPTIONS]
```

### Subcommands

The list below shows the full interface roadmap. v0.1 ships the subset in [Scope: v0.1](#scope-v01-ship-this).

```
Commands:
  draft new              Create a new draft profile (blank template)
  draft init <DATASET>   Create a draft profile from a real dataset (CSV header-driven)
  validate <FILE>        Validate a profile against the schema
  lint <PROFILE>         Validate + check a profile against a dataset
  stats <DATASET>        Deterministic structural stats for a dataset
  suggest-key <DATASET>  Rank candidate key columns deterministically
  freeze <DRAFT>         Freeze a draft into an immutable profile
  list                   List available frozen profiles
  show <PROFILE_ID>      Show a resolved profile
  diff <A> <B>           Diff two profile versions
  push <FILE>            Publish a frozen profile to data-fabric (deferred in v0.1)
  pull <PROFILE_ID>      Fetch a frozen profile by ID from data-fabric (deferred in v0.1)
  witness <query|last|count>  Query the witness ledger
```

### Subcommand details

```
profile draft new --format <FORMAT> --out <FILE>
  --format <FORMAT>      csv (v0.1); other formats deferred

profile draft init <DATASET> --out <FILE> [--format <FORMAT>] [--key <COLUMN>]
  --format <FORMAT>      csv (v0.1); others deferred
  --out <FILE>           Output path for draft profile YAML
  --key <COLUMN>         Optional: set key explicitly
  --key auto             Optional: set key to the top suggest-key candidate

profile validate <FILE> [--json]

profile lint <PROFILE> --against <DATASET> [--json]
  (checks schema validity, then checks referenced columns/key exist in the dataset)

profile stats <DATASET> [--profile <FILE>] [--json]
  (reports column counts, null rates, and key viability; deterministic ordering)

profile suggest-key <DATASET> [--top <N>] [--json]
  (ranks candidates by uniqueness, null rate, and stability signals; deterministic)

profile freeze <DRAFT> --family <FAMILY> --version <INT> --out <FILE>
  --family <FAMILY>      Stable family name (e.g., csv.loan_tape.core)
  --version <INT>        Monotonic version integer
  --out <FILE>           Output path for frozen profile

profile list [--json]
  (v0.1 searches ~/.epistemic/profiles/; built-ins and EPISTEMIC_PROFILE_PATH are deferred)

profile show <PROFILE_ID> [--json]

profile diff <PROFILE_A> <PROFILE_B> [--json]
  (each argument: try as file path first, then resolve as profile ID)

profile witness <query|last|count>
  (queries witness ledger; read-only)

profile push <FROZEN_PROFILE>
  (deferred in v0.1; publishes to data-fabric via thin HTTP wrapper)

profile pull <PROFILE_ID> --out <DIR>
  (deferred in v0.1; fetches from data-fabric via thin HTTP wrapper)
```

### Common flags (all subcommands)

- `--describe`: Print `operator.json` to stdout and exit 0. Checked before input is validated.
- `--schema`: Print JSON Schema for profile YAML to stdout and exit 0. Like `--describe`, checked before input is validated. Deferred in v0.1.
- `--version`: Print `profile <semver>` to stdout and exit 0.
- `--no-witness`: Suppress witness ledger recording.

### Exit codes

- `0`: SUCCESS — operation completed without issues.
- `1`: ISSUES_FOUND — `lint` found issues, `diff` found differences.
- `2`: REFUSAL — invalid input, schema violation, CLI error.

When implemented, network subcommands (`push`/`pull`) return `0` on success and `2` on refusal/transport failure (no domain-level `1` outcome).

### Output modes

`profile` is a subcommand tool that mixes modes:

| Subcommand | Output mode | `--json` |
|------------|-------------|----------|
| `draft new`, `draft init`, `freeze` | YAML file (artifact) | N/A — output is a file |
| `stats`, `suggest-key` | Report (human default) | Switches to JSON on stdout |
| `lint`, `validate` | Report (human default) | Switches to JSON on stdout |
| `list`, `show`, `diff` | Report (human default) | Switches to JSON on stdout |
| `witness` | Report (human default) | N/A |
| `push`, `pull` | Status message (deferred in v0.1) | N/A |

---

## Profile File Schema (v0)

### Draft profile

```yaml
schema_version: 1
status: draft
format: csv

key: [loan_id]

include_columns:
  - loan_id
  - balance
  - rate
  - maturity_date

equivalence:
  float_decimals: 6
  trim_strings: true
```

Draft profiles may omit `hashing` and `equivalence.order` — `profile freeze` fills in defaults (`sha256`, `order-invariant`) for any omitted optional fields before canonicalizing.

### Frozen profile

```yaml
schema_version: 1
profile_id: csv.loan_tape.core.v0
profile_version: 0
profile_family: csv.loan_tape.core
profile_sha256: "sha256:..."
status: frozen
format: csv

hashing:
  algorithm: sha256

equivalence:
  order: order-invariant
  float_decimals: 6
  trim_strings: true

key: [loan_id]

include_columns:
  - loan_id
  - balance
  - rate
  - maturity_date
```

### Profile fields

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `schema_version` | int | yes | Always `1` for v0 |
| `profile_id` | string | frozen only | `family.vN` (e.g., `csv.loan_tape.core.v0`) |
| `profile_version` | int | frozen only | Monotonic within family |
| `profile_family` | string | frozen only | Stable name (e.g., `csv.loan_tape.core`) |
| `profile_sha256` | string | frozen only | SHA256 of canonicalized content (excludes `profile_sha256` itself to avoid circular dependency) |
| `status` | string | yes | `"draft"` or `"frozen"` |
| `format` | string | yes | v0.1 accepts `csv`; other formats are deferred |
| `hashing` | object | no (default on freeze) | `{ algorithm: "sha256" }` |
| `equivalence` | object | no (default on freeze) | Normalization rules |
| `equivalence.order` | string | no (default on freeze) | `"order-invariant"` (default) or `"order-sensitive"` |
| `equivalence.float_decimals` | int | no | Decimal places for float comparison |
| `equivalence.trim_strings` | bool | no | Trim whitespace before comparison |
| `key` | array | no | Key column(s) for row alignment |
| `include_columns` | array | yes | Columns to analyze (in order) |

---

## Versioning Rules

- `profile_family` = stable name (e.g., `csv.loan_tape.core`)
- `profile_version` = integer; monotonic within family by policy
- `profile_id` = `family.vN` (e.g., `csv.loan_tape.core.v0`)
- **Any semantic change is breaking by definition** — if you change semantics, you changed the meaning of the profile

v0.1 validates family/version syntax and non-negative integer constraints. Global monotonicity enforcement is deferred until registry-backed workflows (`push` / `pull`) are implemented.

### What counts as breaking (requires new version)

- Include/exclude column changes
- Normalization changes (trim, float quantization, date format)
- Key requirement changes
- Equivalence changes (order-sensitive ↔ order-invariant)
- Hash algorithm or canonicalization changes

### What can be non-semantic (during draft iteration)

- Any future annotation fields that do not affect `key`, `include_columns`, or `equivalence`
- Editorial changes (comments, ordering in non-canonical source YAML)
- Once frozen, any edit still requires a new `profile_version` (and therefore a new `profile_id`); `profile_sha256` changes automatically

---

## How Profiles Flow Through Report Tools

**`profile`** creates, validates, freezes profiles. Computes `profile_sha256` for frozen profiles.

**`rvl`** uses the profile to scope which columns to analyze. Records `profile_id` (and `profile_sha256` if frozen) in output.

**`compare`** uses the profile to scope which columns to diff.

**`shape`** uses the profile to check if expected columns exist.

**`pack`** can include frozen profiles alongside reports for reproducibility.

Report tools accept `--profile <PATH>` or `--profile-id <ID>` (mutually exclusive; providing both is a refusal — `E_AMBIGUOUS_PROFILE` in the report tool's own refusal codes, not profile's). Without a profile, all columns are considered (the default, backward-compatible behavior). The profile used is recorded in the output as `profile_id` and `profile_sha256` (both populated if frozen, both `null` if draft).

---

## Profile Resolution Order

1. Explicit `--profile <PATH>` (file path)
2. `--profile-id <ID>` resolved via (first match wins):
   a. `~/.epistemic/profiles/` (v0.1)

Built-in profiles and `EPISTEMIC_PROFILE_PATH` resolution are deferred in v0.1.

```bash
# List available profiles
profile list
# csv.loan_tape.core.v0 (~/.epistemic/profiles/)
```

---

## Example Profiles

| Profile | What it scopes | Use case |
|---------|---------------------|----------|
| `csv.schema.v0` | All column names, no equivalence tuning | Schema drift detection (column presence only) |
| `csv.content.full.v0` | All values, order-invariant | Full content identity |
| `csv.loan_tape.core.v1` | Only loan_id, balance, rate, maturity | Loan tape reconciliation |
| `csv.risk_inputs.v2` | Only fields used by a specific model | Model input stability |

These are illustrative examples, not guaranteed built-in profiles in v0.1.

You cannot collapse these into one profile without lying. By making profile scoping explicitly contextual:
- **Future-proofing** — new use cases produce new profiles, not new tools
- **Honest comparability** — "same for purpose X, different for purpose Y"
- **Clean lineage** — "this output depends on profile Z at version N"
- **Reproducibility** — profiles are versioned and recorded in report outputs and packs
- **Better agent orchestration** — agents choose profiles, not meanings

---

## Refusal Codes

| Code | Trigger | Next step |
|------|---------|-----------|
| `E_INVALID_SCHEMA` | Profile file fails schema validation | Fix profile fields |
| `E_MISSING_FIELD` | Required field not declared | Add missing field |
| `E_BAD_VERSION` | `--family` / `--version` fails syntax or integer constraints | Fix version or family string |
| `E_ALREADY_FROZEN` | Attempting to freeze an already-frozen profile | Profile is immutable; create new version |
| `E_IO` | Can't read/write file | Check paths |
| `E_CSV_PARSE` | Can't parse dataset (for init/lint/stats/suggest-key) | Check format/delimiter |
| `E_EMPTY` | Dataset missing header, or row-dependent operation has no data rows | Provide a non-empty dataset |
| `E_COLUMN_NOT_FOUND` | Profile references a column not present in dataset (`stats --profile`) | Fix profile columns or use correct dataset |

Per-code `detail` schemas:

```
E_INVALID_SCHEMA:
  { "errors": [{ "field": "equivalence.order", "error": "must be 'order-invariant' or 'order-sensitive'" }] }

E_MISSING_FIELD:
  { "field": "include_columns" }

E_BAD_VERSION:
  { "family": "csv..bad name", "version": 0, "error": "family/version validation failed" }

E_ALREADY_FROZEN:
  { "profile_id": "csv.loan_tape.core.v0", "profile_sha256": "sha256:..." }

E_COLUMN_NOT_FOUND:
  { "columns": ["accrued_interest", "orig_balance"], "available": ["loan_id", "balance", "rate", "maturity_date"] }

E_IO:
  { "path": "tape.csv", "error": "No such file or directory (os error 2)" }

E_CSV_PARSE:
  { "path": "tape.csv", "error": "invalid UTF-8 at byte 4201" }

E_EMPTY:
  { "path": "tape.csv", "reason": "no header row | no data rows" }
```

Refusal envelope (emitted to stdout):

```json
{
  "version": "profile.v0",
  "outcome": "REFUSAL",
  "refusal": {
    "code": "E_COLUMN_NOT_FOUND",
    "message": "Profile references columns not present in dataset",
    "detail": { "columns": ["accrued_interest"], "available": ["loan_id", "balance", "rate"] },
    "next_command": null
  }
}
```

---

## Witness Record

profile appends a witness record for subcommands that perform deterministic operations (freeze, lint, validate, stats, suggest-key). All other subcommands — draft creation (`draft new`, `draft init`), read-only queries (`list`, `show`, `diff`), and network subcommands (`push`, `pull`) — do not produce witness records.

The record follows the standard `witness.v0` schema:

```json
{
  "id": "blake3:...",
  "tool": "profile",
  "version": "0.1.0",
  "binary_hash": "blake3:...",
  "inputs": [
    { "path": "loan_tape.draft.yaml", "hash": "blake3:...", "bytes": 342 }
  ],
  "params": { "subcommand": "freeze", "family": "csv.loan_tape.core", "version": 0 },
  "outcome": "SUCCESS",
  "exit_code": 0,
  "output_hash": "blake3:...",
  "prev": "blake3:...",
  "ts": "2026-02-24T10:00:00Z"
}
```

Possible outcomes: `SUCCESS` (exit 0), `ISSUES_FOUND` (exit 1, e.g., lint), `REFUSAL` (exit 2).

Per-subcommand `params` shapes:

```
freeze:      { "subcommand": "freeze", "family": "...", "version": 0 }
validate:    { "subcommand": "validate" }
lint:        { "subcommand": "lint", "against": "tape.csv" }
stats:       { "subcommand": "stats", "profile": "loan_tape.v0" | null }
suggest-key: { "subcommand": "suggest-key", "top": 5 }
```

The `output_hash` is BLAKE3 of the primary output. For artifact subcommands (`freeze`), this is the emitted file content. For report subcommands (`stats`, `suggest-key`, `lint`, `validate`), this is the stdout output. `inputs` lists the files consumed by the subcommand. For `lint`, inputs include both the profile and the dataset.

---

## Implementation Notes

### What `draft init` does (v0.1)

1. Parses the dataset header deterministically (CSV)
2. Emits a draft profile with `include_columns` set to the header columns (in file order)
3. Sets `key` to the provided `--key`, to the top `suggest-key` candidate when `--key auto` (or `[]` if no viable candidate is found), or to an empty list otherwise
4. Sets `equivalence.float_decimals: 6` and `equivalence.trim_strings: true` in the draft (editable before freezing). Omits `equivalence.order` and `hashing` — those are filled in by `freeze` with defaults (`order-invariant`, `sha256`)

### What `freeze` does (summary)

Opens a draft, validates it, checks it isn't already frozen, validates family/version format, fills defaults, sets identity fields, canonicalizes, computes SHA256, writes the frozen file, and appends a witness record. See the detailed execution flow (steps a–j under `freeze:` in the Execution flow section) for the authoritative step-by-step with refusal codes.

Any change after freeze = new version, no exceptions.

### What `suggest-key` does

Ranks candidate key columns deterministically by:
1. Uniqueness (% of distinct values / total rows)
2. Null rate (lower is better)
3. Stability signals (column name heuristics: `*_id`, `*_key`, `*_number`)

Output is a ranked list. When `--json` is provided, output is a JSON array of `{ column, uniqueness, null_rate, rank }`.

### Execution flow (per-subcommand)

```
 1. Parse CLI args (clap)                → exit 2 on bad args; --version handled by clap
 2. If --describe: print operator.json, exit 0
 3. If --schema (when implemented): print JSON Schema, exit 0
 4. If witness subcommand: dispatch to witness query/last/count, exit
 5. Dispatch to subcommand handler:

    draft new:
      a. Build blank template for --format
      b. Write to --out                    → E_IO if write fails
      c. Exit 0

    draft init:
      a. Open dataset file                 → E_IO if not found or permission denied
      b. Parse dataset header              → E_CSV_PARSE if invalid, E_EMPTY if no header
      c. Build draft profile from columns
      d. If --key auto: run suggest-key internally, set top candidate
      e. Write to --out                    → E_IO if write fails
      f. Exit 0

    validate:
      a. Open profile file                 → E_IO if not found or permission denied
      b. Parse profile YAML                → E_INVALID_SCHEMA if not valid YAML
      c. Validate against schema           → E_MISSING_FIELD if required field absent; E_INVALID_SCHEMA on other failures
      d. Report results
      e. Exit 0 (valid) or 2 (invalid)

    lint:
      a. Validate profile (same as validate, including E_IO for file access)
      b. Open dataset file                 → E_IO if not found or permission denied
      c. Parse dataset header              → E_CSV_PARSE if invalid, E_EMPTY if no header
      d. Check all include_columns exist   → report missing columns (domain finding, not refusal)
      e. Check key columns exist           → report missing keys (domain finding, not refusal)
      f. Exit 0 (all clear) or 1 (issues found) or 2 (refusal from steps a-c)

    stats:
      a. Open dataset file                 → E_IO if not found or permission denied
      b. Parse dataset                     → E_CSV_PARSE if invalid, E_EMPTY if no data rows
      c. If --profile: open and parse profile YAML → E_IO / E_INVALID_SCHEMA
      d. If --profile: validate profile columns exist in dataset → E_COLUMN_NOT_FOUND if missing
      e. Compute column counts, null rates, uniqueness scores (scoped to profile columns if provided)
      f. Emit report (human or --json)
      g. Exit 0 (or 2 if refusal from steps c-d)

    suggest-key:
      a. Open dataset file                 → E_IO if not found or permission denied
      b. Parse dataset                     → E_CSV_PARSE if invalid, E_EMPTY if no data rows
      c. Rank candidate keys deterministically
      d. Emit ranked list (human or --json)
      e. Exit 0

    freeze:
      a. Open draft file                   → E_IO if not found or permission denied
      b. Parse YAML                        → E_INVALID_SCHEMA if not valid YAML
      c. Validate against schema           → E_MISSING_FIELD if required field absent; E_INVALID_SCHEMA if wrong structure
      d. Check not already frozen          → E_ALREADY_FROZEN
      e. Validate --family format and version integer constraints (v0.1; global monotonicity deferred) → E_BAD_VERSION if invalid
      f. Fill defaults, set identity fields (status, profile_id, version, family)
      g. Canonicalize (stable field order, all fields including identity EXCEPT profile_sha256)
      h. Compute profile_sha256 (SHA256 of canonicalized content from step g)
      i. Write frozen profile to --out     → E_IO if write fails
      j. Exit 0

    list:
      a. Search resolution paths (~/.epistemic/profiles/ in v0.1)
      b. Emit list (human or --json)
      c. Exit 0

    show:
      a. Resolve profile_id to file        → E_IO if not found in any resolution path
      b. Emit profile (human or --json)
      c. Exit 0 (or 2 if resolution failed)

    diff:
      a. Resolve both profiles (paths or IDs) → E_IO if either not found
      b. Compute structural diff
      c. Emit diff report (human or --json)
      d. Exit 0 (identical) or 1 (differences found)

    push:
      (deferred in v0.1)
      a. Open and parse profile file       → E_IO if not found; E_INVALID_SCHEMA if not valid
      b. Validate profile is frozen        → E_INVALID_SCHEMA if status is not "frozen"
      c. POST to data-fabric
      d. Exit 0 (published) or 2 (transport/refusal)

    pull:
      (deferred in v0.1)
      a. GET from data-fabric by profile_id
      b. Write to --out
      c. Exit 0 (fetched) or 2 (not found/transport error)

 6. Append witness record (if applicable, if not --no-witness)
 7. Exit
```

### Cli struct

```rust
#[derive(Parser)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Suppress witness ledger recording
    #[arg(long, global = true)]
    pub no_witness: bool,

    /// Print operator.json and exit
    #[arg(long, global = true)]
    pub describe: bool,

    /// Print JSON Schema and exit
    #[arg(long, global = true)]
    pub schema: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Draft profile subcommands
    Draft {
        #[command(subcommand)]
        action: DraftAction,
    },
    /// Validate a profile against the schema
    Validate {
        /// Profile YAML file
        file: PathBuf,
        /// JSON output
        #[arg(long)]
        json: bool,
    },
    /// Validate + check profile against a dataset
    Lint {
        /// Profile YAML file
        profile: PathBuf,
        /// Dataset to check against
        #[arg(long)]
        against: PathBuf,
        /// JSON output
        #[arg(long)]
        json: bool,
    },
    /// Deterministic structural stats for a dataset
    Stats {
        /// Dataset file
        dataset: PathBuf,
        /// Scope stats to profile columns
        #[arg(long)]
        profile: Option<PathBuf>,
        /// JSON output
        #[arg(long)]
        json: bool,
    },
    /// Rank candidate key columns deterministically
    SuggestKey {
        /// Dataset file
        dataset: PathBuf,
        /// Number of top candidates
        #[arg(long, default_value = "5")]
        top: usize,
        /// JSON output
        #[arg(long)]
        json: bool,
    },
    /// Freeze a draft into an immutable profile
    Freeze {
        /// Draft profile YAML
        draft: PathBuf,
        /// Stable family name
        #[arg(long)]
        family: String,
        /// Monotonic version integer
        #[arg(long)]
        version: u32,
        /// Output path for frozen profile
        #[arg(long)]
        out: PathBuf,
    },
    /// List available frozen profiles
    List {
        /// JSON output
        #[arg(long)]
        json: bool,
    },
    /// Show a resolved profile
    Show {
        /// Profile ID to show
        profile_id: String,
        /// JSON output
        #[arg(long)]
        json: bool,
    },
    /// Diff two profile versions
    Diff {
        /// First profile (path or ID)
        a: String,
        /// Second profile (path or ID)
        b: String,
        /// JSON output
        #[arg(long)]
        json: bool,
    },
    /// Publish a frozen profile to data-fabric (deferred in v0.1)
    Push {
        /// Frozen profile YAML
        file: PathBuf,
    },
    /// Fetch a frozen profile by ID from data-fabric (deferred in v0.1)
    Pull {
        /// Profile ID
        profile_id: String,
        /// Output directory
        #[arg(long)]
        out: PathBuf,
    },
    /// Query the witness ledger
    Witness {
        #[command(subcommand)]
        action: WitnessAction,
    },
}

#[derive(Subcommand)]
pub enum DraftAction {
    /// Create a blank draft template
    New {
        /// Format (v0.1: csv; other formats deferred)
        #[arg(long)]
        format: String,
        /// Output path
        #[arg(long)]
        out: PathBuf,
    },
    /// Create a draft from a real dataset
    Init {
        /// Dataset file
        dataset: PathBuf,
        /// Output path
        #[arg(long)]
        out: PathBuf,
        /// Format (v0.1: csv)
        #[arg(long, default_value = "csv")]
        format: String,
        /// Set key column (or "auto" for suggest-key top candidate)
        #[arg(long)]
        key: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum WitnessAction {
    Query { /* filter flags */ },
    Last,
    Count { /* filter flags */ },
}
```

### Module structure

```
src/
├── cli/
│   ├── args.rs          # clap derive Cli / Command / DraftAction / WitnessAction
│   ├── exit.rs          # Outcome, exit_code()
│   └── mod.rs
├── schema/
│   ├── profile.rs       # Profile struct (draft + frozen), serde
│   ├── validate.rs      # Schema validation logic
│   ├── canonical.rs     # Canonicalization (stable field order) + SHA256
│   └── mod.rs
├── draft/
│   ├── new.rs           # Blank template generation
│   ├── init.rs          # Header-driven draft creation
│   └── mod.rs
├── freeze/
│   ├── freeze.rs        # Draft → frozen transition
│   └── mod.rs
├── lint/
│   ├── lint.rs          # Profile vs dataset validation
│   └── mod.rs
├── stats/
│   ├── stats.rs         # Column stats computation
│   ├── suggest_key.rs   # Key candidate ranking
│   └── mod.rs
├── resolve/
│   ├── resolver.rs      # Profile resolution (path, ID, search paths)
│   ├── list.rs          # List available profiles
│   └── mod.rs
├── diff/
│   ├── diff.rs          # Structural profile diff
│   └── mod.rs
├── network/             # Deferred in v0.1
│   ├── push.rs          # Publish to data-fabric (thin HTTP)
│   ├── pull.rs          # Fetch from data-fabric (thin HTTP)
│   └── mod.rs
├── output/
│   ├── human.rs         # Human-readable output formatting
│   ├── json.rs          # JSON output
│   └── mod.rs
├── refusal/
│   ├── codes.rs         # RefusalCode enum
│   ├── payload.rs       # RefusalPayload construction
│   └── mod.rs
├── witness/
│   ├── record.rs        # Witness record construction
│   ├── ledger.rs        # Append to witness ledger
│   ├── query.rs         # Witness query subcommands
│   └── mod.rs
├── lib.rs               # pub fn run() → u8 (handles errors internally, returns exit code)
└── main.rs              # Minimal: calls profile::run(), maps to ExitCode
```

### `main.rs` (≤15 lines)

```rust
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    let code = profile::run();
    std::process::ExitCode::from(code)
}
```

### Key dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing (derive API) |
| `serde` + `serde_json` | JSON serialization for frozen profiles and report output |
| `serde_yaml` | YAML parsing and emission for profile files |
| `csv` | CSV dataset parsing (for init/lint/stats/suggest-key) |
| `sha2` | SHA256 for `profile_sha256` computation |
| `blake3` | Witness record hashing |
| `chrono` | ISO 8601 timestamp formatting |

---

## Operator Manifest (`operator.json`)

```json
{
  "schema_version": "operator.v0",
  "name": "profile",
  "version": "0.1.0",
  "description": "Creates, validates, and freezes column-scoping profiles for report tools",
  "repository": "https://github.com/cmdrvl/profile",
  "license": "MIT",

  "invocation": {
    "binary": "profile",
    "output_mode": "mixed",
    "output_schema": "profile.v0",
    "json_flag": "--json"
  },

  "arguments": [],

  "options": [],

  "subcommands": [
    { "name": "draft new", "description": "Create blank draft template" },
    { "name": "draft init", "description": "Create draft from dataset header" },
    { "name": "validate", "description": "Validate profile against schema" },
    { "name": "lint", "description": "Validate + check against dataset" },
    { "name": "stats", "description": "Deterministic structural stats" },
    { "name": "suggest-key", "description": "Rank candidate key columns" },
    { "name": "freeze", "description": "Freeze draft into immutable profile" },
    { "name": "list", "description": "List available frozen profiles" },
    { "name": "show", "description": "Show resolved profile" },
    { "name": "diff", "description": "Diff two profile versions" },
    { "name": "witness", "description": "Query the witness ledger" }
  ],

  "exit_codes": {
    "0": { "meaning": "SUCCESS", "domain": "positive" },
    "1": { "meaning": "ISSUES_FOUND", "domain": "negative" },
    "2": { "meaning": "REFUSAL", "domain": "error" }
  },

  "refusals": [
    { "code": "E_INVALID_SCHEMA", "message": "Profile fails schema validation", "action": "fix_input" },
    { "code": "E_MISSING_FIELD", "message": "Required field not declared", "action": "fix_input" },
    { "code": "E_BAD_VERSION", "message": "Family/version syntax or integer constraints failed", "action": "fix_input" },
    { "code": "E_ALREADY_FROZEN", "message": "Profile already frozen", "action": "escalate" },
    { "code": "E_IO", "message": "Can't read/write file", "action": "escalate" },
    { "code": "E_CSV_PARSE", "message": "Can't parse dataset", "action": "fix_input" },
    { "code": "E_EMPTY", "message": "Dataset missing header or data rows required for operation", "action": "fix_input" },
    { "code": "E_COLUMN_NOT_FOUND", "message": "Column not found in dataset", "action": "fix_input" }
  ],

  "capabilities": {
    "formats": ["csv"],
    "profile_aware": false,
    "streaming": false
  },

  "pipeline": {
    "upstream": [],
    "downstream": ["rvl", "compare", "shape"]
  }
}
```

---

## Testing Requirements

### Fixtures

- `datasets/` — small CSV test files:
  - `loan_tape.csv` — standard loan tape with loan_id, balance, rate, maturity_date, etc.
  - `empty.csv` — file with no rows
  - `no_header.csv` — file without a header row
  - `wide.csv` — many columns for suggest-key testing
  - `missing_columns.csv` — subset of expected columns (for lint tests)
- `profiles/` — pre-built profile files:
  - `valid_draft.yaml` — well-formed draft profile
  - `valid_frozen.yaml` — well-formed frozen profile
  - `invalid_schema.yaml` — profile with schema violations
  - `already_frozen.yaml` — frozen profile (for E_ALREADY_FROZEN tests)

### Test categories

- **Draft new tests:** blank template generation for CSV format
- **Draft init tests:** header-driven draft creation; `--key auto` sets correct candidate
- **Validate tests:** valid profiles pass; missing required fields produce E_MISSING_FIELD; other schema violations produce E_INVALID_SCHEMA
- **Lint tests:** columns exist → pass; missing columns → report issues (exit 1); missing key → report
- **Stats tests:** deterministic column stats output; `--json` produces parseable JSON
- **Suggest-key tests:** ranking is deterministic; uniqueness and null rate properly weighted
- **Freeze tests:** draft → frozen with correct SHA256; defaults filled; E_ALREADY_FROZEN on re-freeze; E_BAD_VERSION on invalid family/version format
- **List tests:** finds user profiles from `~/.epistemic/profiles/` with deterministic ordering
- **Show tests:** resolves by profile_id
- **Diff tests:** identical profiles → exit 0; different profiles → exit 1 with diff
- **Refusal tests:** each refusal code produces correct envelope
- **Witness tests:** witness record appended for freeze/lint/validate/stats/suggest-key
- **Canonicalization tests:** same profile content → same SHA256 regardless of field order in source YAML
- **Exit code tests:** 0 success, 1 domain-negative, 2 refusal
- **Golden file tests:** known CSV through `draft init` produces exact expected draft

Deferred test tracks:
- `--schema` behavior
- `push` / `pull` transport and refusal mapping
- Built-in profile and `EPISTEMIC_PROFILE_PATH` resolution

---

## Scope: v0.1 (ship this)

### Must have

- `profile draft new` (blank template, CSV format)
- `profile draft init` (header-driven draft from CSV, `--key`, `--key auto`)
- `profile validate` (schema validation)
- `profile lint --against` (column/key presence checking)
- `profile stats` (column counts, null rates, uniqueness scores, `--json`)
- `profile suggest-key` (deterministic key ranking, `--json`)
- `profile freeze` (canonicalize, SHA256, immutable output)
- `profile list` (search `~/.epistemic/profiles/`)
- `profile show` (resolve and display)
- `profile diff` (structural diff between two profiles)
- `--version` flag
- `operator.json` + `--describe`
- Exit codes 0/1/2
- Refusal system with all 8 codes
- Ambient witness recording + `--no-witness`
- `profile witness <query|last|count>` subcommands

### Can defer

- `profile push` / `profile pull` (requires data-fabric integration)
- `--schema` flag
- Non-CSV format support (xlsx, pdf, parquet, jsonl)
- `EPISTEMIC_PROFILE_PATH` env var resolution
- Built-in profiles from `epistemic` meta-repo

---

## Open Questions

*None currently blocking. Build it.*
