// Push-to-talk voice helpers for the Jarvis page.
//
// Two recording paths are supported:
//
//   1. `browser` — Web Speech API (`SpeechRecognition`). Free, no key,
//      no audio leaves the browser. Limited to Chrome/Edge/Safari and
//      varies in quality by language.
//
//   2. `groq` / `openai` — capture audio with MediaRecorder and POST
//      the blob to an OpenAI-compatible `/v1/audio/transcriptions`
//      endpoint. Same wire format as Whisper, so adding more
//      compatible providers later is a one-line config change.
//
// Speech synthesis always uses the browser's built-in SpeechSynthesis
// API; no remote TTS is ever called from this hook.

import { useCallback, useEffect, useRef, useState } from 'react';
import {
  getPreferredVoiceLocale,
  getSttApiKey,
  getSttEndpoint,
  getSttModel,
  getSttProvider,
  type SttProvider,
} from '../lib/jarvisSettings';

type VoiceMode = 'idle' | 'listening' | 'speaking' | 'thinking';

interface UseVoiceResult {
  mode: VoiceMode;
  audioLevel: number;
  error: string | null;
  startRecording: () => Promise<void>;
  stopRecording: () => Promise<string | null>;
  speak: (text: string, lang?: string) => Promise<void>;
  cancelSpeech: () => void;
  setMode: (m: VoiceMode) => void;
}

interface SpeechRecognitionLike {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  start(): void;
  stop(): void;
  abort(): void;
  onresult:
    | ((ev: { results: ArrayLike<{ 0: { transcript: string }; isFinal: boolean }> }) => void)
    | null;
  onerror: ((ev: { error: string }) => void) | null;
  onend: (() => void) | null;
}

interface SpeechRecognitionCtor {
  new (): SpeechRecognitionLike;
}

function getSpeechRecognitionCtor(): SpeechRecognitionCtor | null {
  const w = window as unknown as {
    SpeechRecognition?: SpeechRecognitionCtor;
    webkitSpeechRecognition?: SpeechRecognitionCtor;
  };
  return w.SpeechRecognition ?? w.webkitSpeechRecognition ?? null;
}

async function transcribeRemote(
  blob: Blob,
  provider: Exclude<SttProvider, 'browser'>,
  language: string,
): Promise<string | null> {
  const apiKey = getSttApiKey(provider);
  if (!apiKey) {
    throw new Error(`${provider} API key not configured — open settings to add one.`);
  }
  const endpoint = getSttEndpoint(provider);
  const model = getSttModel(provider);

  const form = new FormData();
  form.append('file', blob, 'speech.webm');
  form.append('model', model);
  if (language) form.append('language', language);
  form.append('response_format', 'json');

  const res = await fetch(endpoint, {
    method: 'POST',
    headers: { Authorization: `Bearer ${apiKey}` },
    body: form,
  });

  if (!res.ok) {
    const detail = await res.text().catch(() => '');
    throw new Error(`${provider} STT error ${res.status}: ${detail || res.statusText}`);
  }
  const data = (await res.json()) as { text?: string };
  return (data.text ?? '').trim() || null;
}

/**
 * Pick the best available voice for a target locale.
 *
 * Selection order:
 *   1. Local/native voice with exact `lang` match — these are bundled
 *      with the OS, sound the most natural, and have the lowest
 *      latency (no network).
 *   2. Local/native voice with the same language prefix (e.g. `es-MX`
 *      when the user asked for `es-ES`).
 *   3. Any voice (remote/cloud included) with exact match.
 *   4. Any voice with the same language prefix.
 *   5. First voice in the catalogue, as a last resort.
 *
 * `localService` is true for OS-bundled voices and false for remote
 * (Google's "Network synthesis", Microsoft Edge's online voices, …).
 * Local voices are usually higher quality on Mac/iOS and on Windows
 * (SAPI5 / Edge premium); the cloud fallback only matters on Linux
 * where bundled voices are scarce.
 */
function pickVoice(lang: string): SpeechSynthesisVoice | undefined {
  const voices = window.speechSynthesis?.getVoices() ?? [];
  if (voices.length === 0) return undefined;
  const langPrefix = lang.split('-')[0] ?? lang;

  const exactLocal = voices.find((v) => v.lang === lang && v.localService);
  if (exactLocal) return exactLocal;

  const prefixLocal = voices.find(
    (v) => v.lang.startsWith(langPrefix) && v.localService,
  );
  if (prefixLocal) return prefixLocal;

  const exact = voices.find((v) => v.lang === lang);
  if (exact) return exact;

  return voices.find((v) => v.lang.startsWith(langPrefix)) ?? voices[0];
}

/**
 * Strip markdown, code fences, and other characters that the agent
 * model uses for written formatting but that
 * `SpeechSynthesis` would otherwise read aloud literally ("hash hash
 * sub-agent" instead of pausing past the heading).
 *
 * The transformation is intentionally conservative — we never drop
 * letters or numbers, only formatting punctuation and control chars.
 * The goal is "the same paragraph a human would read aloud", not a
 * normalized search string.
 */
export function cleanForSpeech(raw: string): string {
  if (!raw) return '';
  let text = raw;

  // Collapse fenced code blocks: keep the inner code, drop the fences.
  // The model rarely puts essential prose inside fences, but if it does
  // we keep the words for the synth to read.
  text = text.replace(/```[a-zA-Z0-9_-]*\n?/g, ' ').replace(/```/g, ' ');

  // Inline code: keep the symbol, drop the back-ticks.
  text = text.replace(/`([^`]*)`/g, '$1');

  // Markdown images: read the alt text only — the URL is noise on TTS.
  text = text.replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1');

  // Markdown links: keep the visible label, drop the URL.
  text = text.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '$1');

  // ATX headings (`#`, `##`, `###` …) at the start of a line.
  text = text.replace(/^[ \t]*#{1,6}[ \t]+/gm, '');

  // Setext headings — the underline rows of `===` / `---`.
  text = text.replace(/^[ \t]*[=\-]{3,}[ \t]*$/gm, '');

  // Block-level list markers at the start of a line: `-`, `*`, `+`,
  // and ordered `1.` / `1)` items.
  text = text.replace(/^[ \t]*([*\-+]|\d+[.)])[ \t]+/gm, '');

  // Block-quote angle brackets at the start of a line.
  text = text.replace(/^[ \t]*>+[ \t]?/gm, '');

  // Pipe-table dividers (`| --- | --- |`).
  text = text.replace(/^\s*\|?(\s*:?-+:?\s*\|)+\s*$/gm, '');

  // Pipe-table column separators — keep the cell text, drop the bars.
  text = text.replace(/[ \t]*\|[ \t]*/g, ', ');

  // Bold / italic / strike markers: `**word**`, `*word*`, `_word_`,
  // `~~word~~`. The character outside the regex stays.
  text = text.replace(/(\*\*|__)(.*?)\1/g, '$2');
  text = text.replace(/(\*|_)(?!\s)([^*_]+?)(?<!\s)\1/g, '$2');
  text = text.replace(/~~(.*?)~~/g, '$1');

  // Bare URLs: skip them. `https://example.com/path?x=1` read aloud
  // is unusable; the agent has the link in the on-screen transcript
  // anyway.
  text = text.replace(/https?:\/\/[^\s)]+/g, '');

  // Emoji and other symbol pictographs — keep ASCII letters and CJK,
  // drop the rest. Most TTS voices say "smiling face emoji" out loud
  // for `:-)` / `🙂`, which is exactly the behaviour the user
  // complained about.
  text = text.replace(/[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/gu, '');

  // Stray HTML tags the model occasionally inserts.
  text = text.replace(/<\/?[a-zA-Z][^>]*>/g, '');

  // Hash / at / dollar / caret / tilde used as decorators rather than
  // as part of a word. We drop them only when surrounded by
  // whitespace or punctuation so identifiers like `H2O` and prices
  // like `$5` survive.
  text = text.replace(/(^|\s)[#@^~](\s|$)/g, '$1$2');

  // Collapse repeated whitespace introduced by all the above.
  text = text.replace(/[ \t]+/g, ' ').replace(/\n{3,}/g, '\n\n').trim();

  return text;
}

export function useVoice(): UseVoiceResult {
  const [mode, setMode] = useState<VoiceMode>('idle');
  const [audioLevel, setAudioLevel] = useState(0);
  const [error, setError] = useState<string | null>(null);

  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const streamRef = useRef<MediaStream | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const rafRef = useRef<number | null>(null);

  const recognitionRef = useRef<SpeechRecognitionLike | null>(null);
  const browserTranscriptRef = useRef<string>('');
  const providerRef = useRef<SttProvider>('browser');

  const stopMeter = useCallback(() => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
    if (audioContextRef.current) {
      audioContextRef.current.close().catch(() => undefined);
      audioContextRef.current = null;
    }
    analyserRef.current = null;
    setAudioLevel(0);
  }, []);

  const startMeter = useCallback((stream: MediaStream) => {
    const ctx = new AudioContext();
    const source = ctx.createMediaStreamSource(stream);
    const analyser = ctx.createAnalyser();
    analyser.fftSize = 1024;
    source.connect(analyser);
    audioContextRef.current = ctx;
    analyserRef.current = analyser;

    const buf = new Uint8Array(analyser.fftSize);
    const tick = () => {
      const a = analyserRef.current;
      if (!a) return;
      a.getByteTimeDomainData(buf);
      let sumSq = 0;
      for (let i = 0; i < buf.length; i++) {
        const v = (buf[i]! - 128) / 128;
        sumSq += v * v;
      }
      const rms = Math.sqrt(sumSq / buf.length);
      setAudioLevel(Math.min(1, rms * 3.2));
      rafRef.current = requestAnimationFrame(tick);
    };
    rafRef.current = requestAnimationFrame(tick);
  }, []);

  const startRecording = useCallback(async () => {
    setError(null);
    const provider = getSttProvider();
    providerRef.current = provider;

    if (provider === 'browser') {
      const Ctor = getSpeechRecognitionCtor();
      if (!Ctor) {
        const msg =
          'Web Speech API not supported in this browser. Switch STT provider to Groq or OpenAI in settings.';
        setError(msg);
        throw new Error(msg);
      }

      // SpeechRecognition manages the mic itself, but we open a parallel
      // MediaStream just to drive the orb meter.
      try {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        streamRef.current = stream;
        startMeter(stream);
      } catch {
        // Meter is best-effort; recognition can still run without it.
      }

      const recognition = new Ctor();
      recognition.lang = getPreferredVoiceLocale();
      recognition.continuous = true;
      recognition.interimResults = true;

      browserTranscriptRef.current = '';
      recognition.onresult = (ev) => {
        let finalText = '';
        for (let i = 0; i < ev.results.length; i++) {
          const r = ev.results[i];
          if (r && r.isFinal) finalText += r[0].transcript;
        }
        if (finalText) browserTranscriptRef.current = finalText.trim();
      };
      recognition.onerror = (ev) => {
        if (ev.error === 'no-speech' || ev.error === 'aborted') return;
        if (ev.error === 'network') {
          // Chrome's SpeechRecognition relays audio through Google servers
          // and fails on restricted networks. Point the user at the remote
          // provider option in settings instead of leaving them stuck.
          setError(
            'Browser STT requires Google connectivity. Open Settings (gear) and switch to Groq or OpenAI Whisper.',
          );
          return;
        }
        setError(`Recognition error: ${ev.error}`);
      };
      recognition.onend = () => {
        // No auto-restart — stopRecording reads the buffered transcript.
      };

      recognition.start();
      recognitionRef.current = recognition;
      setMode('listening');
      return;
    }

    // Remote provider path — record to a blob, upload on stop.
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      streamRef.current = stream;

      const recorder = new MediaRecorder(stream, { mimeType: 'audio/webm' });
      chunksRef.current = [];
      recorder.ondataavailable = (e) => {
        if (e.data.size > 0) chunksRef.current.push(e.data);
      };
      recorder.start();
      mediaRecorderRef.current = recorder;
      startMeter(stream);
      setMode('listening');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Microphone access denied');
      setMode('idle');
      throw err;
    }
  }, [startMeter]);

  const stopRecording = useCallback(async (): Promise<string | null> => {
    const provider = providerRef.current;
    const stream = streamRef.current;

    if (provider === 'browser') {
      const recognition = recognitionRef.current;
      if (!recognition) return null;
      try {
        recognition.stop();
      } catch {
        // already stopped
      }
      recognitionRef.current = null;
      stream?.getTracks().forEach((t) => t.stop());
      streamRef.current = null;
      stopMeter();
      // Give onresult a tick to flush any pending final segment.
      await new Promise((r) => setTimeout(r, 80));
      const text = browserTranscriptRef.current.trim();
      browserTranscriptRef.current = '';
      setMode('idle');
      return text || null;
    }

    const recorder = mediaRecorderRef.current;
    if (!recorder || recorder.state === 'inactive') return null;

    const stopped = new Promise<Blob>((resolve) => {
      recorder.onstop = () => {
        const blob = new Blob(chunksRef.current, { type: 'audio/webm' });
        resolve(blob);
      };
    });
    recorder.stop();
    const blob = await stopped;

    stream?.getTracks().forEach((t) => t.stop());
    streamRef.current = null;
    mediaRecorderRef.current = null;
    stopMeter();
    setMode('thinking');

    try {
      const localePref = getPreferredVoiceLocale();
      const lang = localePref.split('-')[0] ?? localePref;
      const text = await transcribeRemote(blob, provider, lang);
      setMode('idle');
      return text;
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Transcription failed');
      setMode('idle');
      return null;
    }
  }, [stopMeter]);

  const speak = useCallback(async (text: string, lang?: string): Promise<void> => {
    // Strip markdown / code fences / control chars before handing the
    // string to SpeechSynthesis, otherwise the synth reads symbols
    // ("hash hash designer sub-agent…") aloud.
    const speakable = cleanForSpeech(text);
    if (!speakable || !window.speechSynthesis) return;
    window.speechSynthesis.cancel();
    setMode('speaking');

    const utterance = new SpeechSynthesisUtterance(speakable);
    const targetLang = lang ?? getPreferredVoiceLocale();
    utterance.lang = targetLang;
    const voice = pickVoice(targetLang);
    if (voice) utterance.voice = voice;
    // Slightly slower than 1.0 reads more naturally on most native
    // voices — full speed sounds clipped on long sentences.
    utterance.rate = 0.95;
    utterance.pitch = 1.0;

    return new Promise((resolve) => {
      let frame = 0;
      let speaking = true;
      const start = performance.now();
      const tick = () => {
        if (!speaking) return;
        const elapsed = (performance.now() - start) / 1000;
        const phase = Math.min(1, elapsed / 0.4);
        const wobble = 0.55 + Math.sin(elapsed * 9) * 0.25;
        setAudioLevel(phase * wobble);
        frame = requestAnimationFrame(tick);
      };
      frame = requestAnimationFrame(tick);

      utterance.onend = () => {
        speaking = false;
        cancelAnimationFrame(frame);
        setAudioLevel(0);
        setMode('idle');
        resolve();
      };
      utterance.onerror = () => {
        speaking = false;
        cancelAnimationFrame(frame);
        setAudioLevel(0);
        setMode('idle');
        resolve();
      };
      window.speechSynthesis.speak(utterance);
    });
  }, []);

  const cancelSpeech = useCallback(() => {
    window.speechSynthesis?.cancel();
    setAudioLevel(0);
    setMode('idle');
  }, []);

  // Pre-warm the voice list on mount — Chrome populates it asynchronously.
  useEffect(() => {
    if (typeof window === 'undefined' || !window.speechSynthesis) return;
    window.speechSynthesis.getVoices();
    const onChange = () => window.speechSynthesis.getVoices();
    window.speechSynthesis.addEventListener('voiceschanged', onChange);
    return () => window.speechSynthesis.removeEventListener('voiceschanged', onChange);
  }, []);

  // Cleanup on unmount.
  useEffect(() => {
    return () => {
      mediaRecorderRef.current?.stop();
      streamRef.current?.getTracks().forEach((t) => t.stop());
      try {
        recognitionRef.current?.abort();
      } catch {
        // noop
      }
      stopMeter();
      window.speechSynthesis?.cancel();
    };
  }, [stopMeter]);

  return {
    mode,
    audioLevel,
    error,
    startRecording,
    stopRecording,
    speak,
    cancelSpeech,
    setMode,
  };
}
