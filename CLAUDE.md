# CLAUDE.md — profile

Follow [`AGENTS.md`](./AGENTS.md). The source-of-truth behavior lives in [`docs/PLAN.md`](./docs/PLAN.md).

Claude-specific reminders:

- Do not change canonical field order casually; it changes frozen profile hashes.
- Do not add broad transform behavior to `profile slice`. It is only pre-parse cleanup from explicit directives.
- Use focused tests first, then the full cargo gate.
- Treat unexpected working-tree changes as concurrent agent work and do not revert them.
- Finish with Beads sync, commit, `git push`, and `git push origin main:master`.
