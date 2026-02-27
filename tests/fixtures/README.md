# Test Fixture Corpus

This fixture corpus is shared across `profile` unit and integration tests.

## Directory layout

- `profiles/valid/` — schema-valid draft and frozen profiles for happy-path tests.
- `profiles/invalid/` — malformed or rule-breaking profiles for refusal and lint coverage.
- `datasets/valid/` — deterministic CSV datasets for stats/suggest/lint/freeze scenarios.
- `datasets/invalid/` — parser/empty edge cases.

## Intended usage by test family

- **Draft tests (`bd-2yy`)**
  - `profiles/valid/draft_minimal.yaml`
  - `profiles/valid/draft_with_key.yaml`
  - `datasets/valid/loan_tape_basic.csv`
- **Schema + lint tests (`bd-8rq`)**
  - `profiles/invalid/*`
  - `datasets/valid/loan_tape_missing_rate.csv`
  - `datasets/invalid/*`
- **Stats/suggest/freeze tests (`bd-29f`)**
  - `datasets/valid/loan_tape_basic.csv`
  - `datasets/valid/loan_tape_duplicates.csv`
  - `datasets/valid/no_unique_key.csv`
  - `profiles/valid/frozen_complete.yaml`
- **List/show/diff tests (`bd-2g9`)**
  - `profiles/valid/frozen_complete.yaml`
- **Witness/refusal tests (`bd-3td`)**
  - `profiles/invalid/*`
  - `datasets/invalid/*`

## Determinism rules

- Fixtures are hand-authored and static (no generated timestamps).
- CSV row ordering is fixed.
- YAML key order is stable and human-readable.
- Hash-like values are fixed test literals (never computed at runtime in fixture files).
