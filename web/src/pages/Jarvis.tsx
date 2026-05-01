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

  // Premium chrome — neutral charcoal panels with hairline borders. Single
  // accent (cyan) used sparingly for state feedback. Avoid bright color
  // floods so the Spline orb stays the visual centerpiece.
  const PANEL_BG = 'rgba(10, 10, 14, 0.72)';
  const PANEL_BG_SOFT = 'rgba(10, 10, 14, 0.55)';
  const HAIRLINE = '1px solid rgba(255, 255, 255, 0.08)';
  const TEXT_PRIMARY = 'rgba(244, 244, 245, 0.96)';
  const TEXT_SECONDARY = 'rgba(212, 212, 216, 0.72)';
  const TEXT_MUTED = 'rgba(161, 161, 170, 0.6)';
  const ACCENT = '#22d3ee';

  // State-driven dot color for the status indicator.
  const statusDotColor =
    voice.error ? '#f87171'
      : ws.status !== 'connected' ? '#fbbf24'
      : voice.mode === 'listening' ? ACCENT
      : voice.mode === 'speaking' ? '#a78bfa'
      : voice.mode === 'thinking' ? '#fbbf24'
      : 'rgba(244, 244, 245, 0.45)';

  const isMicBusy = ws.status !== 'connected' || voice.mode === 'thinking' || voice.mode === 'speaking';
  const isListening = voice.mode === 'listening';

  return (
    <div
      className="fixed inset-0 overflow-hidden notranslate"
      translate="no"
      style={{
        background:
          'radial-gradient(ellipse at center, rgba(10, 10, 14, 1) 0%, rgba(4, 4, 6, 1) 70%, rgba(0, 0, 0, 1) 100%)',
      }}
    >
      {/* Fullscreen orb behind everything. */}
      <div className="absolute inset-0 z-0">
        <OrbStage audioLevel={voice.audioLevel} mode={voice.mode} />
      </div>

      {/* Subtle vignette so chrome stays legible over bright orb pixels. */}
      <div
        className="absolute inset-0 z-0 pointer-events-none"
        style={{
          background:
            'radial-gradient(ellipse at center, transparent 50%, rgba(0,0,0,0.45) 100%)',
        }}
      />

      <button
        onClick={() => navigate('/')}
        className="absolute top-5 left-5 z-30 group flex items-center gap-2.5 pl-3 pr-4 py-2 rounded-lg backdrop-blur-xl transition-all duration-200 hover:bg-white/[0.06]"
        aria-label="Back to dashboard"
        title="Back to dashboard"
        style={{ background: PANEL_BG, border: HAIRLINE }}
      >
        <ArrowLeft
          className="h-4 w-4 transition-transform duration-200 group-hover:-translate-x-0.5"
          style={{ color: TEXT_SECONDARY }}
        />
        <span
          className="text-[11px] tracking-[0.18em] uppercase font-medium"
          style={{ color: TEXT_PRIMARY }}
        >
          Dashboard
        </span>
      </button>

      <button
        onClick={() => setShowSettings(true)}
        className="absolute top-5 right-5 z-30 p-2.5 rounded-lg backdrop-blur-xl transition-colors duration-200 hover:bg-white/[0.06]"
        aria-label={t('jarvis.settings_open')}
        title={t('jarvis.settings_open')}
        style={{ background: PANEL_BG, border: HAIRLINE }}
      >
        <SettingsIcon className="h-4 w-4" style={{ color: TEXT_SECONDARY }} />
      </button>

      <div
        className="absolute top-6 left-1/2 -translate-x-1/2 z-20 flex flex-col items-center gap-2 pointer-events-none"
        aria-live="polite"
      >
        <div
          className="flex items-center gap-2.5 pl-3 pr-4 py-2 rounded-full backdrop-blur-xl"
          style={{ background: PANEL_BG, border: HAIRLINE }}
        >
          <span
            className="inline-block h-1.5 w-1.5 rounded-full"
            style={{
              background: statusDotColor,
              boxShadow: `0 0 8px ${statusDotColor}`,
              animation: isListening || voice.mode === 'thinking' ? 'pulse-dot 1.4s ease-in-out infinite' : 'none',
            }}
          />
          <p
            className="text-[11px] tracking-[0.22em] uppercase font-medium"
            style={{ color: TEXT_PRIMARY }}
          >
            {statusLabel}
          </p>
        </div>
        <div
          className="px-2.5 py-1 rounded-full backdrop-blur-xl"
          style={{ background: PANEL_BG_SOFT, border: HAIRLINE }}
        >
          <p
            className="text-[9px] tracking-[0.22em] uppercase font-medium"
            style={{ color: activeProvider === 'browser' ? '#fda4af' : '#a7f3d0' }}
          >
            <span style={{ color: TEXT_MUTED, marginRight: 6 }}>STT</span>
            {providerLabel}
          </p>
        </div>
      </div>

      <div className="absolute left-1/2 -translate-x-1/2 bottom-12 z-20 flex flex-col items-center gap-5 w-full max-w-2xl px-6">
        <button
          onPointerDown={handlePressStart}
          onPointerUp={handlePressEnd}
          onPointerCancel={handlePressEnd}
          onPointerLeave={isListening ? handlePressEnd : undefined}
          disabled={isMicBusy}
          className="group relative flex items-center gap-3 px-7 py-3.5 rounded-full transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed select-none"
          style={{
            background: isListening ? ACCENT : PANEL_BG,
            border: isListening ? '1px solid rgba(34, 211, 238, 0.5)' : HAIRLINE,
            backdropFilter: 'blur(20px)',
            boxShadow: isListening
              ? `0 0 0 4px rgba(34, 211, 238, 0.15), 0 8px 32px rgba(34, 211, 238, 0.35)`
              : '0 8px 32px rgba(0, 0, 0, 0.5)',
          }}
        >
          <Mic
            className="h-4 w-4"
            style={{ color: isListening ? '#0a0a0e' : TEXT_PRIMARY }}
          />
          <span
            className="text-[12px] tracking-[0.18em] uppercase font-semibold"
            style={{ color: isListening ? '#0a0a0e' : TEXT_PRIMARY }}
          >
            {isListening ? t('jarvis.release_to_send') : t('jarvis.hold_to_talk')}
          </span>
        </button>

        {exchanges.length > 0 && (
          <div
            className="w-full space-y-2 p-4 rounded-2xl backdrop-blur-xl"
            style={{ background: PANEL_BG, border: HAIRLINE }}
          >
            {exchanges.slice(-3).map((ex, i) => (
              <div
                key={i}
                className="p-3 rounded-lg"
                style={{ background: 'rgba(255, 255, 255, 0.02)', border: HAIRLINE }}
              >
                <div className="flex items-baseline gap-2 mb-1">
                  <span
                    className="inline-block h-1 w-1 rounded-full"
                    style={{ background: ACCENT }}
                  />
                  <span
                    className="text-[9px] font-semibold uppercase tracking-[0.2em]"
                    style={{ color: TEXT_MUTED }}
                  >
                    {t('jarvis.you')}
                  </span>
                </div>
                <p className="mb-2 text-[13px] leading-relaxed" style={{ color: TEXT_PRIMARY }}>
                  {ex.user}
                </p>
                {ex.assistant && (
                  <>
                    <div className="flex items-baseline gap-2 mb-1 mt-3">
                      <span
                        className="inline-block h-1 w-1 rounded-full"
                        style={{ background: '#a78bfa' }}
                      />
                      <span
                        className="text-[9px] font-semibold uppercase tracking-[0.2em]"
                        style={{ color: TEXT_MUTED }}
                      >
                        Jarvis
                      </span>
                    </div>
                    <p className="text-[13px] leading-relaxed" style={{ color: TEXT_PRIMARY }}>
                      {ex.assistant}
                    </p>
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
