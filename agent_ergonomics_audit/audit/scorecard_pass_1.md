# Scorecard Pass 1

- Surfaces inventoried: 253
- Intent corpus replayed: 503 entries
- Silent failures: 0
- Useless errors: 0
- Useful hints: 500
- Inferred and acted: 3

## Highest-Leverage Improvements

- First-try agent discovery moved from nested-only doctor commands to top-level `--robot-triage`, `capabilities --json`, and `robot-docs guide`.
- Repair guesses now fail safely with exact alternatives instead of generic Clap unknown-argument text.
- `--describe`, README, AGENTS, and PLAN now agree with runtime behavior.

## Deferred

Generalized typo recovery and intent inference for the full 253-surface CLI remains deferred. The current Clap errors are useful, but they do not infer-and-act beyond the new top-level discovery commands.
