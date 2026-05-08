// Orchestrator command center — pixel-art HUD for running an army
// of autonomous agents across multiple companies. Inspired by 4X /
// XCOM management screens: each agent is a unit, each tenant is a
// kingdom, the reactor at the centre is the throne room.

import { useEffect, useMemo, useState } from 'react';
import {
  Activity,
  AlertCircle,
  Building2,
  CheckCircle2,
  Clock,
  Network,
  Plus,
} from 'lucide-react';
import {
  createTenant,
  deleteTenant,
  getAgents,
  type AgentInfo,
  type Tenant,
  type TenantCategory,
  type TenantStage,
} from '@/lib/api';
import { SSEClient } from '@/lib/sse';
import { useTenant } from '@/contexts/TenantContext';
import { PixelSigil } from '@/components/PixelSigil';
import type { SSEEvent } from '@/types/api';

type Tab = 'mesh' | 'live' | 'timeline';

interface AgentActivity {
  name: string;
  state: 'idle' | 'running' | 'done' | 'error';
  lastEventAt: number | null;
  runs: number;
  totalDurationMs: number;
}

const ORCHESTRATOR_NAME = 'orchestrator';
const STATE_DECAY_MS = 4000;

const DEPARTMENTS: { id: string; label: string; agents: string[] }[] = [
  {
    id: 'csuite',
    label: 'C-SUITE',
    agents: ['ceo_advisor', 'cto_advisor', 'cfo_advisor'],
  },
  {
    id: 'idea',
    label: 'IDEA → BUSINESS',
    agents: [
      'idea_generator',
      'idea_validator',
      'customer_researcher',
      'competitor_analyst',
      'market_researcher',
      'red_teamer',
      'pivot_strategist',
    ],
  },
  {
    id: 'business',
    label: 'BUSINESS / GTM',
    agents: [
      'business_developer',
      'product_manager',
      'growth_hacker',
      'pricing_strategist',
      'marketing',
      'content_creator',
      'scriptwriter',
    ],
  },
  {
    id: 'revenue',
    label: 'REVENUE',
    agents: ['sdr_outbound', 'account_executive', 'customer_success'],
  },
  {
    id: 'finance',
    label: 'FINANCE & RISK',
    agents: ['finance_controller', 'risk_analyst', 'data_analyst'],
  },
  {
    id: 'compliance',
    label: 'SECURITY & LEGAL',
    agents: ['security', 'legal_compliance'],
  },
  {
    id: 'engineering',
    label: 'ENGINEERING',
    agents: [
      'planner',
      'architect',
      'server_architect',
      'db_designer',
      'adr_writer',
      'coder',
      'designer',
      'reviewer',
      'tester',
      'qa',
      'cicd',
      'devops',
      'docs',
    ],
  },
];

function eventAgentNames(e: SSEEvent): string[] {
  if (Array.isArray(e.target_agents)) {
    return e.target_agents.filter(
      (s: unknown): s is string => typeof s === 'string',
    );
  }
  if (typeof e.target_agent === 'string') return [e.target_agent];
  if (typeof e.agent === 'string') return [e.agent];
  if (typeof e.model === 'string') return [ORCHESTRATOR_NAME];
  return [];
}

const STAGE_META: Record<TenantStage, { label: string; ordinal: number }> = {
  ideation: { label: 'IDEATION', ordinal: 0 },
  validation: { label: 'VALIDATION', ordinal: 1 },
  mvp: { label: 'MVP', ordinal: 2 },
  launch: { label: 'LAUNCH', ordinal: 3 },
  growth: { label: 'GROWTH', ordinal: 4 },
  scale: { label: 'SCALE', ordinal: 5 },
};

const CATEGORY_META: Record<TenantCategory, { label: string; tint: string }> = {
  saas: { label: 'SAAS', tint: '#7DD3FC' },
  marketplace: { label: 'MARKETPLACE', tint: '#86EFAC' },
  fintech: { label: 'FINTECH', tint: '#FCD34D' },
  ecommerce: { label: 'E-COM', tint: '#F9A8D4' },
  agency: { label: 'AGENCY', tint: '#C084FC' },
  hardware: { label: 'HARDWARE', tint: '#FDBA74' },
  media: { label: 'MEDIA', tint: '#A5F3FC' },
  ai: { label: 'AI', tint: '#FDA4AF' },
  other: { label: 'OTHER', tint: '#94A3B8' },
};

export default function Orchestrator() {
  const { active: activeTenant, tenants, refresh: refreshTenants } = useTenant();

  const [tab, setTab] = useState<Tab>(() => {
    try {
      return (localStorage.getItem('octopus_orch_tab') as Tab) || 'mesh';
    } catch {
      return 'mesh';
    }
  });
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [activity, setActivity] = useState<Record<string, AgentActivity>>({});
  const [events, setEvents] = useState<SSEEvent[]>([]);
  const [connected, setConnected] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [showNewTenant, setShowNewTenant] = useState(false);

  // ─── Tab persistence ─────────────────────────────────────────────
  useEffect(() => {
    try {
      localStorage.setItem('octopus_orch_tab', tab);
    } catch {
      /* ignore */
    }
  }, [tab]);

  // ─── Agent catalog ───────────────────────────────────────────────
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

  // ─── SSE ─────────────────────────────────────────────────────────
  useEffect(() => {
    const client = new SSEClient();
    client.onConnect = () => setConnected(true);
    client.onError = () => setConnected(false);
    client.onEvent = (e) => {
      setEvents((prev) =>
        [...prev, { ...e, _rxAt: Date.now() } as SSEEvent].slice(-200),
      );

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

  // ─── Decay running → idle ────────────────────────────────────────
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

  // ─── Recommended agents per tenant category ──────────────────────
  const [recommended, setRecommended] = useState<string[]>([]);
  useEffect(() => {
    if (!activeTenant) {
      setRecommended([]);
      return;
    }
    // The /api/tenants/{id} endpoint also returns recommended_agents,
    // but we already have category locally — recompute lightly.
    // (The truth lives on the server; this is just for first paint.)
    const map: Partial<Record<TenantCategory, string[]>> = {
      saas: [
        'product_manager', 'growth_hacker', 'pricing_strategist',
        'sdr_outbound', 'account_executive', 'customer_success',
        'cto_advisor', 'data_analyst', 'finance_controller',
        'ceo_advisor', 'marketing', 'coder', 'tester',
      ],
      fintech: [
        'risk_analyst', 'legal_compliance', 'security',
        'finance_controller', 'cfo_advisor', 'data_analyst',
        'product_manager', 'account_executive', 'customer_success',
        'ceo_advisor', 'architect', 'db_designer',
      ],
      marketplace: [
        'business_developer', 'product_manager', 'growth_hacker',
        'customer_researcher', 'customer_success',
        'data_analyst', 'risk_analyst', 'legal_compliance',
        'finance_controller', 'ceo_advisor', 'marketing',
      ],
      ecommerce: [
        'marketing', 'growth_hacker', 'content_creator',
        'pricing_strategist', 'customer_success', 'data_analyst',
        'business_developer', 'finance_controller', 'ceo_advisor',
        'designer', 'scriptwriter',
      ],
      agency: [
        'business_developer', 'account_executive', 'customer_success',
        'content_creator', 'marketing', 'scriptwriter',
        'finance_controller', 'legal_compliance', 'ceo_advisor',
        'designer', 'product_manager',
      ],
      hardware: [
        'architect', 'server_architect', 'designer',
        'risk_analyst', 'legal_compliance', 'security',
        'business_developer', 'pricing_strategist',
        'finance_controller', 'cfo_advisor', 'ceo_advisor',
        'market_researcher',
      ],
      media: [
        'content_creator', 'scriptwriter', 'marketing',
        'growth_hacker', 'data_analyst', 'designer',
        'business_developer', 'ceo_advisor', 'pricing_strategist',
        'customer_researcher',
      ],
      ai: [
        'cto_advisor', 'architect', 'data_analyst',
        'product_manager', 'growth_hacker', 'pricing_strategist',
        'risk_analyst', 'legal_compliance', 'ceo_advisor',
        'coder', 'tester', 'security',
      ],
      other: [
        'idea_generator', 'idea_validator', 'customer_researcher',
        'competitor_analyst', 'red_teamer', 'pivot_strategist',
        'product_manager', 'ceo_advisor',
      ],
    };
    setRecommended(map[activeTenant.category] ?? []);
  }, [activeTenant]);

  const runningAgents = useMemo(
    () =>
      Object.values(activity)
        .filter((a) => a.state === 'running')
        .map((a) => a.name),
    [activity],
  );
  const totalRuns = useMemo(
    () => Object.values(activity).reduce((s, a) => s + a.runs, 0),
    [activity],
  );

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div
          className="h-8 w-8 border-2 rounded-full animate-spin"
          style={{
            borderColor: 'rgba(125, 211, 252, 0.2)',
            borderTopColor: '#7DD3FC',
          }}
        />
      </div>
    );
  }

  return (
    <div
      className="p-5 space-y-4 animate-fade-in min-h-screen"
      style={{ background: '#03060c' }}
    >
      {/* ─── HUD bar ─────────────────────────────────────────────── */}
      <CommandHud
        activeTenant={activeTenant}
        connected={connected}
        runningAgents={runningAgents}
        totalRuns={totalRuns}
        agentCount={agents.length}
        tab={tab}
        onTab={setTab}
        onNewTenant={() => setShowNewTenant(true)}
      />

      {!activeTenant && tenants.length === 0 && (
        <NoTenantBanner onCreate={() => setShowNewTenant(true)} />
      )}

      {error && (
        <div
          className="rounded border p-3 text-xs font-mono"
          style={{
            background: 'rgba(239, 68, 68, 0.08)',
            borderColor: 'rgba(239, 68, 68, 0.4)',
            color: '#FCA5A5',
            letterSpacing: '0.1em',
          }}
        >
          ERR · {error}
        </div>
      )}

      {tab === 'mesh' && (
        <MeshView
          agents={agents}
          activity={activity}
          recommended={recommended}
          tenant={activeTenant}
          selected={selected}
          onSelect={setSelected}
        />
      )}

      {tab === 'live' && <LiveView events={events} activity={activity} />}

      {tab === 'timeline' && <TimelineView events={events} agents={agents} />}

      {showNewTenant && (
        <TenantCreateModal
          onClose={() => setShowNewTenant(false)}
          onCreated={async () => {
            await refreshTenants();
            setShowNewTenant(false);
          }}
        />
      )}
    </div>
  );
}

// ─── HUD command bar ─────────────────────────────────────────────────

function CommandHud({
  activeTenant,
  connected,
  runningAgents,
  totalRuns,
  agentCount,
  tab,
  onTab,
  onNewTenant,
}: {
  activeTenant: Tenant | null;
  connected: boolean;
  runningAgents: string[];
  totalRuns: number;
  agentCount: number;
  tab: Tab;
  onTab: (t: Tab) => void;
  onNewTenant: () => void;
}) {
  const stage = activeTenant ? STAGE_META[activeTenant.stage] : null;
  const cat = activeTenant ? CATEGORY_META[activeTenant.category] : null;

  return (
    <div
      className="rounded border p-4"
      style={{
        background: 'rgba(12, 16, 24, 0.95)',
        borderColor: 'rgba(125, 211, 252, 0.2)',
      }}
    >
      <div className="flex items-end justify-between gap-4 flex-wrap">
        {/* ── Title block ── */}
        <div className="flex items-center gap-4">
          <div
            className="flex items-center justify-center"
            style={{
              width: 56,
              height: 56,
              background: 'rgba(3, 6, 12, 1)',
              border: '2px solid rgba(125, 211, 252, 0.4)',
            }}
          >
            <PixelSigil name={ORCHESTRATOR_NAME} size={40} state="idle" />
          </div>
          <div>
            <p
              className="text-[10px]"
              style={{
                color: '#5BA8D9',
                fontFamily: 'ui-monospace, monospace',
                letterSpacing: '0.4em',
              }}
            >
              OCTOPUS · COMMAND CENTER
            </p>
            <h1
              className="text-2xl"
              style={{
                color: '#BAE6FD',
                fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
                letterSpacing: '0.18em',
                fontWeight: 500,
              }}
            >
              REACTOR · CORE
            </h1>
          </div>
        </div>

        {/* ── Tenant card ── */}
        {activeTenant && stage && cat ? (
          <div
            className="flex items-center gap-4 px-4 py-2 rounded border"
            style={{
              background: 'rgba(3, 6, 12, 0.7)',
              borderColor: cat.tint + '66',
            }}
          >
            <Building2 className="h-5 w-5" style={{ color: cat.tint }} />
            <div>
              <p
                className="text-[9px]"
                style={{
                  color: cat.tint,
                  fontFamily: 'ui-monospace, monospace',
                  letterSpacing: '0.3em',
                }}
              >
                {cat.label}
              </p>
              <p
                className="text-sm"
                style={{
                  color: '#BAE6FD',
                  fontFamily: 'ui-monospace, monospace',
                  letterSpacing: '0.1em',
                }}
              >
                {activeTenant.name}
              </p>
            </div>
            <StageMeter stage={stage} />
          </div>
        ) : (
          <button
            onClick={onNewTenant}
            className="flex items-center gap-2 px-3 py-2 rounded border text-xs"
            style={{
              background: 'rgba(125, 211, 252, 0.08)',
              borderColor: 'rgba(125, 211, 252, 0.4)',
              color: '#7DD3FC',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.2em',
            }}
          >
            <Plus className="h-3.5 w-3.5" /> NEW COMPANY
          </button>
        )}

        {/* ── Live stats + tabs ── */}
        <div className="flex items-center gap-3 flex-wrap">
          <span
            className="inline-flex items-center gap-2 text-[10px] px-2 py-1.5 rounded border"
            style={{
              color: connected ? '#7DD3FC' : '#5BA8D9',
              borderColor: connected
                ? 'rgba(125, 211, 252, 0.4)'
                : 'rgba(91, 168, 217, 0.2)',
              background: connected ? 'rgba(125, 211, 252, 0.06)' : 'transparent',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.2em',
            }}
          >
            <span
              className="inline-block h-1.5 w-1.5 rounded-full"
              style={{
                background: connected ? '#7DD3FC' : '#5BA8D9',
                animation: connected ? 'pulse 2s infinite' : undefined,
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
                ['mesh', 'MESH', Network],
                ['live', 'LIVE', Activity],
                ['timeline', 'TIME', Clock],
              ] as const
            ).map(([id, label, Icon]) => {
              const active = tab === id;
              return (
                <button
                  key={id}
                  role="tab"
                  aria-selected={active}
                  onClick={() => onTab(id)}
                  className="px-3 py-1.5 text-[10px] inline-flex items-center gap-1.5"
                  style={{
                    background: active
                      ? 'rgba(125, 211, 252, 0.12)'
                      : 'transparent',
                    color: active ? '#BAE6FD' : '#5BA8D9',
                    border: active
                      ? '1px solid rgba(125, 211, 252, 0.4)'
                      : '1px solid transparent',
                    fontFamily: 'ui-monospace, monospace',
                    letterSpacing: '0.2em',
                  }}
                >
                  <Icon className="h-3 w-3" /> {label}
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {/* ── Stat strip ── */}
      <div
        className="mt-4 pt-3 grid grid-cols-2 md:grid-cols-4 gap-3 border-t"
        style={{ borderColor: 'rgba(125, 211, 252, 0.1)' }}
      >
        <Stat label="AGENTS" value={agentCount.toString()} />
        <Stat
          label="RUNNING"
          value={runningAgents.length.toString()}
          accent={runningAgents.length > 0}
        />
        <Stat label="TOTAL RUNS" value={totalRuns.toString()} />
        <Stat
          label="EXECUTING"
          value={
            runningAgents.length > 0 ? runningAgents.slice(0, 2).join(' + ') : '—'
          }
          accent={runningAgents.length > 0}
          mono
        />
      </div>
    </div>
  );
}

function Stat({
  label,
  value,
  accent,
  mono,
}: {
  label: string;
  value: string;
  accent?: boolean;
  mono?: boolean;
}) {
  return (
    <div>
      <p
        className="text-[9px]"
        style={{
          color: '#5BA8D9',
          fontFamily: 'ui-monospace, monospace',
          letterSpacing: '0.3em',
        }}
      >
        {label}
      </p>
      <p
        className={mono ? 'text-xs truncate' : 'text-lg'}
        style={{
          color: accent ? '#7DD3FC' : '#BAE6FD',
          fontFamily: 'ui-monospace, monospace',
          letterSpacing: mono ? '0.1em' : '0.05em',
        }}
      >
        {value}
      </p>
    </div>
  );
}

function StageMeter({ stage }: { stage: { label: string; ordinal: number } }) {
  return (
    <div className="flex flex-col">
      <p
        className="text-[9px]"
        style={{
          color: '#5BA8D9',
          fontFamily: 'ui-monospace, monospace',
          letterSpacing: '0.3em',
        }}
      >
        {stage.label}
      </p>
      <div className="flex gap-0.5 mt-1">
        {Array.from({ length: 6 }).map((_, i) => (
          <span
            key={i}
            style={{
              width: 8,
              height: 8,
              background: i <= stage.ordinal ? '#7DD3FC' : 'rgba(91, 168, 217, 0.2)',
              border: '1px solid rgba(125, 211, 252, 0.3)',
              imageRendering: 'pixelated',
            }}
          />
        ))}
      </div>
    </div>
  );
}

// ─── Mesh view (the agent barracks) ──────────────────────────────────

function MeshView({
  agents,
  activity,
  recommended,
  tenant,
  selected,
  onSelect,
}: {
  agents: AgentInfo[];
  activity: Record<string, AgentActivity>;
  recommended: string[];
  tenant: Tenant | null;
  selected: string | null;
  onSelect: (name: string | null) => void;
}) {
  const recSet = useMemo(() => new Set(recommended), [recommended]);
  const byName = useMemo(() => {
    const m: Record<string, AgentInfo> = {};
    for (const a of agents) m[a.name] = a;
    return m;
  }, [agents]);

  const selAgent = selected ? byName[selected] : null;
  const selAct = selected ? activity[selected] : null;

  return (
    <div className="grid grid-cols-1 lg:grid-cols-[1fr_320px] gap-4">
      {/* Departments */}
      <div className="space-y-4">
        {DEPARTMENTS.map((dept) => {
          const present = dept.agents.filter((n) => byName[n]);
          if (present.length === 0) return null;
          return (
            <div
              key={dept.id}
              className="rounded border p-3"
              style={{
                background: 'rgba(12, 16, 24, 0.5)',
                borderColor: 'rgba(125, 211, 252, 0.15)',
              }}
            >
              <p
                className="text-[10px] mb-3"
                style={{
                  color: '#5BA8D9',
                  fontFamily: 'ui-monospace, monospace',
                  letterSpacing: '0.3em',
                }}
              >
                {dept.label} · {present.length}
              </p>
              <div
                className="grid gap-2"
                style={{
                  gridTemplateColumns: 'repeat(auto-fill, minmax(110px, 1fr))',
                }}
              >
                {present.map((name) => {
                  const a = byName[name]!;
                  const act = activity[name];
                  const isRec = !tenant || recSet.has(name);
                  const isSelected = selected === name;
                  return (
                    <UnitCard
                      key={name}
                      agent={a}
                      activity={act}
                      recommended={isRec}
                      selected={isSelected}
                      onClick={() => onSelect(isSelected ? null : name)}
                    />
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>

      {/* Inspector */}
      <aside
        className="rounded border p-4 self-start"
        style={{
          background: 'rgba(12, 16, 24, 0.7)',
          borderColor: 'rgba(125, 211, 252, 0.2)',
        }}
      >
        <p
          className="text-[10px] mb-3"
          style={{
            color: '#5BA8D9',
            fontFamily: 'ui-monospace, monospace',
            letterSpacing: '0.3em',
          }}
        >
          UNIT INSPECTOR
        </p>
        {!selAgent ? (
          <p
            className="text-xs"
            style={{
              color: '#5BA8D9',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.1em',
            }}
          >
            ▸ SELECT A UNIT
          </p>
        ) : (
          <UnitInspector agent={selAgent} activity={selAct} />
        )}
      </aside>
    </div>
  );
}

function UnitCard({
  agent,
  activity,
  recommended,
  selected,
  onClick,
}: {
  agent: AgentInfo;
  activity: AgentActivity | undefined;
  recommended: boolean;
  selected: boolean;
  onClick: () => void;
}) {
  const state = activity?.state ?? 'idle';
  const runs = activity?.runs ?? 0;
  const dim = !recommended;
  const isRunning = state === 'running';
  const accentColor =
    state === 'running'
      ? '#7DD3FC'
      : state === 'done'
        ? '#86EFAC'
        : state === 'error'
          ? '#F87171'
          : '#5BA8D9';

  return (
    <button
      onClick={onClick}
      className="rounded text-left transition-all relative"
      style={{
        background: selected
          ? 'rgba(125, 211, 252, 0.12)'
          : 'rgba(3, 6, 12, 0.6)',
        border: selected
          ? `2px solid ${accentColor}`
          : `1px solid ${isRunning ? accentColor + '88' : 'rgba(125, 211, 252, 0.15)'}`,
        opacity: dim ? 0.45 : 1,
        padding: '10px',
        cursor: 'pointer',
        imageRendering: 'pixelated',
      }}
    >
      <div className="flex items-center gap-2 mb-2">
        <div
          style={{
            width: 32,
            height: 32,
            background: 'rgba(3, 6, 12, 0.9)',
            border: `1px solid ${accentColor}66`,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          <PixelSigil name={agent.name} size={28} state={state} />
        </div>
        <div className="flex-1 min-w-0">
          <p
            className="text-[10px] truncate"
            style={{
              color: isRunning ? '#BAE6FD' : '#94A3B8',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.05em',
            }}
            title={agent.name}
          >
            {agent.name}
          </p>
          <p
            className="text-[9px]"
            style={{
              color: accentColor,
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.2em',
            }}
          >
            {state.toUpperCase()}
          </p>
        </div>
      </div>
      <div className="flex items-center justify-between">
        <p
          className="text-[9px]"
          style={{
            color: '#64748B',
            fontFamily: 'ui-monospace, monospace',
          }}
        >
          ×{runs}
        </p>
        {agent.agentic && (
          <span
            className="text-[8px] px-1"
            style={{
              color: '#7DD3FC',
              border: '1px solid rgba(125, 211, 252, 0.3)',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.15em',
            }}
          >
            AGENTIC
          </span>
        )}
      </div>
      {isRunning && (
        <span
          className="absolute top-1 right-1 inline-block h-1.5 w-1.5 rounded-full"
          style={{
            background: accentColor,
            animation: 'pulse 1.4s infinite',
          }}
        />
      )}
    </button>
  );
}

function UnitInspector({
  agent,
  activity,
}: {
  agent: AgentInfo;
  activity: AgentActivity | null | undefined;
}) {
  const labelStyle = {
    color: '#5BA8D9',
    fontFamily: 'ui-monospace, monospace',
    letterSpacing: '0.2em',
  } as const;
  const valueStyle = {
    color: '#BAE6FD',
    fontFamily: 'ui-monospace, monospace',
    letterSpacing: '0.05em',
  } as const;
  return (
    <div className="space-y-3 text-xs">
      <div className="flex items-center gap-3">
        <div
          style={{
            width: 64,
            height: 64,
            background: 'rgba(3, 6, 12, 0.9)',
            border: '1px solid rgba(125, 211, 252, 0.3)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          <PixelSigil name={agent.name} size={56} state={activity?.state ?? 'idle'} />
        </div>
        <div>
          <p style={labelStyle}>NAME</p>
          <p style={valueStyle}>{agent.name}</p>
        </div>
      </div>

      <div>
        <p className="text-[9px]" style={labelStyle}>
          PROVIDER · MODEL
        </p>
        <p className="text-[10px]" style={valueStyle}>
          {agent.provider} · {agent.model}
        </p>
      </div>

      {agent.system_prompt_summary && (
        <div>
          <p className="text-[9px]" style={labelStyle}>
            ROLE
          </p>
          <p className="text-[11px] leading-relaxed" style={{ color: '#CBD5E1' }}>
            {agent.system_prompt_summary}
          </p>
        </div>
      )}

      <div className="grid grid-cols-2 gap-2">
        <div>
          <p className="text-[9px]" style={labelStyle}>
            DEPTH
          </p>
          <p style={valueStyle}>{agent.max_depth}</p>
        </div>
        <div>
          <p className="text-[9px]" style={labelStyle}>
            ITER
          </p>
          <p style={valueStyle}>{agent.max_iterations}</p>
        </div>
        <div>
          <p className="text-[9px]" style={labelStyle}>
            TOOLS
          </p>
          <p style={valueStyle}>{agent.allowed_tools.length}</p>
        </div>
        <div>
          <p className="text-[9px]" style={labelStyle}>
            RUNS
          </p>
          <p style={valueStyle}>{activity?.runs ?? 0}</p>
        </div>
      </div>

      {agent.memory_namespace && (
        <div>
          <p className="text-[9px]" style={labelStyle}>
            MEMORY · NS
          </p>
          <p className="text-[10px]" style={valueStyle}>
            {agent.memory_namespace}
          </p>
        </div>
      )}
    </div>
  );
}

// ─── Live + Timeline (carry over from prior version, palette-matched)

function LiveView({
  events,
  activity,
}: {
  events: SSEEvent[];
  activity: Record<string, AgentActivity>;
}) {
  const recent = events.slice(-50).reverse();
  const running = Object.values(activity).filter((a) => a.state === 'running');
  const totalRuns = Object.values(activity).reduce((s, a) => s + a.runs, 0);

  return (
    <div className="grid grid-cols-1 lg:grid-cols-[1fr_320px] gap-4">
      <div
        className="rounded border overflow-hidden"
        style={{
          background: 'rgba(12, 16, 24, 0.7)',
          borderColor: 'rgba(125, 211, 252, 0.2)',
        }}
      >
        <div
          className="px-4 py-3 border-b text-[10px]"
          style={{
            borderColor: 'rgba(125, 211, 252, 0.12)',
            color: '#5BA8D9',
            fontFamily: 'ui-monospace, monospace',
            letterSpacing: '0.25em',
          }}
        >
          EVENT STREAM · {events.length} BUFFERED · {totalRuns} RUNS
        </div>
        <div
          className="overflow-y-auto text-xs"
          style={{
            maxHeight: 'calc(100vh - 360px)',
            fontFamily: 'ui-monospace, monospace',
          }}
        >
          {recent.length === 0 ? (
            <div
              className="p-8 text-center"
              style={{ color: '#5BA8D9', letterSpacing: '0.15em' }}
            >
              NO EVENTS YET — DELEGATE FROM /AGENT
            </div>
          ) : (
            <ul>
              {recent.map((e, i) => {
                const isError = e.type === 'error' || e.success === false;
                const isDone =
                  e.type === 'agent_end' ||
                  (e.type === 'tool_call' && e.success !== false);
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

      <aside
        className="rounded border p-4 self-start space-y-4"
        style={{
          background: 'rgba(12, 16, 24, 0.7)',
          borderColor: 'rgba(125, 211, 252, 0.2)',
        }}
      >
        <div>
          <p
            className="text-[10px] mb-3"
            style={{
              color: '#5BA8D9',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.25em',
            }}
          >
            NOW RUNNING · {running.length}
          </p>
          {running.length === 0 ? (
            <p
              className="text-xs"
              style={{
                color: '#5BA8D9',
                fontFamily: 'ui-monospace, monospace',
                letterSpacing: '0.15em',
              }}
            >
              ALL UNITS IDLE
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
                  <PixelSigil name={a.name} size={16} state="running" />
                  {a.name}
                </li>
              ))}
            </ul>
          )}
        </div>
      </aside>
    </div>
  );
}

function TimelineView({
  events,
  agents,
}: {
  events: SSEEvent[];
  agents: AgentInfo[];
}) {
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
        borderColor: 'rgba(125, 211, 252, 0.2)',
      }}
    >
      <div
        className="px-4 py-3 border-b text-[10px]"
        style={{
          borderColor: 'rgba(125, 211, 252, 0.12)',
          color: '#5BA8D9',
          fontFamily: 'ui-monospace, monospace',
          letterSpacing: '0.25em',
        }}
      >
        TIMELINE · LAST {Math.round(span / 1000)}S · {runs.length} RUNS
      </div>
      <div
        className="overflow-y-auto text-xs"
        style={{
          maxHeight: 'calc(100vh - 360px)',
          fontFamily: 'ui-monospace, monospace',
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
                    className="px-3 py-2 align-middle whitespace-nowrap"
                    style={{ width: '210px' }}
                  >
                    <div className="flex items-center gap-2">
                      <PixelSigil name={lane} size={14} state="idle" />
                      <span
                        style={{
                          color: isOrch ? '#7DD3FC' : '#BAE6FD',
                          letterSpacing: '0.1em',
                          fontWeight: isOrch ? 600 : 400,
                        }}
                      >
                        {lane}
                      </span>
                    </div>
                  </td>
                  <td className="px-2 py-2 relative" style={{ height: '28px' }}>
                    <div
                      className="absolute inset-y-2 left-2 right-2 rounded-sm"
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
                          className="absolute"
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
                            imageRendering: 'pixelated',
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

// ─── Empty state + create modal ──────────────────────────────────────

function NoTenantBanner({ onCreate }: { onCreate: () => void }) {
  return (
    <div
      className="rounded border p-6 text-center"
      style={{
        background:
          'radial-gradient(ellipse at center, rgba(125, 211, 252, 0.05) 0%, rgba(3, 6, 12, 0.9) 70%)',
        borderColor: 'rgba(125, 211, 252, 0.2)',
      }}
    >
      <p
        className="text-[10px] mb-2"
        style={{
          color: '#5BA8D9',
          fontFamily: 'ui-monospace, monospace',
          letterSpacing: '0.4em',
        }}
      >
        NO COMPANY SELECTED
      </p>
      <p
        className="text-sm mb-4"
        style={{
          color: '#BAE6FD',
          fontFamily: 'ui-monospace, monospace',
          letterSpacing: '0.1em',
        }}
      >
        Create your first company to deploy the bench against a real venture.
      </p>
      <button
        onClick={onCreate}
        className="inline-flex items-center gap-2 px-4 py-2 rounded border text-xs"
        style={{
          background: 'rgba(125, 211, 252, 0.1)',
          borderColor: 'rgba(125, 211, 252, 0.5)',
          color: '#7DD3FC',
          fontFamily: 'ui-monospace, monospace',
          letterSpacing: '0.2em',
        }}
      >
        <Plus className="h-3.5 w-3.5" /> NEW COMPANY
      </button>
    </div>
  );
}

const CATEGORY_DESCRIPTIONS: Record<TenantCategory, string> = {
  saas: 'Software-as-a-Service · seats / requests / workflows',
  marketplace: 'Two-sided marketplace · GMV, take rate',
  fintech: 'Financial services · regulated, risk-heavy',
  ecommerce: 'Direct-to-consumer / retail',
  agency: 'Services / consulting · time-as-product',
  hardware: 'Physical product · supply chain, certification',
  media: 'Content / publishing / community',
  ai: 'AI-native product · models, GPU costs',
  other: 'Custom / undecided',
};

function TenantCreateModal({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: () => void;
}) {
  const [name, setName] = useState('');
  const [category, setCategory] = useState<TenantCategory>('saas');
  const [mission, setMission] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      setError('Name is required');
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await createTenant({ name: name.trim(), category, mission: mission.trim() });
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      style={{ background: 'rgba(3, 6, 12, 0.92)' }}
      onClick={onClose}
    >
      <form
        onSubmit={submit}
        onClick={(e) => e.stopPropagation()}
        className="rounded border p-6 w-full max-w-md space-y-4"
        style={{
          background: 'rgba(12, 16, 24, 0.95)',
          borderColor: 'rgba(125, 211, 252, 0.4)',
        }}
      >
        <p
          className="text-[10px]"
          style={{
            color: '#5BA8D9',
            fontFamily: 'ui-monospace, monospace',
            letterSpacing: '0.4em',
          }}
        >
          NEW COMPANY · DEPLOY
        </p>
        <h2
          className="text-xl"
          style={{
            color: '#BAE6FD',
            fontFamily: 'ui-monospace, monospace',
            letterSpacing: '0.18em',
          }}
        >
          INITIATE TENANT
        </h2>

        <div>
          <label
            className="text-[10px] block mb-1"
            style={{
              color: '#5BA8D9',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.25em',
            }}
          >
            NAME
          </label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Acme SaaS Co"
            autoFocus
            className="w-full px-3 py-2 rounded text-sm"
            style={{
              background: 'rgba(3, 6, 12, 0.7)',
              border: '1px solid rgba(125, 211, 252, 0.3)',
              color: '#BAE6FD',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.05em',
            }}
          />
        </div>

        <div>
          <label
            className="text-[10px] block mb-1"
            style={{
              color: '#5BA8D9',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.25em',
            }}
          >
            CATEGORY
          </label>
          <div className="grid grid-cols-3 gap-2">
            {(Object.keys(CATEGORY_META) as TenantCategory[]).map((c) => {
              const meta = CATEGORY_META[c];
              const active = category === c;
              return (
                <button
                  type="button"
                  key={c}
                  onClick={() => setCategory(c)}
                  className="px-2 py-2 rounded text-[10px]"
                  style={{
                    background: active ? meta.tint + '22' : 'rgba(3, 6, 12, 0.5)',
                    border: active
                      ? `1px solid ${meta.tint}`
                      : '1px solid rgba(125, 211, 252, 0.15)',
                    color: active ? meta.tint : '#5BA8D9',
                    fontFamily: 'ui-monospace, monospace',
                    letterSpacing: '0.2em',
                  }}
                >
                  {meta.label}
                </button>
              );
            })}
          </div>
          <p
            className="text-[10px] mt-1.5"
            style={{
              color: '#94A3B8',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.05em',
            }}
          >
            {CATEGORY_DESCRIPTIONS[category]}
          </p>
        </div>

        <div>
          <label
            className="text-[10px] block mb-1"
            style={{
              color: '#5BA8D9',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.25em',
            }}
          >
            MISSION (optional)
          </label>
          <input
            type="text"
            value={mission}
            onChange={(e) => setMission(e.target.value)}
            placeholder="One-line positioning..."
            className="w-full px-3 py-2 rounded text-sm"
            style={{
              background: 'rgba(3, 6, 12, 0.7)',
              border: '1px solid rgba(125, 211, 252, 0.3)',
              color: '#BAE6FD',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.05em',
            }}
          />
        </div>

        {error && (
          <p
            className="text-[11px]"
            style={{
              color: '#F87171',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.05em',
            }}
          >
            ERR · {error}
          </p>
        )}

        <div className="flex gap-2 justify-end">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2 rounded text-[10px]"
            style={{
              background: 'transparent',
              border: '1px solid rgba(125, 211, 252, 0.2)',
              color: '#5BA8D9',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.25em',
            }}
          >
            CANCEL
          </button>
          <button
            type="submit"
            disabled={submitting || !name.trim()}
            className="px-4 py-2 rounded text-[10px]"
            style={{
              background: 'rgba(125, 211, 252, 0.15)',
              border: '1px solid #7DD3FC',
              color: '#BAE6FD',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.25em',
              opacity: submitting || !name.trim() ? 0.5 : 1,
            }}
          >
            {submitting ? 'DEPLOYING...' : 'DEPLOY'}
          </button>
        </div>
      </form>
    </div>
  );
}

// Re-export delete helper for the future tenant management UI.
export { deleteTenant };
