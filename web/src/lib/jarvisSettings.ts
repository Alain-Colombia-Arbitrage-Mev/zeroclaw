// Jarvis-specific settings persisted to localStorage.
//
// The Jarvis voice page can be pointed at a non-default gateway (e.g.
// when running the web bundle outside of Tauri but talking to a remote
// daemon) and needs an OpenAI API key in the browser to call the
// Whisper transcription endpoint. Both values fall back gracefully:
//
// - Gateway URL falls back to `apiOrigin` from `basePath.ts`, then to
//   `http://127.0.0.1:42617`.
// - OpenAI key has no fallback; STT simply returns `null` when missing.

import { apiOrigin } from './basePath';

const GATEWAY_KEY = 'zeroclaw_jarvis_gateway';
const OPENAI_KEY = 'zeroclaw_openai_api_key';
const VOICE_KEY = 'zeroclaw_jarvis_voice';
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

/** OpenAI API key used for Whisper STT calls. Empty string when not set. */
export function getOpenAiKey(): string {
  return readLocal(OPENAI_KEY);
}

export function setOpenAiKey(key: string): void {
  writeLocal(OPENAI_KEY, key.trim());
}

/** Preferred SpeechSynthesis voice (locale tag, e.g. "es-ES" or "en-US"). */
export function getPreferredVoiceLocale(): string {
  return readLocal(VOICE_KEY) || 'es-ES';
}

export function setPreferredVoiceLocale(locale: string): void {
  writeLocal(VOICE_KEY, locale.trim());
}
