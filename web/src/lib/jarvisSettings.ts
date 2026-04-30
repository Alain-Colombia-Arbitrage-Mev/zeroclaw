// Jarvis-specific settings persisted to localStorage.
//
// The Jarvis voice page can be pointed at a non-default gateway and at
// any STT (speech-to-text) provider that exposes an OpenAI-compatible
// audio transcriptions endpoint, plus the in-browser Web Speech API.
//
// Falls back gracefully:
//   - Gateway URL falls back to `apiOrigin` from `basePath.ts`, then to
//     `http://127.0.0.1:42617`.
//   - STT provider defaults to `browser` (no key required).
//   - Per-provider API keys have no fallback; STT just returns null.

import { apiOrigin } from './basePath';

export type SttProvider = 'browser' | 'groq' | 'openai';

export const SUPPORTED_STT_PROVIDERS: { value: SttProvider; label: string; needsKey: boolean }[] = [
  { value: 'browser', label: 'Browser (Web Speech API)', needsKey: false },
  { value: 'groq', label: 'Groq (whisper-large-v3)', needsKey: true },
  { value: 'openai', label: 'OpenAI (whisper-1)', needsKey: true },
];

interface ProviderConfig {
  endpoint: string;
  model: string;
}

export const STT_PROVIDER_DEFAULTS: Record<Exclude<SttProvider, 'browser'>, ProviderConfig> = {
  groq: {
    endpoint: 'https://api.groq.com/openai/v1/audio/transcriptions',
    model: 'whisper-large-v3',
  },
  openai: {
    endpoint: 'https://api.openai.com/v1/audio/transcriptions',
    model: 'whisper-1',
  },
};

const GATEWAY_KEY = 'zeroclaw_jarvis_gateway';
const VOICE_KEY = 'zeroclaw_jarvis_voice';
const PROVIDER_KEY = 'zeroclaw_jarvis_stt_provider';
const STT_KEY_PREFIX = 'zeroclaw_jarvis_stt_key_';
const STT_URL_PREFIX = 'zeroclaw_jarvis_stt_url_';
const LEGACY_OPENAI_KEY = 'zeroclaw_openai_api_key';
const DEFAULT_GATEWAY = 'http://127.0.0.1:42617';

function readLocal(key: string): string {
  try {
    return (localStorage.getItem(key) ?? '').trim();
  } catch {
    return '';
  }
}

function writeLocal(key: string, value: string): void {
  try {
    if (value) localStorage.setItem(key, value);
    else localStorage.removeItem(key);
  } catch {
    // localStorage may be unavailable (private mode, sandboxed iframe);
    // settings reset to defaults next reload.
  }
}

/** Effective gateway base URL for the Jarvis page (no trailing slash). */
export function getJarvisGateway(): string {
  const override = readLocal(GATEWAY_KEY);
  if (override) return override.replace(/\/+$/, '');
  if (apiOrigin) return apiOrigin.replace(/\/+$/, '');
  return DEFAULT_GATEWAY;
}

export function setJarvisGateway(url: string): void {
  writeLocal(GATEWAY_KEY, url.trim());
}

/** Active STT provider. Defaults to `browser` (no key required). */
export function getSttProvider(): SttProvider {
  const v = readLocal(PROVIDER_KEY);
  return v === 'groq' || v === 'openai' || v === 'browser' ? v : 'browser';
}

export function setSttProvider(p: SttProvider): void {
  writeLocal(PROVIDER_KEY, p);
}

/** API key for a remote STT provider. Empty string for `browser`. */
export function getSttApiKey(provider: SttProvider): string {
  if (provider === 'browser') return '';
  const direct = readLocal(STT_KEY_PREFIX + provider);
  if (direct) return direct;
  // One-shot migration from the previous OpenAI-only key.
  if (provider === 'openai') {
    const legacy = readLocal(LEGACY_OPENAI_KEY);
    if (legacy) {
      writeLocal(STT_KEY_PREFIX + provider, legacy);
      writeLocal(LEGACY_OPENAI_KEY, '');
      return legacy;
    }
  }
  return '';
}

export function setSttApiKey(provider: SttProvider, key: string): void {
  if (provider === 'browser') return;
  writeLocal(STT_KEY_PREFIX + provider, key.trim());
}

/** Endpoint URL override for a remote provider (defaults applied otherwise). */
export function getSttEndpoint(provider: SttProvider): string {
  if (provider === 'browser') return '';
  const override = readLocal(STT_URL_PREFIX + provider);
  if (override) return override;
  return STT_PROVIDER_DEFAULTS[provider].endpoint;
}

export function setSttEndpoint(provider: SttProvider, url: string): void {
  if (provider === 'browser') return;
  // Persist only when it differs from the default — otherwise wipe so
  // future default changes propagate automatically.
  const trimmed = url.trim();
  if (!trimmed || trimmed === STT_PROVIDER_DEFAULTS[provider].endpoint) {
    writeLocal(STT_URL_PREFIX + provider, '');
  } else {
    writeLocal(STT_URL_PREFIX + provider, trimmed);
  }
}

export function getSttModel(provider: SttProvider): string {
  if (provider === 'browser') return '';
  return STT_PROVIDER_DEFAULTS[provider].model;
}

/** Preferred SpeechSynthesis voice (locale tag, e.g. "es-ES" or "en-US"). */
export function getPreferredVoiceLocale(): string {
  return readLocal(VOICE_KEY) || 'es-ES';
}

export function setPreferredVoiceLocale(locale: string): void {
  writeLocal(VOICE_KEY, locale.trim());
}

// ────────────────────────────────────────────────────────────────────
// Backward compat: legacy callers continue to compile.
// New code should use getSttApiKey('openai') / setSttApiKey('openai', _).
// ────────────────────────────────────────────────────────────────────

/** @deprecated use getSttApiKey('openai') */
export function getOpenAiKey(): string {
  return getSttApiKey('openai');
}
/** @deprecated use setSttApiKey('openai', key) */
export function setOpenAiKey(key: string): void {
  setSttApiKey('openai', key);
}
