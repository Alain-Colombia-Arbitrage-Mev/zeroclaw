// Jarvis voice page — push-to-talk orb wired to the agent WebSocket.
//
// Flow:
//   1. User holds the mic button → mic recording starts, orb turns cyan.
//   2. User releases → blob is sent to Whisper, orb turns amber while
//      transcribing.
//   3. Resulting transcript is forwarded to the agent over the existing
//      gateway WebSocket (`useWebSocket`).
//   4. When the agent emits a `done` frame with `full_response`, the
//      page speaks it via the browser's SpeechSynthesis. Orb turns
//      purple while speaking.
//
// Settings (gateway URL override, OpenAI key, voice locale) are
// persisted in localStorage by `lib/jarvisSettings.ts` and editable
// from the gear icon in the top-right.

import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft, Mic, Settings as SettingsIcon, X } from 'lucide-react';
import JarvisOrb from '../components/JarvisOrb';
import JarvisOrbSpline from '../components/JarvisOrbSpline';
import { useVoice } from '../hooks/useVoice';
import { useWebSocket } from '../hooks/useWebSocket';
import {
  getJarvisGateway,
  getPreferredVoiceLocale,
  getSplineSceneUrl,
  getSttApiKey,
  getSttEndpoint,
  getSttProvider,
  setJarvisGateway,
  setPreferredVoiceLocale,
  setSplineSceneUrl,
  setSttApiKey,
  setSttEndpoint,
  setSttProvider,
  STT_PROVIDER_DEFAULTS,
  SUPPORTED_STT_PROVIDERS,
  type SttProvider,
} from '../lib/jarvisSettings';
import { useLocale } from '../lib/i18n';

/** Fallback Spline scene used when the operator hasn't configured one yet. */
const DEFAULT_SPLINE_SCENE =
  'https://prod.spline.design/jXcoLrXgC8kt-PkP/scene.splinecode';

interface Exchange {
  user: string;
  assistant: string;
}

const SUPPORTED_VOICE_LOCALES = [
  { code: 'es-ES', label: 'Español (España)' },
  { code: 'es-419', label: 'Español (Latam)' },
  { code: 'en-US', label: 'English (US)' },
  { code: 'en-GB', label: 'English (UK)' },
];

export default function Jarvis() {
  const { t } = useLocale();
  const navigate = useNavigate();
  const voice = useVoice();
  const ws = useWebSocket({
    baseUrl: useMemo(() => getJarvisGateway().replace(/^http/, 'ws'), []),
  });

  const [exchanges, setExchanges] = useState<Exchange[]>([]);
  const [pendingUser, setPendingUser] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [activeProvider, setActiveProvider] = useState<SttProvider>(() => getSttProvider());

  // Refresh the displayed provider when the settings modal closes (in case
  // the user changed it).
  useEffect(() => {
    if (!showSettings) setActiveProvider(getSttProvider());
  }, [showSettings]);

  const providerLabel = SUPPORTED_STT_PROVIDERS.find((p) => p.value === activeProvider)?.label ?? activeProvider;

  // Track responses arriving over the WS so we can speak them when done.
  const lastSeenIndexRef = useRef(0);

  useEffect(() => {
    const messages = ws.messages.slice(lastSeenIndexRef.current);
    lastSeenIndexRef.current = ws.messages.length;
    for (const m of messages) {
      if (m.type === 'done' && m.full_response) {
        const reply = m.full_response.trim();
        setExchanges((prev) => {
          const next = [...prev];
          if (pendingUser) {
            next.push({ user: pendingUser, assistant: reply });
          } else if (next.length > 0) {
            const last = next[next.length - 1]!;
            next[next.length - 1] = { ...last, assistant: reply };
          }
          return next.slice(-6);
        });
        setPendingUser(null);
        voice.speak(reply, getPreferredVoiceLocale());
      } else if (m.type === 'error' && m.message) {
        setPendingUser(null);
        voice.setMode('idle');
      }
    }
    // We intentionally do not depend on `voice` (it's stable) or `pendingUser`
    // (read at event time) to avoid double-firing on every re-render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ws.messages]);

  const handlePressStart = async (e: React.PointerEvent | React.MouseEvent) => {
    e.preventDefault();
    if (voice.mode !== 'idle') return;
    try {
      await voice.startRecording();
    } catch {
      // useVoice already surfaced the error
    }
  };

  const handlePressEnd = async () => {
    if (voice.mode !== 'listening') return;
    const transcript = await voice.stopRecording();
    if (!transcript) return;
    setPendingUser(transcript);
    setExchanges((prev) => [...prev, { user: transcript, assistant: '' }].slice(-6));
    if (ws.status === 'connected') {
      try {
        ws.sendMessage(transcript);
      } catch {
        voice.setMode('idle');
      }
    } else {
      voice.setMode('idle');
    }
  };

  const statusLabel = (() => {
    if (voice.error) return voice.error;
    if (ws.status !== 'connected') return t('jarvis.status_disconnected');
    switch (voice.mode) {
      case 'listening': return t('jarvis.status_listening');
      case 'thinking': return t('jarvis.status_transcribing');
      case 'speaking': return t('jarvis.status_speaking');
      default: return t('jarvis.status_ready');
    }
  })();

  return (
    <div
      className="fixed inset-0 overflow-hidden"
      style={{
        background:
          'radial-gradient(ellipse at center, rgba(8, 12, 28, 1) 0%, rgba(4, 6, 14, 1) 60%, rgba(0, 0, 0, 1) 100%)',
      }}
    >
      {/* Fullscreen orb behind everything. Spline by default (the
          operator can override the scene URL or clear it from
          settings; clearing falls back to the built-in Three.js orb). */}
      <div className="absolute inset-0 z-0">
        <OrbStage audioLevel={voice.audioLevel} mode={voice.mode} />
      </div>

      <button
        onClick={() => navigate('/')}
        className="absolute top-4 left-4 z-30 flex items-center gap-2 px-3 py-2 rounded-lg hover:bg-white/10 backdrop-blur-sm border border-white/10"
        aria-label="Back to dashboard"
        title="Back to dashboard"
        style={{ background: 'rgba(0,0,0,0.35)' }}
      >
        <ArrowLeft className="h-4 w-4" style={{ color: 'rgba(255, 255, 255, 0.85)' }} />
        <span className="text-xs tracking-[0.2em] uppercase" style={{ color: 'rgba(255, 255, 255, 0.85)' }}>
          Dashboard
        </span>
      </button>

      <button
        onClick={() => setShowSettings(true)}
        className="absolute top-4 right-4 z-30 p-2 rounded-lg hover:bg-white/10 backdrop-blur-sm border border-white/10"
        aria-label={t('jarvis.settings_open')}
        title={t('jarvis.settings_open')}
        style={{ background: 'rgba(0,0,0,0.35)' }}
      >
        <SettingsIcon className="h-5 w-5" style={{ color: 'rgba(255, 255, 255, 0.9)' }} />
      </button>

      <div
        className="absolute top-6 left-1/2 -translate-x-1/2 z-20 flex flex-col items-center gap-2 pointer-events-none"
        aria-live="polite"
      >
        <div
          className="px-4 py-2 rounded-full backdrop-blur-md border border-white/10"
          style={{ background: 'rgba(0,0,0,0.55)' }}
        >
          <p
            className="text-sm tracking-[0.3em] uppercase font-semibold"
            style={{
              color: 'rgba(255, 255, 255, 0.95)',
              textShadow: '0 1px 6px rgba(0, 0, 0, 0.9)',
            }}
          >
            {statusLabel}
          </p>
        </div>
        <div
          className="px-3 py-1 rounded-full backdrop-blur-md border border-white/10"
          style={{ background: 'rgba(0,0,0,0.45)' }}
        >
          <p
            className="text-[10px] tracking-[0.2em] uppercase"
            style={{ color: activeProvider === 'browser' ? '#fca5a5' : '#86efac' }}
          >
            STT: {providerLabel}
          </p>
        </div>
      </div>

      <div className="absolute left-1/2 -translate-x-1/2 bottom-12 z-20 flex flex-col items-center gap-4 w-full max-w-2xl px-4">
        <button
          onPointerDown={handlePressStart}
          onPointerUp={handlePressEnd}
          onPointerCancel={handlePressEnd}
          onPointerLeave={voice.mode === 'listening' ? handlePressEnd : undefined}
          disabled={ws.status !== 'connected' || voice.mode === 'thinking' || voice.mode === 'speaking'}
          className="btn-electric flex items-center gap-3 px-6 py-4 text-base font-semibold tracking-wide shadow-2xl"
          style={{ boxShadow: '0 8px 32px rgba(0, 0, 0, 0.6)' }}
        >
          <Mic className="h-5 w-5" />
          {voice.mode === 'listening' ? t('jarvis.release_to_send') : t('jarvis.hold_to_talk')}
        </button>

        {exchanges.length > 0 && (
          <div
            className="w-full mt-4 space-y-3 p-4 rounded-2xl backdrop-blur-md border border-white/10"
            style={{ background: 'rgba(0,0,0,0.55)' }}
          >
            {exchanges.slice(-3).map((ex, i) => (
              <div
                key={i}
                className="p-3 rounded-lg border border-white/10"
                style={{ background: 'rgba(0,0,0,0.4)' }}
              >
                <p className="text-xs font-semibold uppercase tracking-wider mb-1" style={{ color: '#7dd3fc' }}>
                  {t('jarvis.you')}
                </p>
                <p className="mb-2 text-sm" style={{ color: 'rgba(255,255,255,0.95)' }}>{ex.user}</p>
                {ex.assistant && (
                  <>
                    <p className="text-xs font-semibold uppercase tracking-wider mb-1 mt-2" style={{ color: '#a78bfa' }}>
                      Jarvis
                    </p>
                    <p className="text-sm" style={{ color: 'rgba(255,255,255,0.95)' }}>{ex.assistant}</p>
                  </>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {showSettings && <JarvisSettingsModal onClose={() => setShowSettings(false)} />}
    </div>
  );
}

/**
 * Picks Spline vs the built-in Three.js orb.
 *
 * - Default: render the bundled Spline scene (DEFAULT_SPLINE_SCENE).
 * - Operator can override the URL from settings; clearing it falls back
 *   to the built-in Three.js orb.
 * - If the Spline runtime fails at load time, we wipe the bad URL and
 *   fall back to Three.js so the page never gets stuck on a black canvas.
 */
function OrbStage({
  audioLevel,
  mode,
}: {
  audioLevel: number;
  mode: 'idle' | 'listening' | 'speaking' | 'thinking';
}) {
  const stored = getSplineSceneUrl();
  const initialUrl = stored || DEFAULT_SPLINE_SCENE;
  const isExportedScene = (url: string): boolean => {
    if (!url) return false;
    try {
      const u = new URL(url);
      return (
        u.hostname.endsWith('spline.design') &&
        !u.hostname.startsWith('app.') &&
        url.endsWith('.splinecode')
      );
    } catch {
      return false;
    }
  };
  const [splineFailed, setSplineFailed] = useState(false);
  const useSpline = isExportedScene(initialUrl) && !splineFailed;

  if (useSpline) {
    return (
      <JarvisOrbSpline
        sceneUrl={initialUrl}
        audioLevel={audioLevel}
        mode={mode}
        onError={() => {
          setSplineSceneUrl('');
          setSplineFailed(true);
        }}
      />
    );
  }
  return <JarvisOrb audioLevel={audioLevel} mode={mode} />;
}

function JarvisSettingsModal({ onClose }: { onClose: () => void }) {
  const { t } = useLocale();
  const [gateway, setGateway] = useState(() => getJarvisGateway());
  const [provider, setProvider] = useState<SttProvider>(() => getSttProvider());
  const [apiKey, setApiKey] = useState(() => getSttApiKey(getSttProvider()));
  const [endpoint, setEndpoint] = useState(() => getSttEndpoint(getSttProvider()));
  const [voiceLocale, setVoiceLocale] = useState(() => getPreferredVoiceLocale());

  // When the user picks a different provider, swap key + endpoint to that
  // provider's stored values so each one keeps its own credentials.
  const handleProviderChange = (next: SttProvider) => {
    // Persist the in-progress entries for the *current* provider before switching.
    if (provider !== 'browser') {
      setSttApiKey(provider, apiKey);
      setSttEndpoint(provider, endpoint);
    }
    setProvider(next);
    setApiKey(getSttApiKey(next));
    setEndpoint(getSttEndpoint(next));
  };

  const providerMeta = SUPPORTED_STT_PROVIDERS.find((p) => p.value === provider);
  const needsKey = providerMeta?.needsKey ?? false;
  const defaultEndpoint =
    provider !== 'browser' ? STT_PROVIDER_DEFAULTS[provider].endpoint : '';

  const handleSave = () => {
    setJarvisGateway(gateway);
    setSttProvider(provider);
    if (provider !== 'browser') {
      setSttApiKey(provider, apiKey);
      setSttEndpoint(provider, endpoint);
    }
    setPreferredVoiceLocale(voiceLocale);
    onClose();
    // Reload so the new gateway URL is used by the WebSocket client.
    window.location.reload();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      style={{ background: 'rgba(0,0,0,0.6)' }}
      onClick={onClose}
    >
      <div
        className="surface-panel p-6 w-full max-w-md"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold" style={{ color: 'var(--pc-text-primary)' }}>
            {t('jarvis.settings_title')}
          </h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-white/5" aria-label="Close">
            <X className="h-4 w-4" style={{ color: 'var(--pc-text-muted)' }} />
          </button>
        </div>

        <label className="block mb-3">
          <span className="text-xs uppercase tracking-wider" style={{ color: 'var(--pc-text-muted)' }}>
            {t('jarvis.settings_gateway')}
          </span>
          <input
            type="text"
            value={gateway}
            onChange={(e) => setGateway(e.target.value)}
            placeholder="http://127.0.0.1:42617"
            className="input-electric w-full px-3 py-2 mt-1 text-sm font-mono"
          />
        </label>

        <label className="block mb-3">
          <span className="text-xs uppercase tracking-wider" style={{ color: 'var(--pc-text-muted)' }}>
            {t('jarvis.settings_stt_provider')}
          </span>
          <select
            value={provider}
            onChange={(e) => handleProviderChange(e.target.value as SttProvider)}
            className="input-electric w-full px-3 py-2 mt-1 text-sm"
          >
            {SUPPORTED_STT_PROVIDERS.map((p) => (
              <option key={p.value} value={p.value}>{p.label}</option>
            ))}
          </select>
        </label>

        {needsKey && (
          <>
            <label className="block mb-3">
              <span className="text-xs uppercase tracking-wider" style={{ color: 'var(--pc-text-muted)' }}>
                {t('jarvis.settings_stt_key')}
              </span>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder={provider === 'groq' ? 'gsk_...' : 'sk-...'}
                className="input-electric w-full px-3 py-2 mt-1 text-sm font-mono"
                autoComplete="off"
              />
              <span className="text-xs block mt-1" style={{ color: 'var(--pc-text-muted)' }}>
                {t('jarvis.settings_stt_key_hint')}
              </span>
            </label>

            <label className="block mb-3">
              <span className="text-xs uppercase tracking-wider" style={{ color: 'var(--pc-text-muted)' }}>
                {t('jarvis.settings_stt_endpoint')}
              </span>
              <input
                type="text"
                value={endpoint}
                onChange={(e) => setEndpoint(e.target.value)}
                placeholder={defaultEndpoint}
                className="input-electric w-full px-3 py-2 mt-1 text-sm font-mono"
              />
              <span className="text-xs block mt-1" style={{ color: 'var(--pc-text-muted)' }}>
                {t('jarvis.settings_stt_endpoint_hint')}
              </span>
            </label>
          </>
        )}

        <label className="block mb-4">
          <span className="text-xs uppercase tracking-wider" style={{ color: 'var(--pc-text-muted)' }}>
            {t('jarvis.settings_voice_locale')}
          </span>
          <select
            value={voiceLocale}
            onChange={(e) => setVoiceLocale(e.target.value)}
            className="input-electric w-full px-3 py-2 mt-1 text-sm"
          >
            {SUPPORTED_VOICE_LOCALES.map((v) => (
              <option key={v.code} value={v.code}>{v.label}</option>
            ))}
          </select>
        </label>

        <div className="flex gap-2 justify-end">
          <button onClick={onClose} className="px-4 py-2 text-sm">
            {t('jarvis.settings_cancel')}
          </button>
          <button onClick={handleSave} className="btn-electric px-4 py-2 text-sm font-medium">
            {t('jarvis.settings_save')}
          </button>
        </div>
      </div>
    </div>
  );
}
