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

function pickVoice(lang: string): SpeechSynthesisVoice | undefined {
  const voices = window.speechSynthesis?.getVoices() ?? [];
  if (voices.length === 0) return undefined;
  const exact = voices.find((v) => v.lang === lang);
  if (exact) return exact;
  const langPrefix = lang.split('-')[0] ?? lang;
  return voices.find((v) => v.lang.startsWith(langPrefix)) ?? voices[0];
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
        if (ev.error !== 'no-speech' && ev.error !== 'aborted') {
          setError(`Recognition error: ${ev.error}`);
        }
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
    if (!text.trim() || !window.speechSynthesis) return;
    window.speechSynthesis.cancel();
    setMode('speaking');

    const utterance = new SpeechSynthesisUtterance(text);
    const targetLang = lang ?? getPreferredVoiceLocale();
    utterance.lang = targetLang;
    const voice = pickVoice(targetLang);
    if (voice) utterance.voice = voice;
    utterance.rate = 1.0;
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
