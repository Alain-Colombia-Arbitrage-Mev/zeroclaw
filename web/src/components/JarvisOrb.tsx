// Siri-style orb — a single liquid sphere with a flowing tri-colour
// gradient. Deliberately minimal: no particles, no rings, no comets,
// no aggressive bloom. The whole effect is one mesh + one shader, so
// it stays smooth at 60 FPS on integrated GPUs and reads as
// "polished UI primitive" rather than "tech demo".
//
// Visual recipe
//   - High-poly icosahedron (subdivision 5 ≈ 10k vertices) so the
//     surface deformation looks fluid instead of faceted.
//   - Vertex shader: 3-octave fractional Brownian motion (fBm) on the
//     domain `position * scale + time * drift`. Output displaces along
//     the normal. Bass / RMS scales the amplitude.
//   - Fragment shader: three brand-aligned colour stops blended by a
//     smoothstep curve over the world-space y axis, plus a slow
//     rotation of the gradient phase so the colours appear to flow.
//     A gentle fresnel rim adds depth without becoming neon.
//   - Tone mapping: ACES filmic on the renderer for a polished look.
//
// Audio coupling is intentionally subtle: motion remains alive while
// idle, and audio just *amplifies* the base motion. Loud input does
// not turn the orb into a punching bag.

import { useEffect, useRef } from 'react';
import * as THREE from 'three';

interface JarvisOrbProps {
  /** 0..1 — overall input level (RMS). */
  audioLevel: number;
  /** Active visual state. */
  mode: 'idle' | 'listening' | 'speaking' | 'thinking';
  /** Optional pixel size; defaults to 360. */
  size?: number;
}

interface Palette {
  /** Top of the gradient (light cap). */
  top: THREE.Color;
  /** Middle of the gradient (dominant tone). */
  mid: THREE.Color;
  /** Bottom of the gradient (deep tone). */
  bot: THREE.Color;
}

const MODE_PALETTES: Record<JarvisOrbProps['mode'], Palette> = {
  // Idle — calm blue → indigo → violet (Siri-ish).
  idle: {
    top: new THREE.Color('#7dd3fc'),
    mid: new THREE.Color('#6366f1'),
    bot: new THREE.Color('#a855f7'),
  },
  // Listening — brighter, cooler cyans, a hint of teal at the bottom.
  listening: {
    top: new THREE.Color('#a5f3fc'),
    mid: new THREE.Color('#22d3ee'),
    bot: new THREE.Color('#0ea5e9'),
  },
  // Speaking — rosy pink / coral / lavender, warmer to feel "alive".
  speaking: {
    top: new THREE.Color('#fda4af'),
    mid: new THREE.Color('#a855f7'),
    bot: new THREE.Color('#6366f1'),
  },
  // Thinking — amber / orange / pink, a slow heat.
  thinking: {
    top: new THREE.Color('#fed7aa'),
    mid: new THREE.Color('#f59e0b'),
    bot: new THREE.Color('#ec4899'),
  },
};

const sphereVertex = /* glsl */ `
  uniform float uTime;
  uniform float uLevel;
  varying vec3 vWorldPos;
  varying vec3 vNormal;
  varying float vDisp;

  // ── Simplex-like 3D noise (Ashima / Stefan Gustavson) ─────────────
  // Compact GLSL port — one octave costs ~30 ALU instructions.
  vec4 permute(vec4 x) { return mod(((x*34.0)+1.0)*x, 289.0); }
  vec4 taylorInvSqrt(vec4 r) { return 1.79284291400159 - 0.85373472095314 * r; }

  float snoise(vec3 v) {
    const vec2 C = vec2(1.0/6.0, 1.0/3.0);
    const vec4 D = vec4(0.0, 0.5, 1.0, 2.0);
    vec3 i  = floor(v + dot(v, C.yyy));
    vec3 x0 = v - i + dot(i, C.xxx);
    vec3 g = step(x0.yzx, x0.xyz);
    vec3 l = 1.0 - g;
    vec3 i1 = min(g.xyz, l.zxy);
    vec3 i2 = max(g.xyz, l.zxy);
    vec3 x1 = x0 - i1 + 1.0 * C.xxx;
    vec3 x2 = x0 - i2 + 2.0 * C.xxx;
    vec3 x3 = x0 - 1.0 + 3.0 * C.xxx;
    i = mod(i, 289.0);
    vec4 p = permute(permute(permute(
              i.z + vec4(0.0, i1.z, i2.z, 1.0))
            + i.y + vec4(0.0, i1.y, i2.y, 1.0))
            + i.x + vec4(0.0, i1.x, i2.x, 1.0));
    float n_ = 1.0 / 7.0;
    vec3 ns = n_ * D.wyz - D.xzx;
    vec4 j = p - 49.0 * floor(p * ns.z * ns.z);
    vec4 x_ = floor(j * ns.z);
    vec4 y_ = floor(j - 7.0 * x_);
    vec4 x = x_ * ns.x + ns.yyyy;
    vec4 y = y_ * ns.x + ns.yyyy;
    vec4 h = 1.0 - abs(x) - abs(y);
    vec4 b0 = vec4(x.xy, y.xy);
    vec4 b1 = vec4(x.zw, y.zw);
    vec4 s0 = floor(b0) * 2.0 + 1.0;
    vec4 s1 = floor(b1) * 2.0 + 1.0;
    vec4 sh = -step(h, vec4(0.0));
    vec4 a0 = b0.xzyw + s0.xzyw * sh.xxyy;
    vec4 a1 = b1.xzyw + s1.xzyw * sh.zzww;
    vec3 p0 = vec3(a0.xy, h.x);
    vec3 p1 = vec3(a0.zw, h.y);
    vec3 p2 = vec3(a1.xy, h.z);
    vec3 p3 = vec3(a1.zw, h.w);
    vec4 norm = taylorInvSqrt(vec4(dot(p0,p0), dot(p1,p1), dot(p2,p2), dot(p3,p3)));
    p0 *= norm.x; p1 *= norm.y; p2 *= norm.z; p3 *= norm.w;
    vec4 m = max(0.6 - vec4(dot(x0,x0), dot(x1,x1), dot(x2,x2), dot(x3,x3)), 0.0);
    m = m * m;
    return 42.0 * dot(m * m, vec4(dot(p0,x0), dot(p1,x1), dot(p2,x2), dot(p3,x3)));
  }

  // 3-octave fBm — gives smooth flowing surface motion.
  float fbm(vec3 p) {
    float f = 0.0;
    float amp = 0.5;
    for (int i = 0; i < 3; i++) {
      f += amp * snoise(p);
      p *= 2.02;
      amp *= 0.5;
    }
    return f;
  }

  void main() {
    vNormal = normalize(normalMatrix * normal);
    // Slow domain drift = "the orb is breathing".
    float n = fbm(position * 1.4 + vec3(0.0, uTime * 0.18, 0.0));
    // Idle baseline ripple, audio amplifies up to ~0.18.
    float disp = (0.018 + uLevel * 0.12) * n;
    vDisp = disp;
    vec3 pos = position + normal * disp;
    vec4 world = modelMatrix * vec4(pos, 1.0);
    vWorldPos = world.xyz;
    gl_Position = projectionMatrix * viewMatrix * world;
  }
`;

const sphereFragment = /* glsl */ `
  uniform vec3 uTopColor;
  uniform vec3 uMidColor;
  uniform vec3 uBotColor;
  uniform float uTime;
  uniform float uLevel;
  varying vec3 vWorldPos;
  varying vec3 vNormal;
  varying float vDisp;

  void main() {
    // Gradient axis rotates slowly so the colours appear to flow.
    float angle = uTime * 0.12;
    vec3 axis = normalize(vec3(sin(angle), 1.0, cos(angle)));
    float t = (dot(normalize(vWorldPos), axis) + 1.0) * 0.5; // 0..1

    // Two smoothstep blends = three-stop gradient with smooth knees.
    vec3 lower = mix(uBotColor, uMidColor, smoothstep(0.0, 0.55, t));
    vec3 grad = mix(lower, uTopColor, smoothstep(0.45, 1.0, t));

    // Subtle fresnel rim — adds dimensionality without neon.
    float fres = pow(1.0 - max(dot(vNormal, vec3(0.0, 0.0, 1.0)), 0.0), 3.0);
    vec3 rim = vec3(1.0) * fres * 0.18;

    // Mild brightness bump from displacement = highlight on bumps.
    float highlight = clamp(vDisp * 6.0, 0.0, 1.0) * 0.12;

    // Audio nudges overall brightness — but never blows out.
    float lift = 0.92 + uLevel * 0.18;

    vec3 col = grad * lift + rim + highlight;
    gl_FragColor = vec4(col, 0.96);
  }
`;

export default function JarvisOrb({ audioLevel, mode, size = 360 }: JarvisOrbProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const audioLevelRef = useRef(audioLevel);
  const modeRef = useRef(mode);

  audioLevelRef.current = audioLevel;
  modeRef.current = mode;

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(size, size);
    renderer.setClearColor(0x000000, 0);
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.toneMappingExposure = 1.0;
    container.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(40, 1, 0.1, 100);
    camera.position.set(0, 0, 3.4);

    // High subdivision for fluid-looking deformation.
    const sphereGeom = new THREE.IcosahedronGeometry(1, 5);
    const palette = MODE_PALETTES.idle;
    const sphereMat = new THREE.ShaderMaterial({
      vertexShader: sphereVertex,
      fragmentShader: sphereFragment,
      uniforms: {
        uTime: { value: 0 },
        uLevel: { value: 0 },
        uTopColor: { value: palette.top.clone() },
        uMidColor: { value: palette.mid.clone() },
        uBotColor: { value: palette.bot.clone() },
      },
      transparent: true,
    });
    const sphere = new THREE.Mesh(sphereGeom, sphereMat);
    scene.add(sphere);

    const start = performance.now();
    let smoothLevel = 0;
    // Per-channel lerp targets so colour transitions are smooth between modes.
    const cTop = palette.top.clone();
    const cMid = palette.mid.clone();
    const cBot = palette.bot.clone();
    let frame = 0;

    const tick = () => {
      const t = (performance.now() - start) / 1000;
      const target = Math.min(1, Math.max(0, audioLevelRef.current));
      smoothLevel += (target - smoothLevel) * 0.14;

      const desired = MODE_PALETTES[modeRef.current];
      cTop.lerp(desired.top, 0.06);
      cMid.lerp(desired.mid, 0.06);
      cBot.lerp(desired.bot, 0.06);
      (sphereMat.uniforms.uTopColor!.value as THREE.Color).copy(cTop);
      (sphereMat.uniforms.uMidColor!.value as THREE.Color).copy(cMid);
      (sphereMat.uniforms.uBotColor!.value as THREE.Color).copy(cBot);

      sphereMat.uniforms.uTime!.value = t;
      sphereMat.uniforms.uLevel!.value = smoothLevel;

      // Slow gentle rotation — the gradient already animates, so the
      // mesh rotation is intentionally minimal.
      sphere.rotation.y = t * 0.06;
      sphere.rotation.x = Math.sin(t * 0.18) * 0.08;

      renderer.render(scene, camera);
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(frame);
      renderer.dispose();
      sphereGeom.dispose();
      sphereMat.dispose();
      if (renderer.domElement.parentElement === container) {
        container.removeChild(renderer.domElement);
      }
    };
  }, [size]);

  return (
    <div
      ref={containerRef}
      style={{ width: size, height: size }}
      aria-hidden="true"
    />
  );
}
