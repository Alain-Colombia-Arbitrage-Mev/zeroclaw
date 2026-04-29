// Push-to-talk voice helpers for the Jarvis page.
//
// `useVoice` exposes:
//   - startRecording / stopRecording: capture mic audio while the
//     push-to-talk button is held. Returns the transcript via Whisper
//     (or `null` if no key is configured / the request fails).
//   - speak: TTS via the browser's SpeechSynthesis API, picking the
//     best matching voice for the configured locale.
//   - audioLevel: a 0..1 RMS read of the current input or output stream
//     for the orb visualisation.
//
// Whisper is called directly from the browser using the user-supplied
// OpenAI key — the gateway is intentionally not proxied for this hop
// to keep the daemon out of the audio path. If you need server-side
// auditing of voice input, swap `transcribeWithWhisper` for a gateway
// endpoint that forwards to your provider of choice.

import { useCallback, useEffect, useRef, useState } from 'react';
import { getOpenAiKey, getPreferredVoiceLocale } from '../lib/jarvisSettings';

type VoiceMode = 'idle' | 'listening' | 'speaking' | 'thinking';

interface UseVoiceResult {
  mode: VoiceMode;
  audioLevel: number;
  error: string | null;
  /** Begin capturing mic audio; resolves when recording is active. */
  startRecording: () => Promise<void>;
  /** Stop capturing and return the Whisper transcript (or null on failure). */
  stopRecording: () => Promise<string | null>;
  /** Speak text via the browser TTS, in the given language. */
  speak: (text: string, lang?: string) => Promise<void>;
  /** Cancel any in-flight speech synthesis. */
  cancelSpeech: () => void;
  setMode: (m: VoiceMode) => void;
}

const WHISPER_ENDPOINT = 'https://api.openai.com/v1/audio/transcriptions';
const WHISPER_MODEL = 'whisper-1';

async function transcribeWithWhisper(
  blob: Blob,
  apiKey: string,
  language?: string,
): Promise<string | null> {
  const form = new FormData();
  form.append('file', blob, 'speech.webm');
  form.append('model', WHISPER_MODEL);
  if (language) form.append('language', language);
  form.append('response_format', 'json');

  const res = await fetch(WHISPER_ENDPOINT, {
    method: 'POST',
    headers: { Authorization: `Bearer ${apiKey}` },
    body: form,
  });

  if (!res.ok) {
    const detail = await res.text().catch(() => '');
    throw new Error(`Whisper error ${res.status}: ${detail || res.statusText}`);
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
      // Map 0..0.5 RMS to 0..1 with a soft ceiling.
      setAudioLevel(Math.min(1, rms * 3.2));
      rafRef.current = requestAnimationFrame(tick);
    };
    rafRef.current = requestAnimationFrame(tick);
  }, []);

  const startRecording = useCallback(async () => {
    setError(null);
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
    const recorder = mediaRecorderRef.current;
    const stream = streamRef.current;
    if (!recorder || recorder.state === 'inactive') {
      return null;
    }

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

    const apiKey = getOpenAiKey();
    if (!apiKey) {
      setError('OpenAI API key not configured — open settings to add one.');
      setMode('idle');
      return null;
    }

    try {
      const localePref = getPreferredVoiceLocale();
      const lang = localePref.split('-')[0] ?? localePref;
      const text = await transcribeWithWhisper(blob, apiKey, lang);
      setMode('idle');
      return text;
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Whisper transcription failed');
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
      // Approximate audio envelope from elapsed time — there's no way
      // to read the synthesizer's amplitude on the web, so we ramp up
      // for a sentence and fade out at the end.
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
