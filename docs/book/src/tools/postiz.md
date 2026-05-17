# Postiz publishing

The `postiz` tool publishes and schedules social content via [Postiz](https://github.com/gitroomhq/postiz-app) — an open-source social media scheduler that can run either self-hosted or against the Postiz cloud.

Compared to [socialclaw](./socialclaw.md): same use case, different model. socialclaw depends on the hosted `getsocialclaw.com` service; Postiz lets you self-host the entire stack with no third-party dependency.

## Prerequisites

Either point `base_url` to the Postiz cloud (`https://api.postiz.com`, default) and grab an API key from your account, **or** run Postiz yourself with the [docker-compose setup](https://github.com/gitroomhq/postiz-app) and use your self-hosted URL.

After Postiz is running, you also need to connect at least one social account through its web UI (OAuth per provider) before the agent can post.

## Configuration

```toml
[postiz]
enabled = true
base_url = "https://api.postiz.com"          # or "https://postiz.yourdomain.com" for self-hosted
api_key = "..."                              # paste from Postiz settings
allowed_providers = ["*"]                    # or ["x", "linkedin", "bluesky", ...]
timeout_secs = 60
max_upload_bytes = 10485760
```

## Operations

### `list_integrations`

Discover which social accounts are connected and their integration IDs (which you'll reference in `create_post`).

```jsonc
{ "operation": "list_integrations" }
```

### `upload_media`

Upload a workspace-local file (image or video) to Postiz's media store. Returns the upload ID needed to attach the file to a post.

```jsonc
{ "operation": "upload_media", "path": "media/photo.jpg" }
```

The path must canonicalize inside `workspace_dir`; files outside are rejected.

### `create_post`

Send the full Postiz POST `/public/v1/posts` payload as `body`. The tool only enforces auth, provider allowlist, and rate limiting — it does **not** wrap the body schema, so refer to the [Postiz public API docs](https://docs.postiz.com/public-api) for the exact shape.

Minimal example:

```jsonc
{
  "operation": "create_post",
  "body": {
    "type": "now",
    "posts": [{
      "integration": { "id": "<integration-id-from-list_integrations>" },
      "value": [{ "content": "Hello from ZeroClaw" }]
    }]
  }
}
```

## Security

- **API key** is a `#[secret]` config field, kept in the OS keyring; never logged.
- **Provider allowlist** is enforced by walking the body for any `provider`, `providerName`, or `providerIdentifier` field — wildcards (`["*"]`) accept anything. Case-insensitive match.
- **Upload size** is hard-capped by `max_upload_bytes`; large files are rejected before the request.
- **Workspace sandbox** — `upload_media` canonicalizes the path against `workspace_dir`; paths that escape (`../`, symlinks, absolute paths outside workspace) are rejected.
- **Budget** — every call consumes one slot in `SecurityPolicy.record_action()`. Read-only autonomy blocks every operation.
- Postiz cloud enforces its own rate limit (90/h, 100/h for cloud paid). The tool surfaces 429 responses directly so the agent can back off.

## Operational notes

- **Self-hosted Postiz** needs the `BACKEND_URL` env var configured correctly in its docker-compose, otherwise OAuth callbacks break. Test the dashboard works before pointing the agent at it.
- The `Authorization` header carries the raw key or token (no `Bearer` prefix) — this matches Postiz's convention but differs from most REST APIs; the tool sets it correctly.
- For complex schedules with multiple platforms in one call, fetch `list_integrations` first to map provider names → IDs, then construct the `posts` array in `create_post`.
