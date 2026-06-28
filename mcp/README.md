# Shared MCP servers

Custom MCP servers shared across every repo that vendors the `standards/` submodule. One
directory per server.

```
mcp/
└── <server-name>/
    ├── server.py      # or a built binary / node entry
    └── README.md      # what it exposes, required env, how to run
```

Consuming repos reference a server by its `standards/mcp/<name>/...` path from their root
`.mcp.json` (see [`../templates/mcp.json`](../templates/mcp.json)). Python servers run via
`uv run --script` with inline deps; keep secrets in env vars, not in `.mcp.json`. Full
convention in [`../docs/skills-and-mcp.md`](../docs/skills-and-mcp.md).

> No shared servers yet — add the first one as a `<server-name>/` directory here.
