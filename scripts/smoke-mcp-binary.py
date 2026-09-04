#!/usr/bin/env python3
"""Run the bounded MCP handshake against the exact staged Eggsact binary."""
import json
import queue
import subprocess
import sys
import threading
import time


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} PATH", file=sys.stderr)
        return 2
    child = subprocess.Popen(
        [sys.argv[1], "--mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, text=True, bufsize=1,
    )
    lines: queue.Queue[str] = queue.Queue()

    def read_lines() -> None:
        assert child.stdout is not None
        for line in child.stdout:
            lines.put(line)

    threading.Thread(target=read_lines, daemon=True).start()

    def request(value: dict) -> dict:
        assert child.stdin is not None
        child.stdin.write(json.dumps(value) + "\n")
        child.stdin.flush()
        try:
            line = lines.get(timeout=15)
        except queue.Empty as exc:
            raise RuntimeError("MCP smoke timed out waiting for response") from exc
        response = json.loads(line)
        if response.get("id") != value.get("id") or "error" in response:
            raise RuntimeError(f"unexpected MCP response: {response}")
        return response

    try:
        initialize = request({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                        "clientInfo": {"name": "eggsact-release-smoke", "version": "1"}},
        })
        if initialize["result"]["serverInfo"]["name"] != "eggsact":
            raise RuntimeError("initialize returned the wrong server identity")
        assert child.stdin is not None
        child.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        child.stdin.flush()
        listed = request({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
        names = {tool["name"] for tool in listed["result"]["tools"]}
        if "math_eval" not in names:
            raise RuntimeError("tools/list did not contain math_eval")
        child.stdin.close()
        child.stdin = None
        deadline = time.monotonic() + 15
        while child.poll() is None and time.monotonic() < deadline:
            time.sleep(0.05)
        if child.poll() is None:
            child.kill()
            raise RuntimeError("MCP process did not shut down after stdin EOF")
        if child.returncode != 0:
            raise RuntimeError(f"MCP process exited with {child.returncode}")
        print(f"MCP smoke passed ({len(names)} tools)")
        return 0
    except Exception as exc:
        child.kill()
        child.wait()
        print(f"MCP smoke failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
