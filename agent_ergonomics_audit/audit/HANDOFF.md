# profile Agent Ergonomics Handoff

Completed at: 2026-06-10T05:22:25Z

## Summary

`profile` now has top-level read-only agent discovery surfaces:

- `profile --robot-triage`
- `profile capabilities --json`
- `profile robot-docs guide`

The nested doctor namespace remains available. `profile doctor --fix` and `profile doctor fix` now refuse safely with exit 2 and stderr-only guidance to the read-only alternatives.

## Verification

- Runtime probes passed for the new surfaces.
- Intent corpus: 503 entries, 0 silent failures, 0 useless errors, 500 useful hints, 3 inferred-and-acted.
- Runtime discipline scripts passed for `profile capabilities`: stdout/stderr split, determinism, non-TTY discipline.
- Regression scripts passed: R-001 through R-004.

## Preflight Note

Skill preflight found `flock` missing on macOS. This pass ran single-agent and did not use file-lock-based parallel coordination.

## Deferred

Generalized typo correction and intent inference across all profile subcommands is not bundled into this release because it touches the full Clap error path and should be designed as a focused follow-up.
