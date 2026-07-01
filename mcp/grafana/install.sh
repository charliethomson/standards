#!/usr/bin/env bash
# Install Grafana's third-party `mcp-grafana` binary into this directory, so the
# `grafana` server in the repo-root .mcp.json is self-contained. Requires Go.
#
# The binary is gitignored (it's ~67M and platform-specific), so run this once
# per machine after cloning. See https://github.com/grafana/mcp-grafana.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
GOBIN="$here" go install github.com/grafana/mcp-grafana/cmd/mcp-grafana@latest
echo "installed: $here/mcp-grafana"
