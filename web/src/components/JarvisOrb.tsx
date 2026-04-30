// Three.js orb for the Jarvis voice page.
//
// Renders a sphere with a custom shader that produces a soft electric
// glow plus a particle cloud orbiting around it. When `audioLevel`
// (0..1) is non-zero, the sphere expands and particles agitate in
// response — the page feeds it the live mic RMS while listening and
// the synthesis envelope while speaking.
//
// State handling lives in the page; this component is purely visual.

import { useEffect, useRef } from 'react';
import * as THREE from 'three';

interface JarvisOrbProps {
  /** 0..1 — drives sphere pulse and particle agitation. */
  audioLevel: number;
  /** "idle" | "listening" | "speaking" | "thinking" — colour cue. */
  mode: 'idle' | 'listening' | 'speaking' | 'thinking';
  /** Optional pixel size; defaults to 360. */
  size?: number;
}

const MODE_COLORS: Record<JarvisOrbProps['mode'], THREE.Color> = {
  idle: new THREE.Color('#3b82f6'),       // blue
  listening: new THREE.Color('#22d3ee'),  // cyan
  speaking: new THREE.Color('#a855f7'),   // purple
  thinking: new THREE.Color('#f59e0b'),   // amber
};

const sphereVertex = /* glsl */ `
  uniform float uTime;
  uniform float uLevel;
  varying vec3 vNormal;
  varying float vDisp;

  // Cheap noise — sufficient for surface ripple
  float noise(vec3 p) {
    return sin(p.x * 4.0 + uTime) * sin(p.y * 4.0 + uTime * 1.3) * sin(p.z * 4.0 + uTime * 0.7);
  }

  void main() {
    vNormal = normalize(normalMatrix * normal);
    float n = noise(position * 1.5 + uTime * 0.4);
    float disp = n * (0.04 + uLevel * 0.20);
    vDisp = disp;
    vec3 pos = position + normal * disp;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
  }
`;

const sphereFragment = /* glsl */ `
  uniform vec3 uColor;
  uniform float uLevel;
  varying vec3 vNormal;
  varying float vDisp;

  void main() {
    float fres = pow(1.0 - max(dot(vNormal, vec3(0.0, 0.0, 1.0)), 0.0), 2.5);
    float pulse = 0.55 + 0.35 * uLevel + 0.18 * vDisp * 8.0;
    vec3 col = uColor * (0.35 + fres * 1.2) * pulse;
    gl_FragColor = vec4(col, 0.9);
  }
`;

export default function JarvisOrb({ audioLevel, mode, size = 360 }: JarvisOrbProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const audioLevelRef = useRef(audioLevel);
  const modeRef = useRef(mode);

  // Mirror props into refs so the rAF loop reads latest without re-creating the scene.
  audioLevelRef.current = audioLevel;
  modeRef.current = mode;

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(size, size);
    renderer.setClearColor(0x000000, 0);
    container.appendChild(renderer.domElement);

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100);
    camera.position.set(0, 0, 3.6);

    // ── Sphere ────────────────────────────────────────────────────────
    const sphereGeom = new THREE.IcosahedronGeometry(1, 64);
    const sphereMat = new THREE.ShaderMaterial({
      vertexShader: sphereVertex,
      fragmentShader: sphereFragment,
      uniforms: {
        uTime: { value: 0 },
        uLevel: { value: 0 },
        uColor: { value: MODE_COLORS.idle.clone() },
      },
      transparent: true,
    });
    const sphere = new THREE.Mesh(sphereGeom, sphereMat);
    scene.add(sphere);

    // ── Particle cloud ────────────────────────────────────────────────
    const PARTICLE_COUNT = 1200;
    const positions = new Float32Array(PARTICLE_COUNT * 3);
    const baseRadii = new Float32Array(PARTICLE_COUNT);
    const phases = new Float32Array(PARTICLE_COUNT);

    for (let i = 0; i < PARTICLE_COUNT; i++) {
      const r = 1.4 + Math.random() * 0.9;
      const theta = Math.random() * Math.PI * 2;
      const phi = Math.acos(2 * Math.random() - 1);
      positions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
      positions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
      positions[i * 3 + 2] = r * Math.cos(phi);
      baseRadii[i] = r;
      phases[i] = Math.random() * Math.PI * 2;
    }
    const particlesGeom = new THREE.BufferGeometry();
    particlesGeom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    const particlesMat = new THREE.PointsMaterial({
      color: 0x9bb6ff,
      size: 0.025,
      transparent: true,
      opacity: 0.85,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    const particles = new THREE.Points(particlesGeom, particlesMat);
    scene.add(particles);

    // ── Animation loop ────────────────────────────────────────────────
    const start = performance.now();
    let smoothLevel = 0;
    const targetColor = MODE_COLORS.idle.clone();
    let frame = 0;

    const tick = () => {
      const t = (performance.now() - start) / 1000;

      // Smooth audio level for less jittery visual response.
      const target = Math.min(1, Math.max(0, audioLevelRef.current));
      smoothLevel += (target - smoothLevel) * 0.18;

      sphereMat.uniforms.uTime!.value = t;
      sphereMat.uniforms.uLevel!.value = smoothLevel;

      // Smoothly lerp colour toward the active mode's hue.
      const desired = MODE_COLORS[modeRef.current];
      targetColor.lerp(desired, 0.08);
      (sphereMat.uniforms.uColor!.value as THREE.Color).copy(targetColor);
      particlesMat.color.copy(targetColor).multiplyScalar(1.4);

      // Slow base rotation, faster when speaking.
      const rotSpeed = 0.12 + smoothLevel * 0.6;
      sphere.rotation.y = t * rotSpeed;
      particles.rotation.y = -t * rotSpeed * 0.4;
      particles.rotation.x = Math.sin(t * 0.3) * 0.3;

      // Particle "breathing" — radial offset modulated by level.
      const posAttr = particlesGeom.getAttribute('position') as THREE.BufferAttribute;
      const arr = posAttr.array as Float32Array;
      for (let i = 0; i < PARTICLE_COUNT; i++) {
        const r = baseRadii[i]! + Math.sin(t * 1.2 + phases[i]!) * (0.06 + smoothLevel * 0.35);
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

      renderer.render(scene, camera);
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(frame);
      renderer.dispose();
      sphereGeom.dispose();
      sphereMat.dispose();
      particlesGeom.dispose();
      particlesMat.dispose();
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
