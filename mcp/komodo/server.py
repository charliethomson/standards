#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = ["mcp>=1.2.0", "httpx>=0.27"]
# ///
"""Minimal Komodo MCP server for the homelab.

Wraps the Komodo Core API (https://komo.do) so an MCP client can read stack /
server / container state and (optionally) run execute-type requests.

Auth and target are taken from the environment:
  KOMODO_HOST          e.g. https://komodo.dev.thmsn.dev   (required)
  KOMODO_API_KEY       Komodo API key                       (required)
  KOMODO_API_SECRET    Komodo API secret                    (required)
  KOMODO_ALLOW_EXECUTE if truthy ("1"/"true"/"yes"), exposes the execute tool.
                       Off by default — this repo's deploy model says deploys
                       and syncs are triggered by the user, not by Claude.

The Komodo API is a single POST per category with a tagged body:
  POST {host}/read     {"type": "ListStacks",   "params": {}}
  POST {host}/execute  {"type": "DeployStack",  "params": {"stack": "media"}}
Request "type" strings are passed straight through, so any documented Komodo
request works without changing this file — see the Komodo API docs for names.
"""

import os
import sys

import httpx
from mcp.server.fastmcp import FastMCP

HOST = (os.environ.get("KOMODO_HOST") or "").rstrip("/")
API_KEY = os.environ.get("KOMODO_API_KEY") or ""
API_SECRET = os.environ.get("KOMODO_API_SECRET") or ""
ALLOW_EXECUTE = (os.environ.get("KOMODO_ALLOW_EXECUTE") or "").lower() in (
    "1",
    "true",
    "yes",
    "on",
)

if not (HOST and API_KEY and API_SECRET):
    sys.stderr.write(
        "komodo-mcp: KOMODO_HOST, KOMODO_API_KEY and KOMODO_API_SECRET must be set\n"
    )
    sys.exit(1)

HEADERS = {
    "X-Api-Key": API_KEY,
    "X-Api-Secret": API_SECRET,
    "Content-Type": "application/json",
}

mcp = FastMCP("komodo")


def _post(endpoint: str, request_type: str, params: dict | None = None):
    body = {"type": request_type, "params": params or {}}
    with httpx.Client(timeout=30.0) as client:
        resp = client.post(f"{HOST}/{endpoint}", headers=HEADERS, json=body)
        resp.raise_for_status()
        return resp.json()


@mcp.tool()
def komodo_read(request_type: str, params: dict | None = None) -> object:
    """Run a Komodo read-type API request and return the JSON response.

    request_type is a Komodo read variant, e.g. "ListStacks", "GetStack",
    "GetStacksSummary", "ListServers", "GetStackLog". params is the matching
    params object, e.g. {"stack": "media"} for "GetStack".
    """
    return _post("read", request_type, params)


@mcp.tool()
def list_stacks() -> object:
    """List all stacks Komodo manages (convenience wrapper for ListStacks)."""
    return _post("read", "ListStacks")


@mcp.tool()
def get_stack(stack: str) -> object:
    """Get full config + state for one stack by name or id (GetStack)."""
    return _post("read", "GetStack", {"stack": stack})


@mcp.tool()
def get_stacks_summary() -> object:
    """Get a health/status summary across all stacks (GetStacksSummary)."""
    return _post("read", "GetStacksSummary")


if ALLOW_EXECUTE:

    @mcp.tool()
    def komodo_execute(request_type: str, params: dict | None = None) -> object:
        """Run a Komodo execute-type request (DEPLOY/RESTART/STOP/etc.).

        Only available when KOMODO_ALLOW_EXECUTE is set. request_type is an
        execute variant, e.g. "DeployStack", "RestartStack", "StopStack",
        "RunSync", "RunProcedure"; params is the matching params object.
        """
        return _post("execute", request_type, params)


if __name__ == "__main__":
    mcp.run()
