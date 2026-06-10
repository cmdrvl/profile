#!/usr/bin/env bash
set -euo pipefail

tool="${PROFILE_BIN:-$(pwd)/target/debug/profile}"

"$tool" --describe | jq -e '
  .version == "0.7.0"
  and (.subcommands | any(.name == "capabilities"))
  and (.subcommands | any(.name == "robot-docs"))
  and .capabilities.agent_surfaces.robot_triage == "profile --robot-triage"
  and .capabilities.agent_surfaces.capabilities == "profile capabilities --json"
  and .capabilities.agent_surfaces.robot_docs == "profile robot-docs guide"
' >/dev/null
