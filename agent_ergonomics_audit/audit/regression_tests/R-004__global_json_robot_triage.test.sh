#!/usr/bin/env bash
set -euo pipefail

tool="${PROFILE_BIN:-$(pwd)/target/debug/profile}"

"$tool" --json --robot-triage | jq -e '
  .version == "profile.v0"
  and .outcome == "SUCCESS"
  and .exit_code == 0
  and .subcommand == "robot triage"
  and .result.schema == "profile.doctor.triage.v1"
  and .result.recommended_actions[0].action == "profile capabilities --json"
' >/dev/null
