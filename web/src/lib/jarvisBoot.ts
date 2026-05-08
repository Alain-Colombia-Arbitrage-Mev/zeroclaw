// Generates a short sci-fi power-on chime via the Web Audio API so we don't
// have to ship an audio asset. The sound layers a low-frequency sweep with a
// glassy upper harmonic and a brief filtered noise tail — close in feeling to
// a "JARVIS online" boot tone without copying any specific source.

const STORAGE_KEY = 'octopus_jarvis_sound_enabled';

export function isJarvisBootSoundEnabled(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) !== '0';
  } catch {
    return true;
  }
}

export function setJarvisBootSoundEnabled(enabled: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY, enabled ? '1' : '0');
  } catch {
    // localStorage unavailable — silent ignore
  }
}

/**
 * Play the JARVIS boot chime. Returns a promise that resolves when the
 * envelope has fully decayed. Safe to call from a React effect — autoplay
 * policies require a user gesture, which the navigation click satisfies.
 */
export async function playJarvisBootSound(volume = 0.35): Promise<void> {
  if (typeof window === 'undefined' || !isJarvisBootSoundEnabled()) return;

  const AudioCtx =
    window.AudioContext ||
    (window as unknown as { webkitAudioContext: typeof AudioContext })
      .webkitAudioContext;
  if (!AudioCtx) return;

  let ctx: AudioContext;
  try {
    ctx = new AudioCtx();
  } catch {
    return;
  }

  // Some browsers create the context in `suspended` state until a gesture.
  if (ctx.state === 'suspended') {
    try {
      await ctx.resume();
    } catch {
      /* ignore */
    }
  }

  const now = ctx.currentTime;
  const duration = 1.1;

  // Master gain — fades in then out so the chime never clicks.
  const master = ctx.createGain();
  master.gain.setValueAtTime(0, now);
  master.gain.linearRampToValueAtTime(volume, now + 0.04);
  master.gain.exponentialRampToValueAtTime(0.0001, now + duration);
  master.connect(ctx.destination);

  // Layer 1: low sine sweep — the "warmth" under the chime.
  const subOsc = ctx.createOscillator();
  subOsc.type = 'sine';
  subOsc.frequency.setValueAtTime(110, now);
  subOsc.frequency.exponentialRampToValueAtTime(220, now + 0.6);
  const subGain = ctx.createGain();
  subGain.gain.setValueAtTime(0.6, now);
  subGain.gain.exponentialRampToValueAtTime(0.0001, now + 0.9);
  subOsc.connect(subGain).connect(master);
  subOsc.start(now);
  subOsc.stop(now + 1);

  // Layer 2: glassy triangle harmonic that sweeps up — the "JARVIS" timbre.
  const leadOsc = ctx.createOscillator();
  leadOsc.type = 'triangle';
  leadOsc.frequency.setValueAtTime(440, now);
  leadOsc.frequency.exponentialRampToValueAtTime(1320, now + 0.5);
  leadOsc.frequency.exponentialRampToValueAtTime(880, now + 0.9);
  const leadFilter = ctx.createBiquadFilter();
  leadFilter.type = 'bandpass';
  leadFilter.frequency.setValueAtTime(900, now);
  leadFilter.frequency.linearRampToValueAtTime(1600, now + 0.5);
  leadFilter.Q.value = 4;
  const leadGain = ctx.createGain();
  leadGain.gain.setValueAtTime(0, now);
  leadGain.gain.linearRampToValueAtTime(0.4, now + 0.08);
  leadGain.gain.exponentialRampToValueAtTime(0.0001, now + 0.95);
  leadOsc.connect(leadFilter).connect(leadGain).connect(master);
  leadOsc.start(now);
  leadOsc.stop(now + 1);

  // Layer 3: detuned 5th for a cinematic stack.
  const fifthOsc = ctx.createOscillator();
  fifthOsc.type = 'sine';
  fifthOsc.frequency.setValueAtTime(659.25, now); // E5
  fifthOsc.detune.setValueAtTime(-7, now);
  const fifthGain = ctx.createGain();
  fifthGain.gain.setValueAtTime(0, now);
  fifthGain.gain.linearRampToValueAtTime(0.18, now + 0.12);
  fifthGain.gain.exponentialRampToValueAtTime(0.0001, now + 0.9);
  fifthOsc.connect(fifthGain).connect(master);
  fifthOsc.start(now + 0.05);
  fifthOsc.stop(now + 1);

  // Layer 4: short noise burst gated through a hi-pass — the "powerup" hiss.
  const noiseDuration = 0.35;
  const noiseBuffer = ctx.createBuffer(
    1,
    Math.floor(ctx.sampleRate * noiseDuration),
    ctx.sampleRate,
  );
  const noiseData = noiseBuffer.getChannelData(0);
  for (let i = 0; i < noiseData.length; i++) {
    noiseData[i] = (Math.random() * 2 - 1) * (1 - i / noiseData.length);
  }
  const noiseSrc = ctx.createBufferSource();
  noiseSrc.buffer = noiseBuffer;
  const noiseFilter = ctx.createBiquadFilter();
  noiseFilter.type = 'highpass';
  noiseFilter.frequency.setValueAtTime(2400, now);
  const noiseGain = ctx.createGain();
  noiseGain.gain.setValueAtTime(0.18, now);
  noiseGain.gain.exponentialRampToValueAtTime(0.0001, now + noiseDuration);
  noiseSrc.connect(noiseFilter).connect(noiseGain).connect(master);
  noiseSrc.start(now);

  // Auto-close the context once the envelope has decayed.
  return new Promise<void>((resolve) => {
    setTimeout(() => {
      ctx.close().catch(() => {});
      resolve();
    }, duration * 1000 + 50);
  });
}
