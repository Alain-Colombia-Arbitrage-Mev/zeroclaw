# SocialClaw publishing

The `socialclaw` tool wraps the [SocialClaw](https://github.com/ndesv21/socialclaw) npm CLI so the agent can validate, schedule, and publish content across X, LinkedIn, Instagram, Facebook Pages, TikTok, Discord, Telegram, YouTube, Reddit, WordPress, and Pinterest.

## Prerequisites

1. A SocialClaw workspace with an active plan at <https://getsocialclaw.com>.
2. A workspace API key copied from the dashboard.
3. Node.js ≥ 20 and the CLI installed globally:

    ```bash
    npm install -g socialclaw
    socialclaw login --api-key <workspace-key>
    ```

   Verify the install:

   ```bash
   socialclaw accounts list --json
   ```

## Configuration

Add a `[socialclaw]` section to `config.toml`:

```toml
[socialclaw]
enabled = true
api_key = "scw_live_..."                 # stored via OS keyring
base_url = "https://getsocialclaw.com"   # leave default unless self-hosted
allowed_providers = ["x", "linkedin"]    # or ["*"] for any
allowed_commands = []                    # empty = curated safe defaults
timeout_secs = 120
max_output_bytes = 2097152
```

### `allowed_providers`

Enforces a hard allowlist for the `--provider`/`-p`/`--provider=<name>` flag in any subcommand. Use `["*"]` to allow anything supported by your plan, or list explicit names ("x", "linkedin", "instagram", "facebook", "tiktok", "discord", "telegram", "youtube", "reddit", "wordpress", "pinterest"). Matching is case-insensitive.

### `allowed_commands`

When empty, a curated default set mirrors the real surface of `socialclaw --help`:

- lifecycle: `validate`, `apply`, `campaigns preview`, `campaigns inspect`, `campaigns clone`, `publish-draft`
- posts: `posts list`, `posts get`, `posts attempts`, `posts delete`, `posts reconcile`, `delete`, `retry`, `cancel`
- runs / status: `status`, `runs inspect`
- accounts: `accounts list`, `accounts capabilities`, `accounts settings`, `accounts actions`, `accounts connect`, `accounts status`, `accounts disconnect`
- assets: `assets upload`, `assets delete`
- analytics: `analytics post`, `analytics account`, `analytics run`, `analytics refresh`
- workspace / health / jobs: `usage`, `workspace health`, `connections health`, `jobs list`

Intentionally **excluded** from the defaults — add them in `allowed_commands` if you need them:

- `view` — writes formatted output to a file path (path handling left to a follow-up).
- `accounts action` — executes an arbitrary provider-side action with open semantics.
- `login`, `install` — local setup commands; not agent capabilities.

Set `allowed_commands` explicitly to tighten the surface. The CLI invocation form is the multi-word subcommand exactly as `socialclaw` accepts it (`"accounts list"`, `"posts get"`, etc.).

## Invocation

The agent calls the tool with a `command` plus optional `args`. The tool always appends `--json`:

```jsonc
{
  "command": "validate",
  "args": ["-f", "schedule.json"]
}
```

```jsonc
{
  "command": "apply",
  "args": ["-f", "schedule.json", "--provider", "linkedin"]
}
```

```jsonc
{
  "command": "posts get",
  "args": ["--post-id", "<post-id>"]
}
```

## Security

- The API key is passed as the `SOCIALCLAW_API_KEY` env var to the subprocess and never logged.
- The agent's working directory is the workspace dir — `-f schedule.json` must reference a file inside it.
- Every call goes through `SecurityPolicy.record_action()`, sharing the hourly rate-limit budget with other action tools.
- Read-only autonomy blocks all socialclaw calls.

## Operational notes

- The first call after `socialclaw login` may be slow as the CLI bootstraps OAuth state.
- For long-running `apply` operations against large schedules, raise `timeout_secs`.
- If `socialclaw` is not on `PATH`, set `command` to the absolute path of the binary (`<npm prefix>/bin/socialclaw` on Unix, `<npm prefix>\socialclaw.cmd` on Windows) by replacing the binary lookup — this requires a code change, so prefer fixing the `PATH` environment variable.
- The hosted workspace must have an active trial or paid plan; `plan_required` and `subscription_*` errors come from SocialClaw, not ZeroClaw.
