# WaveSpeed TTS

The `wavespeed_tts` tool generates speech audio via the [WaveSpeed.ai](https://wavespeed.ai) TTS catalog. It is the **audio counterpart** to `image_gen` — same vendor, same `WAVESPEED_API_KEY`, distinct model allowlist.

This is the recommended TTS path for ZeroClaw because:

- It avoids forcing a third API for voice (already use WaveSpeed for images).
- WaveSpeed hosts Minimax Speech 2.6 HD **serverless** (no dedicated endpoint cost like Together).
- Same POST+poll+download flow as `image_gen`, so the runtime cost of adding it is low.

## Models intended for this tool

| Model | Use case |
|---|---|
| `wavespeed-ai/qwen3-tts/text-to-speech` | Default; natural voice, ~97ms TTFA |
| `minimax/speech-2.6-hd` | Premium quality, multi-language |
| `wavespeed-ai/omnivoice/text-to-speech` | 600+ languages, zero-shot cloning |
| `wavespeed-ai/vibevoice` | Long-form / podcast / multi-speaker |

## Configuration

```toml
[wavespeed_tts]
enabled = true
api_key_env = "WAVESPEED_API_KEY"             # shared with image_gen
allowed_models = [
  "wavespeed-ai/qwen3-tts/text-to-speech",
  "minimax/speech-2.6-hd",
]
default_voice = "Vivian"                      # valid for Qwen3-TTS
default_language = "auto"
output_format = "mp3"                         # mp3 | wav | opus | flac
max_input_chars = 5000
request_timeout_secs = 60
total_timeout_secs = 300
max_download_bytes = 52428800                 # 50 MiB
```

The `api_key` field can be set directly (stored via OS keyring as a `#[secret]`); if unset, the tool reads from the env var named in `api_key_env` (defaults to `WAVESPEED_API_KEY`, same as `image_gen`).

## Invocation

```jsonc
{
  "text": "Hola, bienvenidos al podcast de hoy.",
  "model": "wavespeed-ai/qwen3-tts/text-to-speech",
  "voice": "Vivian",
  "language": "es",
  "output_path": "audio/intro.mp3"
}
```

For Qwen3-TTS, [available voices](https://wavespeed.ai/docs/docs-api/wavespeed-ai/qwen3-tts-text-to-speech) include: Vivian, Serena, Ono_Anna, Sohee, Uncle_Fu, Dylan, Eric, Ryan, Aiden. Other models have their own voice catalogs — refer to each model's page on wavespeed.ai.

The `extra` field is passed through to the WaveSpeed submit body verbatim. Useful for model-specific knobs:

```jsonc
{
  "text": "...",
  "model": "wavespeed-ai/qwen3-tts/text-to-speech",
  "extra": { "style_instruction": "warm, calm, slow pace" }
}
```

```jsonc
{
  "text": "Host A: Welcome.\nHost B: Thanks for having me.",
  "model": "wavespeed-ai/vibevoice",
  "extra": { "speakers": [{"id": "A", "voice": "..."}, {"id": "B", "voice": "..."}] }
}
```

`output_path` is optional; if omitted, the tool writes to `workspace_dir/wavespeed-tts-{timestamp}.<output_format>`.

Successful response:

```json
{
  "model": "wavespeed-ai/qwen3-tts/text-to-speech",
  "voice": "Vivian",
  "language": "es",
  "audio_url": "https://cdn.wavespeed.ai/.../audio.mp3",
  "output_path": "/workspace/audio/intro.mp3",
  "bytes_written": 234567,
  "text_chars": 42
}
```

## Security

- **Model allowlist** is hard-required. Empty allowlist returns a configuration error rather than allowing arbitrary spend.
- **API key**: `#[secret]` config field with env-var fallback. Never logged.
- **Output sandbox**: the resolved output path's parent must canonicalize inside `workspace_dir`. Anything outside is rejected before any HTTP call.
- **Input cap**: `max_input_chars` blocks runaway text — TTS bills per character, so this prevents accidental large bills.
- **Download size cap**: streaming download stops and deletes the partial file the moment it exceeds `max_download_bytes`.
- **Read-only autonomy** blocks all generation; **hourly action budget** enforced via `SecurityPolicy.record_action()`.

## Cost considerations

WaveSpeed TTS is **per-character**. Reference rates (verify on wavespeed.ai):

- Qwen3-TTS: $0.005 per 100 characters (≈ $50 per 1M chars)
- Minimax 2.6 HD: similar tier
- OmniVoice: similar tier

A 5,000-character podcast script is roughly $0.25. Set `max_input_chars` conservatively if your agent might generate long scripts unsupervised; combine with `SecurityPolicy.max_actions_per_hour` for an hourly spend ceiling.

## Operational notes

- First call to a cold model can take 10–20s extra; subsequent calls inside the same window are fast.
- `voice` defaults come from config — set `default_voice` to the most-used voice for your most-used model so the agent doesn't need to pass it every call.
- For long-form output (podcasts >5 min), prefer VibeVoice — it maintains speaker consistency where chunk-and-concatenate approaches drift.
- The `audio_url` returned by WaveSpeed is short-lived (typically 24h). Always download via the tool rather than passing the URL around for later.
