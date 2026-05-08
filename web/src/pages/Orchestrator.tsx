import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Activity,
  Bot,
  Network,
  Clock,
  Zap,
  AlertCircle,
  CheckCircle2,
} from 'lucide-react';
import { getAgents, type AgentInfo } from '@/lib/api';
import { SSEClient } from '@/lib/sse';
import type { SSEEvent } from '@/types/api';

type Tab = 'graph' | 'live' | 'timeline';

interface AgentActivity {
  name: string;
  state: 'idle' | 'running' | 'done' | 'error';
  lastEventAt: number | null;
  runs: number;
  totalDurationMs: number;
}

const ORCHESTRATOR_NAME = 'orchestrator';
const STATE_DECAY_MS = 4000; // running → idle if no event for this long

function eventAgentNames(e: SSEEvent): string[] {
  // 1. Explicit target_agent / target_agents from the SSE forwarder
  if (Array.isArray(e.target_agents)) {
    return e.target_agents.filter((s: unknown): s is string => typeof s === 'string');
  }
  if (typeof e.target_agent === 'string') return [e.target_agent];
  // 2. Generic agent / model fallback
  if (typeof e.agent === 'string') return [e.agent];
  if (typeof e.model === 'string') return [ORCHESTRATOR_NAME];
  return [];
}

export default function Orchestrator() {
  const [tab, setTab] = useState<Tab>(() => {
    try {
      return (localStorage.getItem('octopus_orch_tab') as Tab) || 'graph';
    } catch {
      return 'graph';
    }
  });
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [activity, setActivity] = useState<Record<string, AgentActivity>>({});
  const [events, setEvents] = useState<SSEEvent[]>([]);
  const [connected, setConnected] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 800, h: 600 });

  // Persist tab
  useEffect(() => {
    try {
      localStorage.setItem('octopus_orch_tab', tab);
    } catch {
      /* ignore */
    }
  }, [tab]);

  // Fetch agent catalog
  useEffect(() => {
    getAgents()
      .then((list) => {
        setAgents(list);
        const initial: Record<string, AgentActivity> = {};
        for (const a of list) {
          initial[a.name] = {
            name: a.name,
            state: 'idle',
            lastEventAt: null,
            runs: 0,
            totalDurationMs: 0,
          };
        }
        initial[ORCHESTRATOR_NAME] = {
          name: ORCHESTRATOR_NAME,
          state: 'idle',
          lastEventAt: null,
          runs: 0,
          totalDurationMs: 0,
        };
        setActivity(initial);
      })
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }, []);

  // Container size for graph
  useEffect(() => {
    const el = containerRef.current;
    if (!el || tab !== 'graph') return;
    const measure = () => {
      const r = el.getBoundingClientRect();
      setSize({ w: Math.max(320, r.width), h: Math.max(360, r.height) });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [tab]);

  // SSE connection
  useEffect(() => {
    const client = new SSEClient();
    client.onConnect = () => setConnected(true);
    client.onError = () => setConnected(false);
    client.onEvent = (e) => {
      setEvents((prev) => {
        const next = [...prev, { ...e, _rxAt: Date.now() } as SSEEvent].slice(-200);
        return next;
      });

      const names = eventAgentNames(e);
      if (names.length === 0) return;
      setActivity((prev) => {
        const next = { ...prev };
        for (const name of names) {
          const cur = next[name] ?? {
            name,
            state: 'idle' as const,
            lastEventAt: null,
            runs: 0,
            totalDurationMs: 0,
          };
          let nextState: AgentActivity['state'] = cur.state;
          let runs = cur.runs;
          let total = cur.totalDurationMs;
          if (
            e.type === 'agent_start' ||
            e.type === 'tool_call_start' ||
            e.type === 'llm_request'
          ) {
            nextState = 'running';
          } else if (e.type === 'agent_end') {
            nextState = 'done';
            runs += 1;
            if (typeof e.duration_ms === 'number') total += e.duration_ms;
          } else if (e.type === 'tool_call') {
            nextState = e.success === false ? 'error' : 'done';
            runs += 1;
            if (typeof e.duration_ms === 'number') total += e.duration_ms;
          } else if (e.type === 'error') {
            nextState = 'error';
          }
          next[name] = {
            ...cur,
            state: nextState,
            lastEventAt: Date.now(),
            runs,
            totalDurationMs: total,
          };
        }
        return next;
      });
    };
    client.connect();
    return () => client.disconnect();
  }, []);

  // Decay running → idle when no event for STATE_DECAY_MS
  useEffect(() => {
    const interval = setInterval(() => {
      setActivity((prev) => {
        let changed = false;
        const next = { ...prev };
        for (const k in next) {
          const a = next[k];
          if (!a) continue;
          if (
            a.state === 'running' &&
            a.lastEventAt !== null &&
            Date.now() - a.lastEventAt > STATE_DECAY_MS
          ) {
            next[k] = { ...a, state: 'idle' };
            changed = true;
          }
        }
        return changed ? next : prev;
      });
    }, 1500);
    return () => clearInterval(interval);
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div
          className="h-8 w-8 border-2 rounded-full animate-spin"
          style={{ borderColor: 'var(--pc-border)', borderTopColor: 'var(--pc-accent)' }}
        />
      </div>
    );
  }

  const runningCount = Object.values(activity).filter((a) => a.state === 'running').length;
  const totalRuns = Object.values(activity).reduce((s, a) => s + a.runs, 0);
  const runningAgents = Object.values(activity)
    .filter((a) => a.state === 'running')
    .map((a) => a.name);
  const hasEverFiredEvent = events.length > 0;

  return (
    <div className="p-6 space-y-5 animate-fade-in" style={{ background: '#03060c' }}>
      {/* META-FORGE style header bar */}
      <div
        className="flex items-end justify-between gap-4 flex-wrap pb-4 border-b"
        style={{ borderColor: 'rgba(125, 211, 252, 0.12)' }}
      >
        <div>
          <p
            className="text-[10px] mb-1.5"
            style={{
              color: '#5BA8D9',
              fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
              letterSpacing: '0.4em',
            }}
          >
            OCTOPUS &nbsp;·&nbsp; AGENT MESH
          </p>
          <h1
            className="text-3xl flex items-center gap-3"
            style={{
              color: '#BAE6FD',
              fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
              letterSpacing: '0.18em',
              fontWeight: 500,
            }}
          >
            <Zap className="h-7 w-7" style={{ color: '#7DD3FC' }} />
            REACTOR CORE
          </h1>
          <p
            className="text-xs mt-2"
            style={{
              color: '#5BA8D9',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.1em',
            }}
          >
            {agents.length} sub-agents · {runningCount} running · {totalRuns} runs · SSE {connected ? 'online' : 'reconnecting'}
          </p>
          {runningAgents.length > 0 && (
            <p
              className="text-xs mt-2 inline-flex items-center gap-2 px-2 py-1 rounded"
              style={{
                color: '#BAE6FD',
                background: 'rgba(125, 211, 252, 0.08)',
                border: '1px solid rgba(125, 211, 252, 0.3)',
                fontFamily: 'ui-monospace, monospace',
                letterSpacing: '0.1em',
                boxShadow: '0 0 12px rgba(125, 211, 252, 0.2)',
              }}
            >
              <span
                className="inline-block h-1.5 w-1.5 rounded-full"
                style={{
                  background: '#7DD3FC',
                  boxShadow: '0 0 6px #7DD3FC',
                  animation: 'pulse 1.4s ease-in-out infinite',
                }}
              />
              EXECUTING → {runningAgents.join(' + ')}
            </p>
          )}
        </div>
        <div className="flex items-center gap-3 flex-wrap">
          <span
            className="inline-flex items-center gap-2 text-[10px] px-2.5 py-1.5 rounded border"
            style={{
              color: connected ? '#7DD3FC' : '#5BA8D9',
              borderColor: connected ? 'rgba(125, 211, 252, 0.4)' : 'rgba(91, 168, 217, 0.2)',
              background: connected ? 'rgba(125, 211, 252, 0.06)' : 'transparent',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.2em',
            }}
          >
            <span
              className="inline-block h-1.5 w-1.5 rounded-full"
              style={{
                background: connected ? '#7DD3FC' : '#5BA8D9',
                boxShadow: connected ? '0 0 8px #7DD3FC' : undefined,
                animation: connected ? 'pulse 2s ease-in-out infinite' : undefined,
              }}
            />
            {connected ? 'LIVE' : 'WAIT'}
          </span>
          <div
            className="inline-flex rounded p-0.5 border"
            style={{
              background: 'rgba(12, 16, 24, 0.7)',
              borderColor: 'rgba(125, 211, 252, 0.2)',
            }}
            role="tablist"
          >
            {(
              [
                ['graph', 'GRAPH', Network],
                ['live', 'LIVE', Activity],
                ['timeline', 'TIMELINE', Clock],
              ] as const
            ).map(([id, label, Icon]) => {
              const active = tab === id;
              return (
                <button
                  key={id}
                  role="tab"
                  aria-selected={active}
                  onClick={() => setTab(id)}
                  className="px-3 py-1.5 text-[10px] inline-flex items-center gap-1.5 transition-all"
                  style={{
                    background: active ? 'rgba(125, 211, 252, 0.12)' : 'transparent',
                    color: active ? '#BAE6FD' : '#5BA8D9',
                    border: active
                      ? '1px solid rgba(125, 211, 252, 0.4)'
                      : '1px solid transparent',
                    fontFamily: 'ui-monospace, monospace',
                    letterSpacing: '0.2em',
                    boxShadow: active ? '0 0 12px rgba(125, 211, 252, 0.15)' : undefined,
                  }}
                >
                  <Icon className="h-3 w-3" /> {label}
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {error && (
        <div
          className="rounded border p-4"
          style={{
            background: 'rgba(239, 68, 68, 0.08)',
            borderColor: 'rgba(239, 68, 68, 0.2)',
            color: '#f87171',
          }}
        >
          {error}
        </div>
      )}

      {tab === 'graph' && (
        <div
          ref={containerRef}
          className="rounded border relative overflow-hidden"
          style={{
            background:
              'radial-gradient(ellipse at center, rgba(91, 168, 217, 0.05) 0%, rgba(3, 6, 12, 1) 70%)',
            borderColor: 'rgba(125, 211, 252, 0.15)',
            height: 'calc(100vh - 240px)',
            minHeight: '520px',
          }}
        >
          <ReactorCore agents={agents} activity={activity} width={size.w} height={size.h} />
          {!hasEverFiredEvent && (
            <div
              className="absolute bottom-4 left-1/2 -translate-x-1/2 px-4 py-3 rounded border max-w-md text-center pointer-events-none"
              style={{
                background: 'rgba(3, 6, 12, 0.85)',
                borderColor: 'rgba(125, 211, 252, 0.2)',
                color: '#5BA8D9',
                fontFamily: 'ui-monospace, monospace',
                letterSpacing: '0.1em',
                fontSize: '11px',
              }}
            >
              <div style={{ color: '#BAE6FD', marginBottom: '4px' }}>
                STANDBY · NO ACTIVITY
              </div>
              <div>
                Open <span style={{ color: '#7DD3FC' }}>/agent</span> and ask the
                orchestrator to delegate. The reactor will light up the
                sub-agent it routes to.
              </div>
            </div>
          )}
        </div>
      )}

      {tab === 'live' && <LiveView events={events} activity={activity} agents={agents} />}

      {tab === 'timeline' && <TimelineView events={events} agents={agents} />}
    </div>
  );
}

// ─── Reactor Core (radial graph in META-FORGE language) ─────────────────

function ReactorCore({
  agents,
  activity,
  width,
  height,
}: {
  agents: AgentInfo[];
  activity: Record<string, AgentActivity>;
  width: number;
  height: number;
}) {
  const cx = width / 2;
  const cy = height / 2 - 20; // shift up to leave room for label
  const radius = Math.min(width, height) * 0.36;

  const placed = useMemo(() => {
    return agents.map((agent, i) => {
      const angle = (i / agents.length) * Math.PI * 2 - Math.PI / 2;
      return {
        agent,
        angle,
        x: cx + Math.cos(angle) * radius,
        y: cy + Math.sin(angle) * radius,
      };
    });
  }, [agents, cx, cy, radius]);

  const stateColor = (s: AgentActivity['state']) => {
    switch (s) {
      case 'running':
        return '#7DD3FC';
      case 'done':
        return '#86EFAC';
      case 'error':
        return '#F87171';
      default:
        return '#5BA8D9';
    }
  };

  const orchestratorState = activity[ORCHESTRATOR_NAME]?.state ?? 'idle';
  const coreActive =
    orchestratorState === 'running' ||
    Object.values(activity).some((a) => a.state === 'running');

  // Curved bezier from core to node — outward-bulging arc that reads like
  // an arm of the META-FORGE reactor.
  const armPath = (x: number, y: number, angle: number) => {
    const startR = 110;
    const sx = cx + Math.cos(angle) * startR;
    const sy = cy + Math.sin(angle) * startR;
    // Control point pulled tangentially for a slight curve
    const tangent = angle + Math.PI / 2;
    const bulge = 24;
    const mx = (sx + x) / 2 + Math.cos(tangent) * bulge;
    const my = (sy + y) / 2 + Math.sin(tangent) * bulge;
    return `M ${sx} ${sy} Q ${mx} ${my} ${x} ${y}`;
  };

  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`}>
      <defs>
        <radialGradient id="core-halo" cx="50%" cy="50%">
          <stop offset="0%" stopColor="#7DD3FC" stopOpacity="0.45" />
          <stop offset="60%" stopColor="#5BA8D9" stopOpacity="0.08" />
          <stop offset="100%" stopColor="#7DD3FC" stopOpacity="0" />
        </radialGradient>
        <radialGradient id="core-fill" cx="50%" cy="40%">
          <stop offset="0%" stopColor="#1a2838" stopOpacity="0.95" />
          <stop offset="100%" stopColor="#0c1018" stopOpacity="1" />
        </radialGradient>
        <pattern id="scan-lines" width="2" height="3" patternUnits="userSpaceOnUse">
          <rect width="2" height="3" fill="none" />
          <line x1="0" y1="0" x2="2" y2="0" stroke="#7DD3FC" strokeOpacity="0.04" />
        </pattern>
      </defs>

      {/* Outer halo */}
      <circle cx={cx} cy={cy} r={radius * 0.95} fill="url(#core-halo)">
        {coreActive && (
          <animate
            attributeName="r"
            values={`${radius * 0.9};${radius * 1.0};${radius * 0.9}`}
            dur="3.2s"
            repeatCount="indefinite"
          />
        )}
      </circle>

      {/* Subtle scan-line wash */}
      <rect
        x={cx - radius * 1.2}
        y={cy - radius * 1.2}
        width={radius * 2.4}
        height={radius * 2.4}
        fill="url(#scan-lines)"
        opacity="0.6"
      />

      {/* Arm spokes (curved bezier) */}
      {placed.map(({ agent, x, y, angle }) => {
        const a = activity[agent.name];
        const isActive = a?.state === 'running';
        return (
          <path
            key={`arm-${agent.name}`}
            d={armPath(x, y, angle)}
            fill="none"
            stroke={isActive ? '#7DD3FC' : '#1a2130'}
            strokeWidth={isActive ? 2 : 1.4}
            strokeLinecap="round"
          >
            {isActive && (
              <animate
                attributeName="stroke-opacity"
                values="0.4;1;0.4"
                dur="1.4s"
                repeatCount="indefinite"
              />
            )}
          </path>
        );
      })}

      {/* Outer reactor ring */}
      <ellipse
        cx={cx}
        cy={cy}
        rx={130}
        ry={108}
        fill="url(#core-fill)"
        stroke="#5BA8D9"
        strokeOpacity="0.5"
        strokeWidth={2}
      />

      {/* Inner cavity */}
      <ellipse
        cx={cx}
        cy={cy - 18}
        rx={95}
        ry={70}
        fill="#1a2838"
        fillOpacity="0.7"
        stroke="#7DD3FC"
        strokeOpacity="0.4"
        strokeWidth={1.5}
      />

      {/* Two intake vents (top of inner cavity) */}
      <rect x={cx - 45} y={cy} width={32} height={3.5} rx={1.5} fill="#BAE6FD" />
      <rect x={cx + 13} y={cy} width={32} height={3.5} rx={1.5} fill="#BAE6FD" />

      {/* Core eye — the orchestrator */}
      <circle cx={cx} cy={cy - 18} r={18} fill="#03060c" />
      <circle
        cx={cx}
        cy={cy - 18}
        r={14}
        fill="none"
        stroke={stateColor(orchestratorState)}
        strokeWidth={1.5}
      >
        {coreActive && (
          <animate
            attributeName="r"
            values="13;17;13"
            dur="2s"
            repeatCount="indefinite"
          />
        )}
      </circle>
      <circle
        cx={cx}
        cy={cy - 18}
        r={5}
        fill={stateColor(orchestratorState)}
        opacity={coreActive ? 1 : 0.6}
      />

      {/* Central label inside the reactor */}
      <text
        x={cx}
        y={cy + 50}
        textAnchor="middle"
        fontFamily="ui-monospace, SFMono-Regular, Menlo, monospace"
        fontSize="13"
        fill="#BAE6FD"
        letterSpacing="0.5em"
      >
        OCTOPUS
      </text>
      <text
        x={cx}
        y={cy + 70}
        textAnchor="middle"
        fontFamily="ui-monospace, SFMono-Regular, Menlo, monospace"
        fontSize="9"
        fill="#5BA8D9"
        letterSpacing="0.3em"
      >
        REACTOR · CORE
      </text>

      {/* Sub-agent nodes — pod silhouette: outer ring + inner core */}
      {placed.map(({ agent, x, y, angle }) => {
        const a = activity[agent.name];
        const color = stateColor(a?.state ?? 'idle');
        const isRunning = a?.state === 'running';
        // Place label outside the node along the angle so it doesn't overlap
        const labelR = 22;
        const lx = x + Math.cos(angle) * labelR;
        const ly = y + Math.sin(angle) * labelR + 3;
        const anchor =
          Math.cos(angle) > 0.4 ? 'start' : Math.cos(angle) < -0.4 ? 'end' : 'middle';
        return (
          <g key={agent.name}>
            {/* Halo for running */}
            {isRunning && (
              <circle cx={x} cy={y} r={18} fill={color} fillOpacity="0.15">
                <animate
                  attributeName="r"
                  values="14;22;14"
                  dur="1.6s"
                  repeatCount="indefinite"
                />
                <animate
                  attributeName="fill-opacity"
                  values="0.05;0.25;0.05"
                  dur="1.6s"
                  repeatCount="indefinite"
                />
              </circle>
            )}
            {/* Outer ring */}
            <circle
              cx={x}
              cy={y}
              r={9}
              fill="#03060c"
              stroke={color}
              strokeWidth={1.4}
              strokeOpacity={isRunning ? 1 : 0.7}
            />
            {/* Inner core */}
            <circle cx={x} cy={y} r={3.5} fill={color} />
            {/* Tick mark indicating state direction */}
            <line
              x1={x + Math.cos(angle) * 9}
              y1={y + Math.sin(angle) * 9}
              x2={x + Math.cos(angle) * 13}
              y2={y + Math.sin(angle) * 13}
              stroke={color}
              strokeWidth={1.4}
              strokeOpacity={isRunning ? 1 : 0.5}
            />
            {/* Label outside the node */}
            <text
              x={lx}
              y={ly}
              textAnchor={anchor}
              fontFamily="ui-monospace, SFMono-Regular, Menlo, monospace"
              fontSize="9"
              fill={isRunning ? '#BAE6FD' : '#5BA8D9'}
              letterSpacing="0.1em"
            >
              {agent.name.length > 16 ? agent.name.slice(0, 15) + '…' : agent.name}
            </text>
          </g>
        );
      })}
    </svg>
  );
}

// ─── Live view ──────────────────────────────────────────────────────────

function LiveView({
  events,
  activity,
  agents,
}: {
  events: SSEEvent[];
  activity: Record<string, AgentActivity>;
  agents: AgentInfo[];
}) {
  const recent = events.slice(-50).reverse();
  const running = Object.values(activity).filter((a) => a.state === 'running');
  const totalRuns = Object.values(activity).reduce((s, a) => s + a.runs, 0);

  const panelStyle = {
    background: 'rgba(12, 16, 24, 0.7)',
    borderColor: 'rgba(125, 211, 252, 0.18)',
  } as const;
  const panelHeaderStyle = {
    borderColor: 'rgba(125, 211, 252, 0.12)',
    color: '#5BA8D9',
    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
    letterSpacing: '0.25em',
  } as const;

  return (
    <div className="grid grid-cols-1 lg:grid-cols-[1fr_340px] gap-4">
      <div className="rounded border overflow-hidden" style={panelStyle}>
        <div
          className="px-4 py-3 border-b text-[10px]"
          style={panelHeaderStyle}
        >
          EVENT STREAM · {events.length} BUFFERED · {totalRuns} RUNS
        </div>
        <div
          className="overflow-y-auto text-xs"
          style={{
            maxHeight: 'calc(100vh - 340px)',
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
          }}
        >
          {recent.length === 0 ? (
            <div className="p-8 text-center" style={{ color: '#5BA8D9', letterSpacing: '0.15em' }}>
              NO EVENTS YET — DELEGATE FROM THE CHAT.
            </div>
          ) : (
            <ul>
              {recent.map((e, i) => {
                const isError = e.type === 'error' || e.success === false;
                const isDone =
                  e.type === 'agent_end' || (e.type === 'tool_call' && e.success !== false);
                const Icon = isError ? AlertCircle : isDone ? CheckCircle2 : Activity;
                const color = isError ? '#F87171' : isDone ? '#86EFAC' : '#7DD3FC';
                const target =
                  (e.target_agent as string | undefined) ??
                  (Array.isArray(e.target_agents)
                    ? (e.target_agents as string[]).join('+')
                    : undefined);
                return (
                  <li
                    key={i}
                    className="px-4 py-2 flex items-start gap-2.5 border-b last:border-0"
                    style={{ borderColor: 'rgba(125, 211, 252, 0.05)' }}
                  >
                    <Icon className="h-3 w-3 mt-0.5 shrink-0" style={{ color }} />
                    <span style={{ color: '#5BA8D9', minWidth: '60px' }}>
                      {(e.timestamp ?? '').slice(11, 19)}
                    </span>
                    <span
                      style={{
                        color,
                        letterSpacing: '0.1em',
                        minWidth: '120px',
                        textTransform: 'uppercase',
                      }}
                    >
                      {e.type}
                    </span>
                    <span className="truncate" style={{ color: '#BAE6FD' }}>
                      {target && <span style={{ color: '#7DD3FC' }}>→ {target} </span>}
                      {e.tool ?? e.model ?? e.message ?? e.component ?? ''}
                      {typeof e.duration_ms === 'number' && (
                        <span style={{ color: '#5BA8D9' }}> · {e.duration_ms}ms</span>
                      )}
                    </span>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </div>

      <aside className="rounded border p-5 self-start space-y-5" style={panelStyle}>
        <div>
          <h2 className="text-[10px] mb-3" style={panelHeaderStyle}>
            NOW RUNNING · {running.length}
          </h2>
          {running.length === 0 ? (
            <p
              className="text-xs"
              style={{
                color: '#5BA8D9',
                fontFamily: 'ui-monospace, monospace',
                letterSpacing: '0.15em',
              }}
            >
              ALL AGENTS IDLE
            </p>
          ) : (
            <ul className="space-y-2">
              {running.map((a) => (
                <li
                  key={a.name}
                  className="flex items-center gap-2 text-xs"
                  style={{
                    color: '#BAE6FD',
                    fontFamily: 'ui-monospace, monospace',
                    letterSpacing: '0.1em',
                  }}
                >
                  <span
                    className="inline-block h-1.5 w-1.5 rounded-full"
                    style={{
                      background: '#7DD3FC',
                      boxShadow: '0 0 8px #7DD3FC',
                      animation: 'pulse 1.6s ease-in-out infinite',
                    }}
                  />
                  {a.name}
                </li>
              ))}
            </ul>
          )}
        </div>

        <div>
          <h2 className="text-[10px] mb-3" style={panelHeaderStyle}>
            BENCH LEADERBOARD
          </h2>
          <ul className="space-y-1">
            {agents
              .map((a) => activity[a.name])
              .filter((a): a is AgentActivity => Boolean(a))
              .sort((a, b) => b.runs - a.runs)
              .slice(0, 12)
              .map((a) => {
                const max =
                  Math.max(
                    1,
                    ...agents.map((x) => activity[x.name]?.runs ?? 0),
                  );
                const pct = (a.runs / max) * 100;
                return (
                  <li
                    key={a.name}
                    className="flex items-center gap-2 text-xs relative"
                    style={{
                      fontFamily: 'ui-monospace, monospace',
                      letterSpacing: '0.05em',
                    }}
                  >
                    <Bot className="h-3 w-3 shrink-0" style={{ color: '#7DD3FC' }} />
                    <span className="truncate flex-1 z-10" style={{ color: '#BAE6FD' }}>
                      {a.name}
                    </span>
                    <span style={{ color: '#5BA8D9' }}>{a.runs}</span>
                    {a.runs > 0 && (
                      <span
                        className="absolute inset-y-0 left-0 rounded"
                        style={{
                          width: `${pct}%`,
                          background:
                            'linear-gradient(90deg, rgba(125, 211, 252, 0.12), transparent)',
                          pointerEvents: 'none',
                        }}
                      />
                    )}
                  </li>
                );
              })}
          </ul>
        </div>
      </aside>
    </div>
  );
}

// ─── Timeline (Gantt-ish) ───────────────────────────────────────────────

function TimelineView({ events, agents }: { events: SSEEvent[]; agents: AgentInfo[] }) {
  // Build runs by pairing _start / _end events per agent name (best-effort).
  // Prefer target_agent (delegate dispatch) over generic agent/model fallback.
  const runs = useMemo(() => {
    const out: Array<{
      agent: string;
      start: number;
      end: number;
      type: string;
      ok: boolean;
    }> = [];
    const open = new Map<string, number>();
    for (const e of events) {
      const t = (e._rxAt as number | undefined) ?? Date.now();
      const targets =
        Array.isArray(e.target_agents) && e.target_agents.length > 0
          ? (e.target_agents as string[])
          : typeof e.target_agent === 'string'
            ? [e.target_agent]
            : [
                (e.agent as string | undefined) ??
                  (e.model as string | undefined) ??
                  ORCHESTRATOR_NAME,
              ];
      for (const key of targets) {
        if (
          e.type === 'agent_start' ||
          e.type === 'tool_call_start' ||
          e.type === 'llm_request'
        ) {
          open.set(`${key}|${e.tool ?? e.model ?? ''}`, t);
        } else if (e.type === 'agent_end' || e.type === 'tool_call') {
          const startKey = `${key}|${e.tool ?? e.model ?? ''}`;
          const start = open.get(startKey) ?? t - (e.duration_ms ?? 200);
          open.delete(startKey);
          out.push({
            agent: key,
            start,
            end: t,
            type: e.type,
            ok: e.success !== false,
          });
        }
      }
    }
    return out;
  }, [events]);

  const now = Date.now();
  const earliest = runs.length > 0 ? Math.min(...runs.map((r) => r.start)) : now - 60_000;
  const span = Math.max(now - earliest, 30_000);

  const lanes = useMemo(() => {
    const set = new Set<string>();
    set.add(ORCHESTRATOR_NAME);
    for (const a of agents) set.add(a.name);
    return Array.from(set);
  }, [agents]);

  return (
    <div
      className="rounded border overflow-hidden"
      style={{
        background: 'rgba(12, 16, 24, 0.7)',
        borderColor: 'rgba(125, 211, 252, 0.18)',
      }}
    >
      <div
        className="px-4 py-3 border-b text-[10px] flex items-center justify-between"
        style={{
          borderColor: 'rgba(125, 211, 252, 0.12)',
          color: '#5BA8D9',
          fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
          letterSpacing: '0.25em',
        }}
      >
        <span>
          TIMELINE · LAST {Math.round(span / 1000)}S · {runs.length} RUNS
        </span>
        <span>{runs.length === 0 && 'SESSION-ONLY · LOST ON RELOAD'}</span>
      </div>
      <div
        className="overflow-y-auto text-xs"
        style={{
          maxHeight: 'calc(100vh - 340px)',
          fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
        }}
      >
        <table className="w-full">
          <tbody>
            {lanes.map((lane, idx) => {
              const laneRuns = runs.filter((r) => r.agent === lane);
              const isOrch = lane === ORCHESTRATOR_NAME;
              return (
                <tr
                  key={lane}
                  className="border-b last:border-0"
                  style={{
                    borderColor: 'rgba(125, 211, 252, 0.05)',
                    background:
                      idx % 2 === 0 ? 'rgba(125, 211, 252, 0.015)' : 'transparent',
                  }}
                >
                  <td
                    className="px-4 py-2 align-middle whitespace-nowrap"
                    style={{
                      color: isOrch ? '#7DD3FC' : '#BAE6FD',
                      width: '200px',
                      letterSpacing: '0.12em',
                      fontWeight: isOrch ? 600 : 400,
                    }}
                  >
                    {isOrch && '◆ '}
                    {lane}
                  </td>
                  <td className="px-2 py-2 relative" style={{ height: '28px' }}>
                    <div
                      className="absolute inset-y-2 left-2 right-2 rounded"
                      style={{
                        background:
                          'repeating-linear-gradient(90deg, transparent 0 8px, rgba(125, 211, 252, 0.06) 8px 9px)',
                      }}
                    />
                    {laneRuns.map((r, i) => {
                      const left = ((r.start - earliest) / span) * 100;
                      const width = Math.max(((r.end - r.start) / span) * 100, 0.6);
                      return (
                        <div
                          key={i}
                          className="absolute rounded-sm"
                          title={`${r.type} · ${r.end - r.start}ms`}
                          style={{
                            left: `calc(${Math.max(left, 0)}% + 8px)`,
                            width: `${width}%`,
                            top: '6px',
                            bottom: '6px',
                            background: r.ok
                              ? 'linear-gradient(180deg, rgba(125, 211, 252, 0.7), rgba(91, 168, 217, 0.5))'
                              : 'linear-gradient(180deg, rgba(248, 113, 113, 0.7), rgba(190, 85, 85, 0.5))',
                            border: r.ok
                              ? '1px solid #7DD3FC'
                              : '1px solid #F87171',
                            boxShadow: r.ok
                              ? '0 0 8px rgba(125, 211, 252, 0.4)'
                              : '0 0 8px rgba(248, 113, 113, 0.4)',
                          }}
                        />
                      );
                    })}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
