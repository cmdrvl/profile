# profile Agent Ergonomics Playbook

## First Commands

1. `profile --robot-triage`
2. `profile capabilities --json`
3. `profile robot-docs guide`

These are read-only and do not require profile paths, datasets, registries, witness ledgers, stdin, or network endpoints.

## Repair Policy

`profile doctor --fix` and `profile doctor fix` are intentionally unavailable in this release. They exit 2 and point agents at read-only alternatives instead of implying automatic profile repair is safe.

## Structured Output

Use `--json` for normal command parsing. `profile capabilities --json` and `profile --json --robot-triage` use the unified `profile.v0` envelope; `profile --robot-triage` without `--json` emits the triage object directly for one-call discovery.
