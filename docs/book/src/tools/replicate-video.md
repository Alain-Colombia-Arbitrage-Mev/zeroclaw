# Replicate video

The `replicate_video` tool generates video via the [Replicate](https://replicate.com) HTTP API. Replicate hosts hundreds of generative video models (Veo, Kling, Luma Dream Machine, Pika, Stable Video Diffusion, Hunyuan, etc.) behind a single REST surface.

This is the **video counterpart** to the existing `image_gen` tool (WaveSpeed). Use `replicate_video` when the agent needs short-form clips for Reels/Shorts/TikTok or hero videos for landing pages.

## Prerequisites

1. Sign up at <https://replicate.com> and create an API token.
2. Pick the models you want the agent to use and add them to `allowed_models`. Replicate hosts thousands of models — leaving the allowlist open is discouraged.

Common video models on Replicate (verify availability and pricing before adding):

| Model | Use case |
|---|---|
| `google/veo-3.1` | Cinematic short clips, high fidelity |
| `kwaivgi/kling-v2.5-master` | Fast iterations, strong motion |
| `lumalabs/luma-dream-machine` | Stylized motion, image-to-video |
| `pikalabs/pika-1.5` | Short loops, motion brushes |
| `stability-ai/stable-video-diffusion` | Open-source, cheap |

## Configuration

```toml
[replicate_video]
enabled = true
api_token = "r8_..."                       # stored via OS keyring
allowed_models = [
  "google/veo-3.1",
  "kwaivgi/kling-v2.5-master",
]
request_timeout_secs = 60                  # per HTTP request
total_timeout_secs = 600                   # end-to-end (submit → poll → download)
max_download_bytes = 209715200             # 200 MiB
```

If `api_token` is omitted, the tool falls back to the `REPLICATE_API_TOKEN` environment variable.

## Invocation

```jsonc
{
  "model": "google/veo-3.1",
  "input": {
    "prompt": "A surfer riding a wave at sunset, cinematic 4k",
    "duration_seconds": 8,
    "aspect_ratio": "9:16"
  },
  "output_path": "media/surfer.mp4"
}
```

The `input` object is forwarded **verbatim** to Replicate — refer to each model's page (e.g. <https://replicate.com/google/veo-3.1>) for its specific input schema. The tool does not validate input fields.

`output_path` is optional; if omitted, a timestamped `.mp4` filename is generated under `workspace_dir`. Either way the parent directory must exist and resolve inside the workspace.

Successful response shape:

```json
{
  "prediction_id": "abc123",
  "model": "google/veo-3.1",
  "video_url": "https://r2.replicate.delivery/...mp4",
  "output_path": "/workspace/media/surfer.mp4",
  "bytes_written": 12345678
}
```

## Security

- **Model allowlist** is hard-required. Empty allowlist returns a clear configuration error rather than allowing arbitrary spend.
- **API token** is a `#[secret]` config field, stored in the OS keyring (or falls back to `REPLICATE_API_TOKEN` env var). Never logged.
- **Output sandbox** — the resolved output path's parent directory must canonicalize inside `workspace_dir`. Anything outside is rejected before any HTTP call.
- **Download size cap** — streaming download stops and deletes the partial file the moment it exceeds `max_download_bytes` (prevents disk-fill attacks).
- **Total timeout** kills the prediction lifecycle (submit → poll → download) at `total_timeout_secs`.
- **Budget** — every call consumes one slot in `SecurityPolicy.record_action()`. Read-only autonomy blocks every operation.

## Cost considerations

Replicate charges per second of compute. Video models are **expensive** — a 5-second Veo 3.1 clip can run $0.30–$1.00; a 30-second Kling clip in the $0.50 range. Rate-limit the agent aggressively in `SecurityPolicy.max_actions_per_hour` if you don't want bills.

## Operational notes

- The first call after enabling the tool may be slow — Replicate cold-starts the model on a fresh GPU.
- The polling loop runs every 2 seconds. Most video models complete in 30–120 seconds; tune `total_timeout_secs` upward for longer clips.
- If a prediction returns `output` in an unusual shape (model-specific), the tool walks string / array / object outputs to find the first MP4 URL. Models returning purely non-MP4 output (e.g. image previews) will surface "no video URL in `output`" as a clear error.
