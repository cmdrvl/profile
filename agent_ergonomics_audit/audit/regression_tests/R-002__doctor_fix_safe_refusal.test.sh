#!/usr/bin/env bash
set -euo pipefail

tool="${PROFILE_BIN:-$(pwd)/target/debug/profile}"

set +e
stderr="$("$tool" doctor --fix 2>&1 >/dev/null)"
status="$?"
set -e

test "$status" -eq 2
grep -F "profile doctor --fix is unavailable" <<<"$stderr" >/dev/null
grep -F "profile --robot-triage" <<<"$stderr" >/dev/null
grep -F "profile capabilities --json" <<<"$stderr" >/dev/null
grep -F "profile robot-docs guide" <<<"$stderr" >/dev/null

set +e
stderr="$("$tool" doctor fix 2>&1 >/dev/null)"
status="$?"
set -e

test "$status" -eq 2
grep -F "profile doctor --fix is unavailable" <<<"$stderr" >/dev/null
grep -F "profile --robot-triage" <<<"$stderr" >/dev/null
