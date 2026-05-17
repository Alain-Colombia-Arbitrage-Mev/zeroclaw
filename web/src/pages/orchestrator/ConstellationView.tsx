// Constellation view — radial mission-topology canvas of the orchestrator
// hub, departmental orbits, and individual agent satellites, with a side
// telemetry strip (fleet status, subsystems, unit inspector, downlink
// ticker).
//
// Extracted from Orchestrator.tsx (was inline, ~930 LOC) without behaviour
// changes. The interface, CV layout constants, and buildConstellation()
// helper move with the component since nothing else uses them. Everything
// else this component touches — DEPARTMENTS, AgentActivity, ORCHESTRATOR_NAME,
// the *_META tables, and the side-panel sub-components (ConsolePanel,
// FleetRow, SubRow, UnitInspector) — is imported from the parent module
// where they remain co-located with their other consumers.

import { useEffect, useMemo, useState } from 'react';
import { Zap } from 'lucide-react';
import type { AgentInfo, Tenant } from '@/lib/api';
import { PixelSigil } from '@/components/PixelSigil';
import type { SSEEvent } from '@/types/api';
import {
  ACTIVITIES_META,
  CATEGORY_META,
  ConsolePanel,
  DEPARTMENTS,
  FleetRow,
  ORCHESTRATOR_NAME,
  STAGE_META,
  SubRow,
  UnitInspector,
  type AgentActivity,
} from '../Orchestrator';

// ─── Constellation view (radial mission topology) ──────────────────
//
// Orchestrator hub at the centre, departments orbit at the inner
// ring, individual agents at the outer arc within their department's
// angular sector. Lines from hub → agent pulse during activity.

interface ConstellationNode {
  name: string;
  agent: AgentInfo;
  dept: (typeof DEPARTMENTS)[number];
  x: number;
  y: number;
  ring: number;
}

const CV = {
  W: 1000,
  H: 1000,
  CX: 500,
  CY: 500,
  HUB_R: 56,
  DEPT_RADIUS: 220,
  DEPT_R: 30,
  AGENT_RING_NEAR: 350,
  AGENT_RING_FAR: 410,
  AGENT_R: 22,
  DEPT_ARC_PAD_DEG: 4,
};

function buildConstellation(
  agents: AgentInfo[],
): { nodes: ConstellationNode[]; depts: Array<{ dept: typeof DEPARTMENTS[number]; angle: number; presentCount: number }> } {
  const byName: Record<string, AgentInfo> = {};
  for (const a of agents) byName[a.name] = a;

  const nodes: ConstellationNode[] = [];
  const depts: Array<{
    dept: typeof DEPARTMENTS[number];
    angle: number;
    presentCount: number;
  }> = [];

  const N = DEPARTMENTS.length;
  const sectorDeg = 360 / N;

  DEPARTMENTS.forEach((dept, i) => {
    // Distribute department angles starting at top (-90°), going CW
    const deptAngleDeg = -90 + i * sectorDeg;
    const present = dept.agents.filter((n) => byName[n]);
    depts.push({ dept, angle: deptAngleDeg, presentCount: present.length });

    if (present.length === 0) return;

    const arcDeg = sectorDeg - 2 * CV.DEPT_ARC_PAD_DEG;
    // Angles for agents within this sector
    const startDeg = deptAngleDeg - arcDeg / 2;
    present.forEach((name, j) => {
      const t = present.length === 1 ? 0.5 : j / (present.length - 1);
      const angDeg = startDeg + t * arcDeg;
      const ring = j % 2 === 0 ? CV.AGENT_RING_NEAR : CV.AGENT_RING_FAR;
      const rad = (angDeg * Math.PI) / 180;
      nodes.push({
        name,
        agent: byName[name]!,
        dept,
        x: CV.CX + Math.cos(rad) * ring,
        y: CV.CY + Math.sin(rad) * ring,
        ring,
      });
    });
  });

  return { nodes, depts };
}

function ConstellationView({
  agents,
  activity,
  recommended,
  tenant,
  selected,
  onSelect,
  events,
  connected,
}: {
  agents: AgentInfo[];
  activity: Record<string, AgentActivity>;
  recommended: string[];
  tenant: Tenant | null;
  selected: string | null;
  onSelect: (name: string | null) => void;
  events: SSEEvent[];
  connected: boolean;
}) {
  const recSet = useMemo(() => new Set(recommended), [recommended]);
  const byName = useMemo(() => {
    const m: Record<string, AgentInfo> = {};
    for (const a of agents) m[a.name] = a;
    return m;
  }, [agents]);

  const { nodes, depts } = useMemo(
    () => buildConstellation(agents),
    [agents],
  );

  const stateCounts = useMemo(() => {
    const c = { idle: 0, running: 0, done: 0, error: 0 };
    for (const a of Object.values(activity)) {
      if (a.state in c) c[a.state]++;
    }
    return c;
  }, [activity]);

  const recentEvents = useMemo(() => events.slice(-12).reverse(), [events]);

  const totalOps = useMemo(
    () => Object.values(activity).reduce((s, a) => s + a.runs, 0),
    [activity],
  );

  // Events-per-minute over the last 60 s — telemetry-rate readout.
  const eventsPerMin = useMemo(() => {
    if (events.length === 0) return 0;
    const cutoff = Date.now() - 60_000;
    let n = 0;
    for (const e of events) {
      const t = (e as { _rxAt?: number })._rxAt;
      if (typeof t === 'number' && t >= cutoff) n++;
    }
    return n;
  }, [events]);

  // Last event age in seconds (used by SUBSYSTEMS readouts).
  const lastEventAge = useMemo<number | null>(() => {
    if (events.length === 0) return null;
    const last = events[events.length - 1];
    const t = (last as { _rxAt?: number })._rxAt;
    if (typeof t !== 'number') return null;
    return Math.max(0, (Date.now() - t) / 1000);
  }, [events]);

  // 1Hz tick so the SUBSYSTEMS LAST EVT counter ticks even when no new
  // events arrive (otherwise it'd appear frozen at the value computed
  // when events last changed).
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, []);

  const selAgent = selected ? byName[selected] : null;
  const selAct = selected ? activity[selected] : null;

  // Find the node coordinates for the selected agent (for highlight ring)
  const selNode = useMemo(
    () => nodes.find((n) => n.name === selected),
    [nodes, selected],
  );

  return (
    <div className="grid grid-cols-1 xl:grid-cols-[1fr_340px] gap-4">
      {/* ── Constellation canvas ───────────────────────────────── */}
      <div
        className="rounded border relative overflow-hidden"
        style={{
          background:
            'radial-gradient(ellipse at center, rgba(15, 23, 42, 0.6) 0%, rgba(3, 6, 12, 0.95) 75%)',
          borderColor: 'rgba(125, 211, 252, 0.18)',
          minHeight: '640px',
          height: 'calc(100vh - 280px)',
        }}
      >
        {/* Concentric grid background — subtle telemetry grid */}
        <svg
          viewBox={`0 0 ${CV.W} ${CV.H}`}
          className="absolute inset-0 w-full h-full"
          style={{ shapeRendering: 'geometricPrecision' }}
          preserveAspectRatio="xMidYMid meet"
        >
          <defs>
            <radialGradient id="hubglow" cx="50%" cy="50%" r="50%">
              <stop offset="0%" stopColor="#7DD3FC" stopOpacity="0.45" />
              <stop offset="60%" stopColor="#0EA5E9" stopOpacity="0.12" />
              <stop offset="100%" stopColor="#0EA5E9" stopOpacity="0" />
            </radialGradient>
            <filter id="softglow" x="-50%" y="-50%" width="200%" height="200%">
              <feGaussianBlur stdDeviation="3" result="b" />
              <feMerge>
                <feMergeNode in="b" />
                <feMergeNode in="SourceGraphic" />
              </feMerge>
            </filter>
          </defs>

          {/* Concentric rings + crosshair */}
          {[140, 220, 300, 380, 450].map((r) => (
            <circle
              key={r}
              cx={CV.CX}
              cy={CV.CY}
              r={r}
              fill="none"
              stroke="rgba(125, 211, 252, 0.06)"
              strokeWidth={1}
            />
          ))}
          <line
            x1={CV.CX} y1={50} x2={CV.CX} y2={CV.H - 50}
            stroke="rgba(125, 211, 252, 0.04)" strokeWidth={1}
          />
          <line
            x1={50} y1={CV.CY} x2={CV.W - 50} y2={CV.CY}
            stroke="rgba(125, 211, 252, 0.04)" strokeWidth={1}
          />

          {/* Tick marks every 10° on outer ring */}
          {Array.from({ length: 36 }).map((_, i) => {
            const a = (i * 10 * Math.PI) / 180;
            const r1 = 460;
            const r2 = i % 9 === 0 ? 478 : 470;
            return (
              <line
                key={i}
                x1={CV.CX + Math.cos(a) * r1}
                y1={CV.CY + Math.sin(a) * r1}
                x2={CV.CX + Math.cos(a) * r2}
                y2={CV.CY + Math.sin(a) * r2}
                stroke="rgba(125, 211, 252, 0.15)"
                strokeWidth={1}
              />
            );
          })}

          {/* Degree labels every 30° (0/30/60/.../330) — telemetry
              compass habit. 0° at the right per SVG convention. */}
          {Array.from({ length: 12 }).map((_, i) => {
            const deg = i * 30;
            const rad = (deg * Math.PI) / 180;
            const r = 448;
            return (
              <text
                key={`deg-${deg}`}
                x={CV.CX + Math.cos(rad) * r}
                y={CV.CY + Math.sin(rad) * r}
                textAnchor="middle"
                dominantBaseline="middle"
                fontFamily="ui-monospace, monospace"
                fontSize="9"
                letterSpacing="1.5"
                fill="rgba(125, 211, 252, 0.4)"
              >
                {deg.toString().padStart(3, '0')}
              </text>
            );
          })}

          {/* Scan sweep — NASA mission-plot radar. Uses SMIL
              <animateTransform> (instead of CSS keyframes) so it
              spins reliably across every browser regardless of
              transform-box / transform-origin quirks on SVG <g>.
              Two layers: a soft 60° trail + a sharp leading edge. */}
          <defs>
            <linearGradient id="sweep-grad" x1="0" y1="0" x2="1" y2="0">
              <stop offset="0%" stopColor="#7DD3FC" stopOpacity="0.0" />
              <stop offset="60%" stopColor="#7DD3FC" stopOpacity="0.10" />
              <stop offset="95%" stopColor="#7DD3FC" stopOpacity="0.32" />
              <stop offset="100%" stopColor="#BAE6FD" stopOpacity="0.55" />
            </linearGradient>
            <radialGradient id="sweep-glow" cx="50%" cy="50%" r="50%">
              <stop offset="0%" stopColor="#7DD3FC" stopOpacity="0.0" />
              <stop offset="80%" stopColor="#7DD3FC" stopOpacity="0.0" />
              <stop offset="100%" stopColor="#7DD3FC" stopOpacity="0.04" />
            </radialGradient>
          </defs>
          {/* Soft ambient halo (always visible) so the radar plane
              reads as "live", not just an empty axis grid. */}
          <circle
            cx={CV.CX}
            cy={CV.CY}
            r={460}
            fill="url(#sweep-glow)"
            pointerEvents="none"
          />
          {/* Rotating wedge with SMIL — animates regardless of CSS. */}
          <g pointerEvents="none">
            <path
              d={`M ${CV.CX} ${CV.CY} L ${CV.CX + 460} ${CV.CY - 75} A 460 460 0 0 1 ${CV.CX + 460} ${CV.CY + 75} Z`}
              fill="url(#sweep-grad)"
            />
            {/* Bright leading edge — the moving "beam" itself. */}
            <line
              x1={CV.CX}
              y1={CV.CY}
              x2={CV.CX + 460}
              y2={CV.CY}
              stroke="#BAE6FD"
              strokeOpacity={0.55}
              strokeWidth={1.2}
            />
            <animateTransform
              attributeName="transform"
              attributeType="XML"
              type="rotate"
              from={`0 ${CV.CX} ${CV.CY}`}
              to={`360 ${CV.CX} ${CV.CY}`}
              dur="8s"
              repeatCount="indefinite"
            />
          </g>

          {/* Department spokes — sector dividers */}
          {depts.map((d, i) => {
            const sectorDeg = 360 / depts.length;
            const sepDeg = -90 + i * sectorDeg + sectorDeg / 2;
            const rad = (sepDeg * Math.PI) / 180;
            return (
              <line
                key={`sep-${d.dept.id}`}
                x1={CV.CX + Math.cos(rad) * 130}
                y1={CV.CY + Math.sin(rad) * 130}
                x2={CV.CX + Math.cos(rad) * 460}
                y2={CV.CY + Math.sin(rad) * 460}
                stroke="rgba(125, 211, 252, 0.05)"
                strokeWidth={1}
                strokeDasharray="2 4"
              />
            );
          })}

          {/* Connection lines: hub → agent (tinted by dept, glowing if running) */}
          {nodes.map((n) => {
            const act = activity[n.name];
            const running = act?.state === 'running';
            const error = act?.state === 'error';
            const dim = tenant && !recSet.has(n.name);
            const isSel = selected === n.name;
            const lineColor = error
              ? '#F87171'
              : running
                ? n.dept.accent
                : n.dept.accent;
            const baseOpacity = dim ? 0.06 : isSel ? 0.7 : running ? 0.55 : 0.18;
            return (
              <g key={`l-${n.name}`}>
                <line
                  x1={CV.CX}
                  y1={CV.CY}
                  x2={n.x}
                  y2={n.y}
                  stroke={lineColor}
                  strokeWidth={running || isSel ? 1.5 : 0.8}
                  strokeOpacity={baseOpacity}
                />
                {running && (
                  <line
                    x1={CV.CX}
                    y1={CV.CY}
                    x2={n.x}
                    y2={n.y}
                    stroke={lineColor}
                    strokeWidth={2}
                    strokeOpacity={0.9}
                    strokeDasharray="6 12"
                    style={{ animation: 'cv-flow 1.4s linear infinite' }}
                  />
                )}
              </g>
            );
          })}

          {/* Department badges */}
          {depts.map((d) => {
            const rad = (d.angle * Math.PI) / 180;
            const x = CV.CX + Math.cos(rad) * CV.DEPT_RADIUS;
            const y = CV.CY + Math.sin(rad) * CV.DEPT_RADIUS;
            const running = d.dept.agents.some(
              (n) => activity[n]?.state === 'running',
            );
            return (
              <g key={`d-${d.dept.id}`}>
                <circle
                  cx={x}
                  cy={y}
                  r={CV.DEPT_R}
                  fill="rgba(3, 6, 12, 0.95)"
                  stroke={d.dept.accent}
                  strokeOpacity={running ? 0.9 : 0.55}
                  strokeWidth={running ? 2 : 1.2}
                />
                <circle
                  cx={x}
                  cy={y}
                  r={CV.DEPT_R + 5}
                  fill="none"
                  stroke={d.dept.accent}
                  strokeOpacity={running ? 0.55 : 0}
                  strokeWidth={1.5}
                  style={
                    running
                      ? {
                          animation: 'cv-ping 1.6s ease-out infinite',
                          transformBox: 'fill-box',
                          transformOrigin: 'center',
                        }
                      : undefined
                  }
                />
                <text
                  x={x}
                  y={y - 2}
                  textAnchor="middle"
                  fontFamily="ui-monospace, monospace"
                  fontSize="13"
                  fontWeight="700"
                  letterSpacing="2.5"
                  fill={d.dept.accent}
                >
                  {d.dept.callsign}
                </text>
                <text
                  x={x}
                  y={y + 12}
                  textAnchor="middle"
                  fontFamily="ui-monospace, monospace"
                  fontSize="8"
                  letterSpacing="1.5"
                  fill="#94A3B8"
                >
                  N={d.presentCount}
                </text>
              </g>
            );
          })}

          {/* Selected highlight ring */}
          {selNode && (
            <>
              {/* Outer expanding halo (cv-ping). */}
              <circle
                cx={selNode.x}
                cy={selNode.y}
                r={CV.AGENT_R + 6}
                fill="none"
                stroke={selNode.dept.accent}
                strokeOpacity={0.95}
                strokeWidth={2}
                style={{
                  animation: 'cv-ping 1.4s ease-out infinite',
                  transformBox: 'fill-box',
                  transformOrigin: 'center',
                }}
              />
              {/* Static highlight ring underneath so the selection
                  reads even between ping cycles. */}
              <circle
                cx={selNode.x}
                cy={selNode.y}
                r={CV.AGENT_R + 4}
                fill="none"
                stroke={selNode.dept.accent}
                strokeOpacity={0.8}
                strokeWidth={1.5}
                strokeDasharray="3 2"
              />
            </>
          )}

          {/* Agent satellites */}
          {nodes.map((n) => {
            const act = activity[n.name];
            const state = act?.state ?? 'idle';
            const dim = tenant && !recSet.has(n.name);
            const isSel = selected === n.name;
            const ringColor =
              state === 'error'
                ? '#F87171'
                : state === 'running'
                  ? n.dept.accent
                  : state === 'done'
                    ? '#86EFAC'
                    : n.dept.accent;
            const running = state === 'running';
            return (
              <g
                key={`a-${n.name}`}
                style={{ cursor: 'pointer', opacity: dim ? 0.32 : 1 }}
                onClick={() => onSelect(isSel ? null : n.name)}
              >
                {/* Hover hit area */}
                <circle
                  cx={n.x}
                  cy={n.y}
                  r={CV.AGENT_R + 6}
                  fill="transparent"
                />
                {/* RUNNING — expanding ping ring (cv-ping) makes the
                    agent unmistakably alive. Sits BEHIND the satellite
                    so it doesn't obscure the sigil. */}
                {running && (
                  <circle
                    cx={n.x}
                    cy={n.y}
                    r={CV.AGENT_R + 4}
                    fill="none"
                    stroke={ringColor}
                    strokeOpacity={0.8}
                    strokeWidth={2}
                    style={{
                      animation: 'cv-ping 1.3s ease-out infinite',
                      transformBox: 'fill-box',
                      transformOrigin: 'center',
                    }}
                  />
                )}
                {/* RUNNING — second ring offset for double-pulse effect */}
                {running && (
                  <circle
                    cx={n.x}
                    cy={n.y}
                    r={CV.AGENT_R + 4}
                    fill="none"
                    stroke={ringColor}
                    strokeOpacity={0.6}
                    strokeWidth={1.5}
                    style={{
                      animation: 'cv-ping 1.3s ease-out infinite',
                      animationDelay: '0.65s',
                      transformBox: 'fill-box',
                      transformOrigin: 'center',
                    }}
                  />
                )}
                <circle
                  cx={n.x}
                  cy={n.y}
                  r={CV.AGENT_R}
                  fill={running ? `${ringColor}1c` : 'rgba(3, 6, 12, 0.95)'}
                  stroke={ringColor}
                  strokeOpacity={isSel ? 1 : running ? 0.95 : 0.45}
                  strokeWidth={isSel ? 2 : running ? 2.2 : 1}
                  style={
                    running
                      ? { color: ringColor, animation: 'cv-running-glow 1.4s ease-in-out infinite' }
                      : undefined
                  }
                />
                {/* Embed sigil in the satellite */}
                <foreignObject
                  x={n.x - 16}
                  y={n.y - 16}
                  width="32"
                  height="32"
                  style={{ pointerEvents: 'none' }}
                >
                  <div
                    style={{
                      width: 32,
                      height: 32,
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                    }}
                  >
                    <PixelSigil name={n.name} size={28} state={state} />
                  </div>
                </foreignObject>
                {/* Name label */}
                <text
                  x={n.x}
                  y={n.y + CV.AGENT_R + 12}
                  textAnchor="middle"
                  fontFamily="ui-monospace, monospace"
                  fontSize="9"
                  letterSpacing="0.5"
                  fill={isSel ? '#E0F2FE' : state === 'running' ? '#BAE6FD' : '#94A3B8'}
                >
                  {n.name.length > 16 ? n.name.slice(0, 15) + '…' : n.name}
                </text>
                {/* RUNNING — corner LED. Bright + pulsing so the
                    glance-test confirms "this satellite is alive". */}
                {running && (
                  <circle
                    cx={n.x + CV.AGENT_R - 3}
                    cy={n.y - CV.AGENT_R + 3}
                    r="3.5"
                    fill={ringColor}
                    style={{
                      animation: 'cv-ping 1.0s ease-out infinite',
                      transformBox: 'fill-box',
                      transformOrigin: 'center',
                      filter: `drop-shadow(0 0 4px ${ringColor})`,
                    }}
                  />
                )}
              </g>
            );
          })}

          {/* Hub: orchestrator core */}
          <circle
            cx={CV.CX}
            cy={CV.CY}
            r={CV.HUB_R + 30}
            fill="url(#hubglow)"
          />
          <circle
            cx={CV.CX}
            cy={CV.CY}
            r={CV.HUB_R}
            fill="rgba(3, 6, 12, 0.98)"
            stroke="#7DD3FC"
            strokeOpacity="0.7"
            strokeWidth="1.5"
          />
          <circle
            cx={CV.CX}
            cy={CV.CY}
            r={CV.HUB_R - 6}
            fill="none"
            stroke="rgba(125, 211, 252, 0.4)"
            strokeWidth="1"
            strokeDasharray="2 3"
          />
          <foreignObject
            x={CV.CX - 30}
            y={CV.CY - 30}
            width="60"
            height="60"
            style={{ pointerEvents: 'none' }}
          >
            <div
              style={{
                width: 60,
                height: 60,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
            >
              <PixelSigil
                name={ORCHESTRATOR_NAME}
                size={48}
                state={stateCounts.running > 0 ? 'running' : 'idle'}
              />
            </div>
          </foreignObject>
          <text
            x={CV.CX}
            y={CV.CY + CV.HUB_R + 18}
            textAnchor="middle"
            fontFamily="ui-monospace, monospace"
            fontSize="10"
            letterSpacing="2.5"
            fontWeight="600"
            fill="#7DD3FC"
          >
            ORCHESTRATOR
          </text>
        </svg>

        {/* Local CSS keyframes for the constellation.
            Critical: SVG <circle> elements scale from (0,0) by
            default. We force `transform-box: fill-box` +
            `transform-origin: center` on every animated element so
            the visual actually pulses around its own centre instead
            of from the SVG origin. Without this fix the "running"
            satellite looked completely static. */}
        <style>{`
          @keyframes cv-flow {
            from { stroke-dashoffset: 0; }
            to   { stroke-dashoffset: -36; }
          }
          @keyframes cv-ping {
            0%   { transform: scale(1);   opacity: 0.85; }
            70%  { transform: scale(1.8); opacity: 0.15; }
            100% { transform: scale(2.2); opacity: 0; }
          }
          @keyframes cv-pulse-stroke {
            0%, 100% { stroke-opacity: 0.45; stroke-width: 1.2; }
            50%      { stroke-opacity: 1;    stroke-width: 2.8; }
          }
          @keyframes cv-sweep {
            from { transform: rotate(0deg); }
            to   { transform: rotate(360deg); }
          }
          @keyframes cv-running-glow {
            0%, 100% { filter: drop-shadow(0 0 2px currentColor); }
            50%      { filter: drop-shadow(0 0 10px currentColor); }
          }
          /* Force SVG elements to scale + rotate around their own
             centre, not the SVG (0,0) origin. */
          .cv-anim-center {
            transform-box: fill-box;
            transform-origin: center;
          }
        `}</style>

        {/* CRT scanlines overlay — sub-perceptual, adds the
            phosphor / line-store feel without harming readability. */}
        <div
          className="absolute inset-0 pointer-events-none"
          style={{
            background:
              'repeating-linear-gradient(0deg, rgba(125, 211, 252, 0.025) 0 1px, transparent 1px 3px)',
            mixBlendMode: 'overlay',
          }}
        />
        {/* Vignette — subtle screen edge shading */}
        <div
          className="absolute inset-0 pointer-events-none"
          style={{
            background:
              'radial-gradient(ellipse at center, transparent 55%, rgba(0, 0, 0, 0.5) 100%)',
          }}
        />

        {/* Floating legend (top-left) */}
        <div
          className="absolute top-3 left-3 px-2.5 py-1.5 rounded-sm border text-[9px]"
          style={{
            background: 'rgba(3, 6, 12, 0.85)',
            borderColor: 'rgba(125, 211, 252, 0.25)',
            color: '#5BA8D9',
            fontFamily: 'ui-monospace, monospace',
            letterSpacing: '0.25em',
          }}
        >
          {tenant ? `MISSION · ${tenant.name.toUpperCase()}` : 'NO MISSION · ALL UNITS'}
        </div>
      </div>

      {/* ── Telemetry side panel ───────────────────────────────── */}
      <aside className="space-y-3 self-start">
        {/* Mission brief — only when active tenant has a description */}
        {tenant && tenant.description && tenant.description.trim() && (
          <div
            className="rounded border"
            style={{
              background: 'rgba(12, 16, 24, 0.85)',
              borderColor: CATEGORY_META[tenant.category].tint + '55',
              borderLeft: `3px solid ${CATEGORY_META[tenant.category].tint}`,
            }}
          >
            <div
              className="px-3 py-2 border-b text-[10px] flex items-center justify-between"
              style={{
                borderColor: 'rgba(125, 211, 252, 0.1)',
                color: CATEGORY_META[tenant.category].tint,
                fontFamily: 'ui-monospace, monospace',
                letterSpacing: '0.3em',
              }}
            >
              <span>MISSION BRIEF</span>
              <span style={{ color: '#5BA8D9' }}>
                {STAGE_META[tenant.stage].label}
              </span>
            </div>
            <div className="px-3 py-2.5">
              {tenant.mission && tenant.mission.trim() && (
                <p
                  className="text-[11px] mb-2 italic"
                  style={{
                    color: '#E0F2FE',
                    fontFamily: 'ui-monospace, monospace',
                    letterSpacing: '0.04em',
                    lineHeight: 1.4,
                  }}
                >
                  “{tenant.mission}”
                </p>
              )}
              {tenant.activities && tenant.activities.length > 0 && (
                <div className="flex flex-wrap gap-1 mb-2">
                  {tenant.activities.map((a) => {
                    const meta = ACTIVITIES_META[a];
                    return (
                      <span
                        key={a}
                        className="text-[9px] px-1.5 py-0.5"
                        style={{
                          background: meta.tint + '14',
                          border: `1px solid ${meta.tint}55`,
                          color: meta.tint,
                          fontFamily: 'ui-monospace, monospace',
                          letterSpacing: '0.18em',
                          fontWeight: 600,
                        }}
                        title={meta.description}
                      >
                        ◆ {meta.label}
                      </span>
                    );
                  })}
                </div>
              )}
              <p
                className="text-[10px] whitespace-pre-wrap"
                style={{
                  color: '#CBD5E1',
                  fontFamily: 'ui-monospace, monospace',
                  letterSpacing: '0.02em',
                  lineHeight: 1.55,
                  maxHeight: 220,
                  overflowY: 'auto',
                }}
              >
                {tenant.description}
              </p>
            </div>
          </div>
        )}

        {/* PNL-A · FLEET STATUS — tabular */}
        <ConsolePanel id="PNL-A" title="FLEET STATUS">
          <table className="w-full text-[10px] tabular-nums" style={{ fontFamily: 'ui-monospace, monospace' }}>
            <thead>
              <tr style={{ color: '#5BA8D9' }}>
                <th className="text-left px-3 py-1 font-normal" style={{ letterSpacing: '0.25em' }}>STATE</th>
                <th className="text-right px-3 py-1 font-normal" style={{ letterSpacing: '0.25em' }}>COUNT</th>
                <th className="text-right px-3 py-1 font-normal" style={{ letterSpacing: '0.25em' }}>BAR</th>
              </tr>
            </thead>
            <tbody>
              <FleetRow label="IDLE" value={stateCounts.idle} total={agents.length} color="#5BA8D9" />
              <FleetRow label="RUN " value={stateCounts.running} total={agents.length} color="#7DD3FC" pulse />
              <FleetRow label="DONE" value={stateCounts.done} total={agents.length} color="#86EFAC" />
              <FleetRow label="ERR " value={stateCounts.error} total={agents.length} color="#F87171" />
            </tbody>
          </table>
        </ConsolePanel>

        {/* PNL-B · SUBSYSTEMS — engineering readouts */}
        <ConsolePanel id="PNL-B" title="SUBSYSTEMS">
          <table className="w-full text-[10px] tabular-nums" style={{ fontFamily: 'ui-monospace, monospace' }}>
            <tbody>
              <SubRow label="DOWNLINK"  value={`${eventsPerMin.toFixed(1)} EVT/MIN`} status={connected && eventsPerMin > 0 ? 'go' : connected ? 'idle' : 'red'} />
              <SubRow label="LAST EVT"  value={lastEventAge === null ? '— NIL —' : `${Math.round(lastEventAge)} S`} status={lastEventAge === null ? 'idle' : lastEventAge < 30 ? 'go' : lastEventAge < 120 ? 'amber' : 'red'} />
              <SubRow label="QUEUE"     value={`${stateCounts.running} ACTIVE`} status={stateCounts.running > 4 ? 'amber' : stateCounts.running > 0 ? 'go' : 'idle'} />
              <SubRow label="OPS TOTAL" value={`${totalOps}`} status="info" />
              <SubRow label="BUFFER"    value={`${events.length} / 200`} status={events.length > 180 ? 'amber' : 'info'} />
            </tbody>
          </table>
        </ConsolePanel>

        {/* PNL-C · UNIT INSPECTOR */}
        <ConsolePanel id="PNL-C" title="UNIT INSPECTOR">
          {!selAgent ? (
            <p
              className="text-xs px-3 py-3"
              style={{
                color: '#5BA8D9',
                fontFamily: 'ui-monospace, monospace',
                letterSpacing: '0.1em',
              }}
            >
              ▸ TAP A SATELLITE
            </p>
          ) : (
            <div className="px-3 py-3">
              <UnitInspector agent={selAgent} activity={selAct} />
            </div>
          )}
        </ConsolePanel>

        {/* PNL-D · DOWNLINK ticker */}
        <div
          className="rounded border"
          style={{
            background: 'rgba(12, 16, 24, 0.85)',
            borderColor: 'rgba(125, 211, 252, 0.18)',
          }}
        >
          <div
            className="px-3 py-2 border-b text-[10px] flex items-center justify-between"
            style={{
              borderColor: 'rgba(125, 211, 252, 0.1)',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.3em',
            }}
          >
            <span className="flex items-center gap-1.5" style={{ color: '#5BA8D9' }}>
              <Zap className="h-3 w-3" /> PNL-D · DOWNLINK
            </span>
            <span style={{ color: '#94A3B8', fontSize: 9, letterSpacing: '0.15em' }}>
              S-BAND
            </span>
          </div>
          <ul
            className="text-[10px] max-h-72 overflow-y-auto"
            style={{ fontFamily: 'ui-monospace, monospace' }}
          >
            {recentEvents.length === 0 ? (
              <li className="px-3 py-3 text-center" style={{ color: '#5BA8D9', letterSpacing: '0.15em' }}>
                — NO TRAFFIC —
              </li>
            ) : (
              recentEvents.map((e, i) => {
                const isError = e.type === 'error' || e.success === false;
                const isDone = e.type === 'agent_end' || (e.type === 'tool_call' && e.success !== false);
                const color = isError ? '#F87171' : isDone ? '#86EFAC' : '#7DD3FC';
                const target =
                  (e.target_agent as string | undefined) ??
                  (Array.isArray(e.target_agents)
                    ? (e.target_agents as string[])[0]
                    : undefined);
                return (
                  <li
                    key={i}
                    className="px-3 py-1.5 border-b last:border-0 flex gap-2 items-baseline"
                    style={{ borderColor: 'rgba(125, 211, 252, 0.05)' }}
                  >
                    <span style={{ color: '#5BA8D9' }}>{(e.timestamp ?? '').slice(11, 19)}</span>
                    <span style={{ color, letterSpacing: '0.1em', textTransform: 'uppercase', fontSize: 9 }}>
                      {e.type.replace('_', ' ')}
                    </span>
                    {target && (
                      <span className="truncate" style={{ color: '#94A3B8' }}>
                        {target}
                      </span>
                    )}
                  </li>
                );
              })
            )}
          </ul>
        </div>
      </aside>
    </div>
  );
}

export default ConstellationView;
