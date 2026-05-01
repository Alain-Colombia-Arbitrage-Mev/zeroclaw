// Spline-backed Jarvis orb. Wraps `@splinetool/react-spline` and
// pushes the live `audioLevel` + `mode` into the scene's variables
// (when the scene defines them) so a Spline-authored orb can react to
// the mic stream and the agent's state without code changes.
//
// To make a Spline scene reactive, expose two variables in the
// editor:
//   - `audioLevel` — number, 0..1 (mic / synth RMS)
//   - `mode`       — string: idle | listening | speaking | thinking
//
// You can wire those variables to any property in the scene
// (intensity, scale, material colour, etc.). If the scene doesn't
// expose them, the push is a silent no-op — the orb still renders.

import { useEffect, useRef, useState } from 'react';
import Spline from '@splinetool/react-spline';
import type { Application } from '@splinetool/runtime';

interface JarvisOrbSplineProps {
  /** `*.splinecode` URL — Export → Code Export → React → `scene` prop. */
  sceneUrl: string;
  /** 0..1 mic / synth RMS for scene reactivity. */
  audioLevel: number;
  /** Visual state — pushed as the `mode` Spline variable. */
  mode: 'idle' | 'listening' | 'speaking' | 'thinking';
  /** Optional load callback (e.g. to flip a parent loading flag). */
  onLoad?: () => void;
  /** Optional error callback used by the page to fall back to the
   *  built-in Three.js orb when the Spline runtime can't load. */
  onError?: (err: unknown) => void;
}

export default function JarvisOrbSpline({
  sceneUrl,
  audioLevel,
  mode,
  onLoad,
  onError,
}: JarvisOrbSplineProps) {
  const appRef = useRef<Application | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [errorText, setErrorText] = useState<string | null>(null);

  // Push current audio + mode into the scene every render. `setVariable`
  // is a fast no-op when the variable isn't defined in the scene, so we
  // don't gate this on a "ready" flag.
  useEffect(() => {
    const app = appRef.current;
    if (!app) return;
    try {
      // Both names are silently ignored by Spline if the scene doesn't
      // declare them — that's the documented contract, no try/catch
      // dance needed beyond defensive guards.
      if (typeof app.setVariable === 'function') {
        app.setVariable('audioLevel', audioLevel);
        app.setVariable('mode', mode);
      }
    } catch {
      // Some Spline runtime versions throw on unknown variable names
      // instead of returning silently — swallow so a malformed scene
      // doesn't kill the page.
    }
  }, [audioLevel, mode]);

  return (
    <div className="absolute inset-0 w-full h-full">
      <Spline
        scene={sceneUrl}
        onLoad={(app) => {
          appRef.current = app;
          setLoaded(true);
          // eslint-disable-next-line no-console
          console.info('[JarvisOrbSpline] scene loaded', sceneUrl);
          onLoad?.();
        }}
        onError={(e: unknown) => {
          const msg = e instanceof Error ? e.message : String(e);
          setErrorText(msg);
          // eslint-disable-next-line no-console
          console.error('[JarvisOrbSpline] scene failed to load', sceneUrl, e);
          onError?.(e);
        }}
        style={{ width: '100%', height: '100%' }}
      />
      {!loaded && !errorText && (
        <div
          className="absolute inset-0 flex items-center justify-center pointer-events-none"
          style={{ color: 'rgba(255,255,255,0.45)', fontSize: '0.8rem', letterSpacing: '0.2em' }}
        >
          LOADING SPLINE SCENE…
        </div>
      )}
      {errorText && (
        <div
          className="absolute inset-x-0 bottom-1/3 text-center pointer-events-none"
          style={{ color: '#f87171', fontSize: '0.75rem' }}
        >
          Spline error: {errorText}
        </div>
      )}
    </div>
  );
}
