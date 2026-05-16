# Scrapling CLI (fallback)

The `scrapling_cli` tool wraps the [Scrapling](https://github.com/D4Vinci/Scrapling) `extract` CLI as a direct subprocess. It is a fallback for environments where running the Scrapling MCP server is undesirable (no daemon, simpler ops, one-shot invocations).

If you want the agent to use Scrapling routinely, prefer the [MCP integration](./mcp.md#example-scrapling-web-scraping) — it amortizes browser startup across calls. Use `scrapling_cli` when you need single, isolated scrapes without a persistent server.

## Prerequisites

```bash
pip install "scrapling[shell]"   # provides the `extract` CLI
scrapling install                # one-time browser dependency setup
```

## Configuration

```toml
[scrapling_cli]
enabled = true
allowed_domains = ["example.com", "docs.example.org"]   # or ["*"]
timeout_secs = 180
max_output_bytes = 2097152
env_passthrough = []   # extra env vars to pass through (e.g. PROXY_URL)
```

## Invocation

```jsonc
{ "operation": "get", "url": "https://example.com/page" }
```

Operations map 1:1 to `scrapling extract <op>`:

- `get`, `post`, `put`, `delete` — plain HTTP fetches
- `fetch` — `DynamicFetcher` (browser automation)
- `stealthy_fetch` — `StealthyFetcher` (anti-bot / Cloudflare Turnstile bypass)

Optional extra flags can be passed through the `args` array:

```jsonc
{
  "operation": "stealthy_fetch",
  "url": "https://example.com",
  "args": ["--css", ".product .price"]
}
```

The tool writes Scrapling's output to a temp file inside the workspace, reads its contents (truncated to `max_output_bytes`), and removes the file before returning.

## Security

- Allowlist-only domains, http(s) only, no userinfo, no whitespace.
- `SecurityPolicy.record_action()` consumes the hourly action budget.
- Read-only autonomy blocks every call.
- The subprocess runs with `env_clear()` plus a curated list of safe env vars; add more via `env_passthrough` if your scrape needs `HTTPS_PROXY`, etc.
- Temp files are written under `workspace_dir` and removed even on failure.
