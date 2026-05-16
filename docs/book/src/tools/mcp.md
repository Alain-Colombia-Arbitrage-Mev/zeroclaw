# MCP

ZeroClaw supports the **Model Context Protocol (MCP)**, allowing you to extend the agent's capabilities with external tools and context providers. This guide explains how to register and configure MCP servers.

## Overview

MCP servers can be connected via three transport types:
- **stdio**: Long-running local processes (e.g., Node.js or Python scripts).
- **sse**: Remote servers via Server-Sent Events.
- **http**: Simple HTTP POST-based servers.

## Configuration

MCP servers are configured under `[mcp]` and `[[mcp.servers]]` in `config.toml`. The display `name` (used as the tool prefix `name__tool_name`) is required, plus `transport` (`stdio` | `sse` | `http`) and the transport-specific fields. See the [Config reference](../reference/config.md) for the full field index and defaults.

Keep `deferred_loading = true` (the default) to load tool schemas on demand — this minimizes initial token overhead.

## Security and Auto-Approval

By default, any tool execution from an MCP server requires manual approval unless your autonomy level is set to `full`.

To automatically approve tools from a specific MCP server, add its prefix to the `auto_approve` list in the `[autonomy]` section:

```toml
[autonomy]
auto_approve = [
  "my_local_tool__read_file", # Allow specific tool from 'my_local_tool'
  "my_remote_tool__get_weather" # Allow specific tool from 'my_remote_tool'
]
```

## Tips

- **Tool Filtering**: You can limit which MCP tools are exposed to the LLM using `tool_filter_groups` in your project configuration.
- **Deferred Loading**: Keeping `deferred_loading = true` reduces the initial token overhead by only sending tool names to the LLM. The agent will fetch the full schema only when it decides to use the tool.

## Example: Scrapling (web scraping)

[Scrapling](https://github.com/D4Vinci/Scrapling) is a Python web-scraping framework that ships a stdio MCP server exposing tools for HTTP fetching, stealth fetching (Cloudflare Turnstile bypass), dynamic-browser fetching, screenshots, and session management.

### Install

```bash
pip install "scrapling[ai]"
scrapling install   # one-time browser dependency setup
```

### Register in `config.toml`

```toml
[mcp]
enabled = true

[[mcp.servers]]
name = "scrapling"
transport = "stdio"
command = "scrapling"
args = ["mcp"]
tool_timeout_secs = 180
```

After restart, Scrapling's tools are exposed to the agent under the `scrapling__` prefix — for example `scrapling__get`, `scrapling__fetch`, `scrapling__stealthy_fetch`, `scrapling__screenshot`, `scrapling__open_session`.

### Notes

- The first call after install may be slow because Scrapling spawns headless browsers on demand.
- For long-running crawls, raise `tool_timeout_secs` (hard-capped at 600 seconds by ZeroClaw).
- To auto-approve specific Scrapling tools, add their prefixed names to `autonomy.auto_approve` (see above).
- If `scrapling` is not on `PATH`, set `command` to the absolute path of the executable (for example, the one inside your virtualenv's `bin/` or `Scripts/`).
