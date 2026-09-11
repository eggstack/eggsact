#!/usr/bin/env python3
"""Score recorded direct-vs-discovery MCP traces without provider access.

The input is one or more JSON trace files. Each file has this shape::

    {
      "model": "model/version",
      "client": "client/version",
      "mode": "direct|discovery",
      "initial_tool_definition_bytes": 1234,
      "scenarios": [
        {
          "id": "patch-summary",
          "expected_tool": "patch_summary",
          "acceptable_tools": [],
          "success": true,
          "selected_tool": "patch_summary",
          "mcp_calls": 2,
          "invalid_arguments": 0,
          "wrong_tool_retries": 0
        }
      ]
    }

The script scores supplied traces only. It never calls a model, network, or
MCP server, so it is suitable for maintainer evidence and offline CI checks.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path}: top-level value must be an object")
    for key in ("model", "client", "mode", "scenarios"):
        if key not in value:
            raise ValueError(f"{path}: missing {key}")
    if value["mode"] not in ("direct", "discovery"):
        raise ValueError(f"{path}: mode must be direct or discovery")
    if not isinstance(value["scenarios"], list):
        raise ValueError(f"{path}: scenarios must be an array")
    return value


def score(trace: dict[str, Any]) -> dict[str, Any]:
    scenarios = trace["scenarios"]
    if not scenarios:
        raise ValueError("trace must contain at least one scenario")

    successful = 0
    correct = 0
    invalid_arguments = 0
    wrong_tool_retries = 0
    total_calls = 0
    for scenario in scenarios:
        if not isinstance(scenario, dict):
            raise ValueError("each scenario must be an object")
        expected = scenario.get("expected_tool")
        acceptable = set(scenario.get("acceptable_tools", [])) | {expected}
        selected = scenario.get("selected_tool")
        successful += int(bool(scenario.get("success", False)))
        correct += int(selected in acceptable)
        invalid_arguments += int(scenario.get("invalid_arguments", 0))
        wrong_tool_retries += int(scenario.get("wrong_tool_retries", 0))
        total_calls += int(scenario.get("mcp_calls", 0))

    count = len(scenarios)
    return {
        "model": trace["model"],
        "client": trace["client"],
        "mode": trace["mode"],
        "scenario_count": count,
        "task_success_rate": successful / count,
        "capability_selection_rate": correct / count,
        "invalid_arguments": invalid_arguments,
        "wrong_tool_retries": wrong_tool_retries,
        "average_mcp_calls": total_calls / count,
        "initial_tool_definition_bytes": trace.get("initial_tool_definition_bytes"),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", nargs="+", type=Path, help="recorded JSON trace files")
    args = parser.parse_args()
    results = [score(load(path)) for path in args.trace]
    print(json.dumps({"traces": results}, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
