#!/usr/bin/env bash
set -euo pipefail

tool="${PROFILE_BIN:-$(pwd)/target/debug/profile}"

"$tool" --robot-triage | jq -e '
  .schema == "profile.doctor.triage.v1"
  and .status == "healthy"
  and .recommended_actions[0].action == "profile capabilities --json"
' >/dev/null

"$tool" capabilities --json | jq -e '
  .version == "profile.v0"
  and .subcommand == "capabilities"
  and .result.agent_surfaces.robot_triage.command == "profile --robot-triage"
  and .result.agent_surfaces.capabilities.command == "profile capabilities --json"
  and .result.agent_surfaces.robot_docs.command == "profile robot-docs guide"
' >/dev/null

"$tool" robot-docs guide | grep -F "profile robot-docs guide" >/dev/null
