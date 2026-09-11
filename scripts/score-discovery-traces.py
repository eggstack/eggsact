#!/usr/bin/env python3
"""Score recorded direct-vs-discovery MCP traces without provider access.

Canonical trace contract (plural expected/acceptable sets)::

    {
      "model": "openai/gpt-5.2-codex-2026-08-01",
      "provider": "openai",
      "client": "codex-cli/0.42.0",
      "mode": "direct|discovery",
      "run_date": "2026-09-11",
      "initial_tool_definition_bytes": 111911,
      "server_instructions": "on|off",
      "scenario_corpus_revision": "optional git sha or date",
      "scenarios": [
        {
          "id": "s25",
          "expected_tools": ["patch_summary"],
          "acceptable_tools": [],
          "success": true,
          "selected_tool": "patch_summary",
          "search_calls": 1,
          "tool_invoke_calls": 1,
          "mcp_calls": 2,
          "invalid_arguments": 0,
          "wrong_tool_retries": 0
        }
      ]
    }

Legacy singular ``expected_tool`` is accepted as an alias for
``expected_tools: [expected_tool]`` so older maintainer traces still load,
but new traces must use the plural form to match
``tests/fixtures/tool_discovery_scenarios.json`` (which uses
``expected_tools``). Malformed traces fail loudly instead of scoring
``None`` as an expected tool.

Paired comparison::

    python3 scripts/score-discovery-traces.py --pair direct.json discovery.json

requires the same model, provider, client, corpus revision, and
server-instructions treatment with opposite modes and identical scenario
id sets. It reports success/selection/invalid/retry/call/context deltas
and a PASS/FAIL summary for the rollout noninferiority gates:

- discovery success no worse than direct by more than 2 percentage points;
- selection no worse by more than 2 percentage points;
- invalid-argument and retry behaviour not worse in a way that changes
  success (discovery totals must not exceed direct totals when success
  regresses, and retry inflation is reported);
- discovery workflow actually exercised (at least one search and one
  invoke across the corpus).

The script never calls a model, network, or MCP server, so it is suitable
for maintainer evidence and offline CI checks. Do not record credentials,
account identifiers, unrelated prompts, local paths, or sensitive
environment data in trace files.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def _fail(path: Path, message: str) -> ValueError:
    return ValueError(f"{path}: {message}")


def _require_str(obj: dict[str, Any], key: str, path: Path) -> str:
    value = obj.get(key)
    if not isinstance(value, str) or not value.strip():
        raise _fail(path, f"missing or empty {key} (non-empty string required)")
    return value


def _require_int_ge0(obj: dict[str, Any], key: str, path: Path, sid: str) -> int:
    if key not in obj:
        raise _fail(path, f"scenario '{sid}' missing {key} (integer >=0 required)")
    value = obj[key]
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise _fail(path, f"scenario '{sid}' field {key} must be integer >=0")
    return value


def load(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise _fail(path, f"invalid JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise _fail(path, "top-level value must be an object")
    model = _require_str(value, "model", path)
    provider = _require_str(value, "provider", path)
    client = _require_str(value, "client", path)
    mode = value.get("mode")
    if mode not in ("direct", "discovery"):
        raise _fail(path, "mode must be direct or discovery")
    run_date = _require_str(value, "run_date", path)
    if "initial_tool_definition_bytes" not in value:
        raise _fail(path, "missing initial_tool_definition_bytes (integer >=0 required)")
    raw_bytes = value["initial_tool_definition_bytes"]
    if not isinstance(raw_bytes, int) or isinstance(raw_bytes, bool) or raw_bytes < 0:
        raise _fail(path, "initial_tool_definition_bytes must be integer >=0")
    instructions = value.get("server_instructions")
    if instructions not in ("on", "off"):
        raise _fail(path, 'server_instructions must be "on" or "off"')
    scenarios = value.get("scenarios")
    if not isinstance(scenarios, list) or not scenarios:
        raise _fail(path, "scenarios must be a non-empty array")
    seen: set[str] = set()
    normalized: list[dict[str, Any]] = []
    for index, scenario in enumerate(scenarios):
        where = f"scenarios[{index}]"
        if not isinstance(scenario, dict):
            raise _fail(path, f"{where} must be an object")
        sid = scenario.get("id")
        if not isinstance(sid, str) or not sid.strip():
            raise _fail(path, f"{where} missing id (non-empty string required)")
        if sid in seen:
            raise _fail(path, f"duplicate scenario id '{sid}'")
        seen.add(sid)
        # Audience/kind compatibility: traces score task scenarios only.
        # Containment entries (must_not_expose) are skipped from the success
        # denominator but still validated when present.
        kind = scenario.get("kind", "task")
        if kind not in ("task", "must_not_expose"):
            raise _fail(path, f"scenario '{sid}' has unknown kind '{kind}'")
        audience = scenario.get("audience", "model")
        if audience not in ("model", "harness"):
            raise _fail(path, f"scenario '{sid}' has unknown audience '{audience}'")
        if kind == "must_not_expose":
            forbidden = scenario.get("forbidden_tools")
            if not isinstance(forbidden, list) or not forbidden or not all(
                isinstance(t, str) and t.strip() for t in forbidden
            ):
                raise _fail(
                    path, f"scenario '{sid}' must_not_expose needs non-empty forbidden_tools"
                )
            normalized.append(
                {"id": sid, "kind": kind, "audience": audience, "forbidden_tools": forbidden}
            )
            continue
        # Task scenario: canonical plural contract with legacy singular alias.
        expected: list[str] = []
        if "expected_tools" in scenario:
            raw = scenario["expected_tools"]
            if not isinstance(raw, list) or not raw or not all(
                isinstance(t, str) and t.strip() for t in raw
            ):
                raise _fail(
                    path, f"scenario '{sid}' expected_tools must be a non-empty string array"
                )
            expected = list(raw)
        elif "expected_tool" in scenario:
            legacy = scenario["expected_tool"]
            if not isinstance(legacy, str) or not legacy.strip():
                raise _fail(path, f"scenario '{sid}' legacy expected_tool must be non-empty")
            print(
                f"warning: {path} scenario '{sid}' uses legacy expected_tool; "
                "prefer expected_tools",
                file=sys.stderr,
            )
            expected = [legacy]
        else:
            raise _fail(path, f"scenario '{sid}' missing expected_tools")
        acceptable_raw = scenario.get("acceptable_tools", [])
        if not isinstance(acceptable_raw, list) or not all(
            isinstance(t, str) and t.strip() for t in acceptable_raw
        ):
            raise _fail(path, f"scenario '{sid}' acceptable_tools must be a string array")
        if "success" not in scenario or not isinstance(scenario["success"], bool):
            raise _fail(path, f"scenario '{sid}' missing success (boolean required)")
        selected = scenario.get("selected_tool")
        if not isinstance(selected, str) or not selected.strip():
            raise _fail(path, f"scenario '{sid}' missing selected_tool (non-empty string)")
        search_calls = _require_int_ge0(scenario, "search_calls", path, sid)
        invoke_calls = _require_int_ge0(scenario, "tool_invoke_calls", path, sid)
        mcp_calls = _require_int_ge0(scenario, "mcp_calls", path, sid)
        invalid_args = _require_int_ge0(scenario, "invalid_arguments", path, sid)
        retries = _require_int_ge0(scenario, "wrong_tool_retries", path, sid)
        normalized.append(
            {
                "id": sid,
                "kind": kind,
                "audience": audience,
                "expected_tools": expected,
                "acceptable_tools": list(acceptable_raw),
                "success": scenario["success"],
                "selected_tool": selected,
                "search_calls": search_calls,
                "tool_invoke_calls": invoke_calls,
                "mcp_calls": mcp_calls,
                "invalid_arguments": invalid_args,
                "wrong_tool_retries": retries,
            }
        )
    value["_normalized"] = normalized
    # Keep validated header fields accessible for pairing.
    value["_header"] = {
        "model": model,
        "provider": provider,
        "client": client,
        "mode": mode,
        "run_date": run_date,
        "initial_tool_definition_bytes": raw_bytes,
        "server_instructions": instructions,
        "scenario_corpus_revision": value.get("scenario_corpus_revision", ""),
    }
    return value


def score(trace: dict[str, Any]) -> dict[str, Any]:
    header = trace["_header"]
    tasks = [s for s in trace["_normalized"] if s.get("kind", "task") == "task"]
    skipped = len(trace["_normalized"]) - len(tasks)
    if not tasks:
        raise ValueError("trace must contain at least one task scenario")
    successful = 0
    correct = 0
    invalid_arguments = 0
    wrong_tool_retries = 0
    search_calls = 0
    invoke_calls = 0
    total_calls = 0
    for scenario in tasks:
        acceptable = set(scenario["acceptable_tools"]) | set(scenario["expected_tools"])
        successful += int(bool(scenario["success"]))
        correct += int(scenario["selected_tool"] in acceptable)
        invalid_arguments += int(scenario["invalid_arguments"])
        wrong_tool_retries += int(scenario["wrong_tool_retries"])
        search_calls += int(scenario["search_calls"])
        invoke_calls += int(scenario["tool_invoke_calls"])
        total_calls += int(scenario["mcp_calls"])
    count = len(tasks)
    return {
        "model": header["model"],
        "provider": header["provider"],
        "client": header["client"],
        "mode": header["mode"],
        "run_date": header["run_date"],
        "server_instructions": header["server_instructions"],
        "scenario_corpus_revision": header["scenario_corpus_revision"],
        "scenario_count": count,
        "skipped_containment": skipped,
        "task_success_rate": successful / count,
        "task_successes": successful,
        "capability_selection_rate": correct / count,
        "capability_selections": correct,
        "invalid_arguments": invalid_arguments,
        "invalid_argument_rate": invalid_arguments / count,
        "wrong_tool_retries": wrong_tool_retries,
        "wrong_tool_retry_rate": wrong_tool_retries / count,
        "average_search_calls": search_calls / count,
        "total_search_calls": search_calls,
        "average_tool_invoke_calls": invoke_calls / count,
        "total_tool_invoke_calls": invoke_calls,
        "average_mcp_calls": total_calls / count,
        "total_mcp_calls": total_calls,
        "initial_tool_definition_bytes": header["initial_tool_definition_bytes"],
    }


def pair_scores(
    direct_path: Path, discovery_path: Path, direct: dict[str, Any], discovery: dict[str, Any]
) -> dict[str, Any]:
    direct_score = score(direct)
    discovery_score = score(discovery)
    dh = direct["_header"]
    sh = discovery["_header"]
    for key in ("model", "provider", "client", "server_instructions", "scenario_corpus_revision"):
        if dh[key] != sh[key]:
            raise ValueError(
                f"pair mismatch on {key}: direct={dh[key]!r} discovery={sh[key]!r} "
                f"({direct_path} vs {discovery_path}); refusing to compare unequal corpora"
            )
    if dh["mode"] != "direct" or sh["mode"] != "discovery":
        raise ValueError(
            f"--pair expects direct then discovery; got {dh['mode']} then {sh['mode']}"
        )
    direct_ids = [s["id"] for s in direct["_normalized"] if s.get("kind", "task") == "task"]
    discovery_ids = [
        s["id"] for s in discovery["_normalized"] if s.get("kind", "task") == "task"
    ]
    missing_in_discovery = sorted(set(direct_ids) - set(discovery_ids))
    missing_in_direct = sorted(set(discovery_ids) - set(direct_ids))
    if missing_in_discovery or missing_in_direct:
        raise ValueError(
            "scenario id sets differ; refusing to compare unequal corpora: "
            f"missing in discovery={missing_in_discovery} missing in direct={missing_in_direct}"
        )
    success_delta_pp = (
        discovery_score["task_success_rate"] - direct_score["task_success_rate"]
    ) * 100.0
    selection_delta_pp = (
        discovery_score["capability_selection_rate"] - direct_score["capability_selection_rate"]
    ) * 100.0
    direct_bytes = direct_score["initial_tool_definition_bytes"] or 0
    discovery_bytes = discovery_score["initial_tool_definition_bytes"] or 0
    ratio = (discovery_bytes / direct_bytes) if direct_bytes else 0.0
    # C3 noninferiority gates (maintainer evidence, not merge CI).
    gate_success = success_delta_pp >= -2.0
    gate_selection = selection_delta_pp >= -2.0
    # Invalid-argument behaviour must not be worse in a way that changes
    # success or creates repeated retries. Flag when discovery adds invalid
    # args/retries while success regresses, or when retry inflation is large.
    invalid_worse = discovery_score["invalid_arguments"] > direct_score["invalid_arguments"]
    retry_worse = discovery_score["wrong_tool_retries"] > direct_score["wrong_tool_retries"]
    gate_arguments = not (
        (invalid_worse or retry_worse) and success_delta_pp < 0.0
    ) and discovery_score["wrong_tool_retry_rate"] <= direct_score[
        "wrong_tool_retry_rate"
    ] + 0.10
    gate_workflow = (
        discovery_score["total_search_calls"] > 0
        and discovery_score["total_tool_invoke_calls"] > 0
    )
    overall = gate_success and gate_selection and gate_arguments and gate_workflow
    return {
        "direct": direct_score,
        "discovery": discovery_score,
        "model": dh["model"],
        "provider": dh["provider"],
        "client": dh["client"],
        "server_instructions": dh["server_instructions"],
        "scenario_corpus_revision": dh["scenario_corpus_revision"],
        "task_success_rate_direct": direct_score["task_success_rate"],
        "task_success_rate_discovery": discovery_score["task_success_rate"],
        "success_delta_pp": success_delta_pp,
        "capability_selection_rate_direct": direct_score["capability_selection_rate"],
        "capability_selection_rate_discovery": discovery_score["capability_selection_rate"],
        "selection_delta_pp": selection_delta_pp,
        "invalid_arguments_direct": direct_score["invalid_arguments"],
        "invalid_arguments_discovery": discovery_score["invalid_arguments"],
        "wrong_tool_retries_direct": direct_score["wrong_tool_retries"],
        "wrong_tool_retries_discovery": discovery_score["wrong_tool_retries"],
        "average_search_calls_discovery": discovery_score["average_search_calls"],
        "average_mcp_calls_direct": direct_score["average_mcp_calls"],
        "average_mcp_calls_discovery": discovery_score["average_mcp_calls"],
        "initial_bytes_direct": direct_bytes,
        "initial_bytes_discovery": discovery_bytes,
        "discovery_direct_byte_ratio": ratio,
        "gates": {
            "success_noninferiority_2pp": gate_success,
            "selection_noninferiority_2pp": gate_selection,
            "invalid_retry_no_harm": gate_arguments,
            "discovery_workflow_exercised": gate_workflow,
        },
        "verdict": "PASS" if overall else "FAIL",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", nargs="*", type=Path, help="recorded JSON trace files")
    parser.add_argument(
        "--pair",
        nargs=2,
        type=Path,
        metavar=("DIRECT", "DISCOVERY"),
        help="compare a matched direct/discovery pair and report deltas and gates",
    )
    args = parser.parse_args()
    if args.pair is not None:
        if args.trace:
            parser.error("--pair cannot be combined with positional traces")
        direct_path, discovery_path = args.pair
        direct = load(direct_path)
        discovery = load(discovery_path)
        result = pair_scores(direct_path, discovery_path, direct, discovery)
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0 if result["verdict"] == "PASS" else 1
    if not args.trace:
        parser.error("provide trace files or --pair DIRECT DISCOVERY")
    results = [score(load(path)) for path in args.trace]
    print(json.dumps({"traces": results}, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
