// Iron-Man-style Jarvis orb with bloom, orbital rings, FFT-driven
// deformation, particle trails, and an inner energy core.
//
// Layers, from inside out:
//   1. Inner core    — small SphereGeometry with bright additive material;
//                      flickers with treble band.
//   2. Outer shell   — IcosahedronGeometry shader with vertex displacement
//                      driven by the bass band (slow, heavy pulse) and
//                      fresnel-tinted fragment.
//   3. Three orbital rings — TorusGeometry on offset planes, each with
//                      its own rotation axis and speed; expand with bass.
//   4. Particle cloud — 1000 ambient points orbiting the sphere at random
//                      radii; agitation modulated by the mid band.
//   5. Particle trails — 60 "comet" streaks rendered as additive Lines,
//                      each holding an 10-step position history. Speed
//                      and length scale with treble.
//
// Post-processing: EffectComposer with UnrealBloomPass for the
// arc-reactor glow.

import { useEffect, useRef } from 'react';
import * as THREE from 'three';
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js';
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js';
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js';

interface AudioBands {
  bass: number;
  mid: number;
  treble: number;
}

interface JarvisOrbProps {
  /** 0..1 overall RMS — used as fallback when bands are zero. */
  audioLevel: number;
  /** Per-band 0..1 audio energy. Bass drives core, mid drives particles, treble drives sparkle. */
  audioBands?: AudioBands;
  mode: 'idle' | 'listening' | 'speaking' | 'thinking';
  /** Optional pixel size; defaults to 360. */
  size?: number;
}

const MODE_COLORS: Record<JarvisOrbProps['mode'], THREE.Color> = {
  idle: new THREE.Color('#3b82f6'),
  listening: new THREE.Color('#22d3ee'),
  speaking: new THREE.Color('#a855f7'),
  thinking: new THREE.Color('#f59e0b'),
};

const sphereVertex = /* glsl */ `
  uniform float uTime;
  uniform float uBass;
  varying vec3 vNormal;
  varying float vDisp;

  // Cheap noise — sufficient for surface ripple
  float noise(vec3 p) {
    return sin(p.x * 4.0 + uTime) * sin(p.y * 4.0 + uTime * 1.3) * sin(p.z * 4.0 + uTime * 0.7);
  }

  void main() {
    vNormal = normalize(normalMatrix * normal);
    float n = noise(position * 1.6 + uTime * 0.4);
    float disp = n * (0.04 + uBass * 0.28);
    vDisp = disp;
    vec3 pos = position + normal * disp;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
  }
`;

const sphereFragment = /* glsl */ `
  uniform vec3 uColor;
  uniform float uBass;
  uniform float uTreble;
  varying vec3 vNormal;
  varying float vDisp;

  void main() {
    float fres = pow(1.0 - max(dot(vNormal, vec3(0.0, 0.0, 1.0)), 0.0), 2.5);
    float pulse = 0.5 + 0.4 * uBass + 0.18 * vDisp * 8.0 + uTreble * 0.25;
    vec3 col = uColor * (0.35 + fres * 1.4) * pulse;
    gl_FragColor = vec4(col, 0.85);
  }
`;

const coreVertex = /* glsl */ `
  varying vec3 vNormal;
  void main() {
    vNormal = normalize(normalMatrix * normal);
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  }
`;

const coreFragment = /* glsl */ `
  uniform vec3 uColor;
  uniform float uTreble;
  uniform float uTime;
  varying vec3 vNormal;

  void main() {
    float fres = pow(1.0 - max(dot(vNormal, vec3(0.0, 0.0, 1.0)), 0.0), 1.6);
    float flicker = 0.85 + sin(uTime * 18.0) * 0.05 + uTreble * 0.7;
    vec3 col = mix(uColor, vec3(1.0), 0.55) * flicker;
    gl_FragColor = vec4(col + fres * 0.6, 1.0);
  }
`;

export default function JarvisOrb({
  audioLevel,
  audioBands,
  mode,
  size = 360,
}: JarvisOrbProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const audioLevelRef = useRef(audioLevel);
  const audioBandsRef = useRef<AudioBands>(audioBands ?? { bass: 0, mid: 0, treble: 0 });
  const modeRef = useRef(mode);

  // Mirror props into refs so the rAF loop reads latest without re-creating the scene.
  audioLevelRef.current = audioLevel;
  audioBandsRef.current = audioBands ?? {
    bass: audioLevel,
    mid: audioLevel * 0.7,
    treble: audioLevel * 0.5,
  };
  modeRef.current = mode;

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(size, size);
    renderer.setClearColor(0x000000, 0);
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    container.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
    camera.position.set(0, 0, 4.0);

    // ── Inner core (small bright sphere) ─────────────────────────────
    const coreGeom = new THREE.SphereGeometry(0.42, 48, 48);
    const coreMat = new THREE.ShaderMaterial({
      vertexShader: coreVertex,
      fragmentShader: coreFragment,
      uniforms: {
        uTime: { value: 0 },
        uTreble: { value: 0 },
        uColor: { value: MODE_COLORS.idle.clone() },
      },
      transparent: false,
    });
    const core = new THREE.Mesh(coreGeom, coreMat);
    scene.add(core);

    // ── Outer shell (deformed icosahedron) ───────────────────────────
    const sphereGeom = new THREE.IcosahedronGeometry(1, 64);
    const sphereMat = new THREE.ShaderMaterial({
      vertexShader: sphereVertex,
      fragmentShader: sphereFragment,
      uniforms: {
        uTime: { value: 0 },
        uBass: { value: 0 },
        uTreble: { value: 0 },
        uColor: { value: MODE_COLORS.idle.clone() },
      },
      transparent: true,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    const sphere = new THREE.Mesh(sphereGeom, sphereMat);
    scene.add(sphere);

    // ── Orbital rings ────────────────────────────────────────────────
    const RING_RADII = [1.45, 1.7, 2.0];
    const RING_TUBE = 0.012;
    const ringTilts: [number, number, number][] = [
      [Math.PI / 2.2, 0, 0],
      [Math.PI / 3, Math.PI / 4, 0],
      [Math.PI / 4, -Math.PI / 3, Math.PI / 6],
    ];
    const rings: THREE.Mesh[] = [];
    const ringMaterials: THREE.MeshBasicMaterial[] = [];
    for (let i = 0; i < RING_RADII.length; i++) {
      const geom = new THREE.TorusGeometry(RING_RADII[i]!, RING_TUBE, 16, 192);
      const mat = new THREE.MeshBasicMaterial({
        color: 0x9bd6ff,
        transparent: true,
        opacity: 0.65,
        blending: THREE.AdditiveBlending,
        depthWrite: false,
      });
      const torus = new THREE.Mesh(geom, mat);
      const tilt = ringTilts[i]!;
      torus.rotation.set(tilt[0], tilt[1], tilt[2]);
      scene.add(torus);
      rings.push(torus);
      ringMaterials.push(mat);
    }

    // ── Particle cloud ────────────────────────────────────────────────
    const PARTICLE_COUNT = 1000;
    const particlePositions = new Float32Array(PARTICLE_COUNT * 3);
    const baseRadii = new Float32Array(PARTICLE_COUNT);
    const phases = new Float32Array(PARTICLE_COUNT);

    for (let i = 0; i < PARTICLE_COUNT; i++) {
      const r = 1.5 + Math.random() * 1.0;
      const theta = Math.random() * Math.PI * 2;
      const phi = Math.acos(2 * Math.random() - 1);
      particlePositions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
      particlePositions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
      particlePositions[i * 3 + 2] = r * Math.cos(phi);
      baseRadii[i] = r;
      phases[i] = Math.random() * Math.PI * 2;
    }
    const particlesGeom = new THREE.BufferGeometry();
    particlesGeom.setAttribute('position', new THREE.BufferAttribute(particlePositions, 3));
    const particlesMat = new THREE.PointsMaterial({
      color: 0x9bb6ff,
      size: 0.022,
      transparent: true,
      opacity: 0.85,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    const particles = new THREE.Points(particlesGeom, particlesMat);
    scene.add(particles);

    // ── Comet trails ──────────────────────────────────────────────────
    // Each "comet" is a Line with TRAIL_LEN segments; we shift positions
    // along each frame so the head leads and the tail fades in alpha.
    const COMET_COUNT = 60;
    const TRAIL_LEN = 10;
    const trailGeoms: THREE.BufferGeometry[] = [];
    const trailMats: THREE.LineBasicMaterial[] = [];
    const trailLines: THREE.Line[] = [];
    const cometState: {
      theta: number;
      phi: number;
      radius: number;
      speed: number;
      phase: number;
    }[] = [];

    for (let i = 0; i < COMET_COUNT; i++) {
      const trailPositions = new Float32Array(TRAIL_LEN * 3);
      const trailColors = new Float32Array(TRAIL_LEN * 3);
      const r = 1.6 + Math.random() * 0.9;
      const theta = Math.random() * Math.PI * 2;
      const phi = Math.acos(2 * Math.random() - 1);
      const x = r * Math.sin(phi) * Math.cos(theta);
      const y = r * Math.sin(phi) * Math.sin(theta);
      const z = r * Math.cos(phi);
      for (let j = 0; j < TRAIL_LEN; j++) {
        trailPositions[j * 3] = x;
        trailPositions[j * 3 + 1] = y;
        trailPositions[j * 3 + 2] = z;
        // Linear alpha gradient from head (1.0) to tail (0.0) baked into
        // per-vertex colours.
        const a = 1 - j / (TRAIL_LEN - 1);
        trailColors[j * 3] = a;
        trailColors[j * 3 + 1] = a;
        trailColors[j * 3 + 2] = a;
      }
      const geom = new THREE.BufferGeometry();
      geom.setAttribute('position', new THREE.BufferAttribute(trailPositions, 3));
      geom.setAttribute('color', new THREE.BufferAttribute(trailColors, 3));
      const mat = new THREE.LineBasicMaterial({
        vertexColors: true,
        transparent: true,
        opacity: 0.8,
        blending: THREE.AdditiveBlending,
        depthWrite: false,
      });
      const line = new THREE.Line(geom, mat);
      scene.add(line);
      trailGeoms.push(geom);
      trailMats.push(mat);
      trailLines.push(line);
      cometState.push({
        theta,
        phi,
        radius: r,
        speed: 0.3 + Math.random() * 1.2,
        phase: Math.random() * Math.PI * 2,
      });
    }

    // ── Post-processing (bloom) ───────────────────────────────────────
    const composer = new EffectComposer(renderer);
    composer.setSize(size, size);
    composer.addPass(new RenderPass(scene, camera));
    const bloomPass = new UnrealBloomPass(
      new THREE.Vector2(size, size),
      1.2, // strength
      0.7, // radius
      0.05, // threshold (low so even idle elements glow)
    );
    composer.addPass(bloomPass);
    composer.addPass(new OutputPass());

    // ── Animation loop ────────────────────────────────────────────────
    const start = performance.now();
    let smoothBass = 0;
    let smoothMid = 0;
    let smoothTreble = 0;
    const targetColor = MODE_COLORS.idle.clone();
    let frame = 0;

    const tick = () => {
      const t = (performance.now() - start) / 1000;
      const bands = audioBandsRef.current;
      const targetBass = Math.min(1, Math.max(0, bands.bass));
      const targetMid = Math.min(1, Math.max(0, bands.mid));
      const targetTreble = Math.min(1, Math.max(0, bands.treble));
      smoothBass += (targetBass - smoothBass) * 0.18;
      smoothMid += (targetMid - smoothMid) * 0.22;
      smoothTreble += (targetTreble - smoothTreble) * 0.32;

      // ── Colour lerp ──
      const desired = MODE_COLORS[modeRef.current];
      targetColor.lerp(desired, 0.08);
      (sphereMat.uniforms.uColor!.value as THREE.Color).copy(targetColor);
      (coreMat.uniforms.uColor!.value as THREE.Color).copy(targetColor);
      particlesMat.color.copy(targetColor).multiplyScalar(1.4);
      const ringTint = targetColor.clone().multiplyScalar(1.6);
      for (const m of ringMaterials) m.color.copy(ringTint);
      for (const m of trailMats) m.color = ringTint.clone();

      // ── Sphere shell ──
      sphereMat.uniforms.uTime!.value = t;
      sphereMat.uniforms.uBass!.value = smoothBass;
      sphereMat.uniforms.uTreble!.value = smoothTreble;
      const rotSpeed = 0.12 + smoothBass * 0.5;
      sphere.rotation.y = t * rotSpeed;
      sphere.rotation.x = Math.sin(t * 0.4) * 0.15;

      // ── Inner core ──
      coreMat.uniforms.uTime!.value = t;
      coreMat.uniforms.uTreble!.value = smoothTreble;
      const coreScale = 1.0 + smoothBass * 0.25 + Math.sin(t * 4.0) * 0.04;
      core.scale.setScalar(coreScale);

      // ── Rings ──
      for (let i = 0; i < rings.length; i++) {
        const r = rings[i]!;
        const baseSpeed = 0.5 + i * 0.35;
        r.rotation.x += (baseSpeed * 0.01) * (1 + smoothBass * 1.5);
        r.rotation.y += (baseSpeed * 0.013) * (1 + smoothMid * 1.0);
        r.rotation.z += (baseSpeed * 0.007) * (1 + smoothTreble * 0.6);
        const pulse = 1 + smoothBass * 0.18 + Math.sin(t * 1.5 + i) * 0.03;
        r.scale.setScalar(pulse);
        ringMaterials[i]!.opacity = 0.5 + smoothBass * 0.45;
      }

      // ── Particles ──
      const posAttr = particlesGeom.getAttribute('position') as THREE.BufferAttribute;
      const arr = posAttr.array as Float32Array;
      const breath = 0.06 + smoothMid * 0.45;
      for (let i = 0; i < PARTICLE_COUNT; i++) {
        const r = baseRadii[i]! + Math.sin(t * 1.2 + phases[i]!) * breath;
        const cx = arr[i * 3]!;
        const cy = arr[i * 3 + 1]!;
        const cz = arr[i * 3 + 2]!;
        const len = Math.sqrt(cx * cx + cy * cy + cz * cz) || 1;
        const k = r / len;
        arr[i * 3] = cx * k;
        arr[i * 3 + 1] = cy * k;
        arr[i * 3 + 2] = cz * k;
      }
      posAttr.needsUpdate = true;
      particles.rotation.y = -t * (0.18 + smoothMid * 0.5);
      particles.rotation.x = Math.sin(t * 0.3) * 0.3;
      particlesMat.size = 0.018 + smoothMid * 0.025;

      // ── Comet trails ──
      for (let i = 0; i < COMET_COUNT; i++) {
        const c = cometState[i]!;
        c.theta += c.speed * (0.012 + smoothTreble * 0.06);
        const wobble = Math.sin(t * 1.7 + c.phase) * 0.12;
        const r = c.radius + wobble;
        const x = r * Math.sin(c.phi) * Math.cos(c.theta);
        const y = r * Math.sin(c.phi) * Math.sin(c.theta);
        const z = r * Math.cos(c.phi);

        const geom = trailGeoms[i]!;
        const tarr = (geom.getAttribute('position') as THREE.BufferAttribute)
          .array as Float32Array;
        // Shift older positions one slot down (memmove, head=0 stays freshest).
        for (let j = TRAIL_LEN - 1; j > 0; j--) {
          tarr[j * 3] = tarr[(j - 1) * 3]!;
          tarr[j * 3 + 1] = tarr[(j - 1) * 3 + 1]!;
          tarr[j * 3 + 2] = tarr[(j - 1) * 3 + 2]!;
        }
        tarr[0] = x;
        tarr[1] = y;
        tarr[2] = z;
        (geom.getAttribute('position') as THREE.BufferAttribute).needsUpdate = true;
        trailMats[i]!.opacity = 0.4 + smoothTreble * 0.6;
      }

      composer.render();
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(frame);
      composer.dispose();
      renderer.dispose();
      sphereGeom.dispose();
      sphereMat.dispose();
      coreGeom.dispose();
      coreMat.dispose();
      particlesGeom.dispose();
      particlesMat.dispose();
      for (const m of rings) m.geometry.dispose();
      for (const m of ringMaterials) m.dispose();
      for (const g of trailGeoms) g.dispose();
      for (const m of trailMats) m.dispose();
      bloomPass.dispose();
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
