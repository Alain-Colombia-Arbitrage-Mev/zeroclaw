// Orchestrator command center — mission-control HUD for running a
// fleet of autonomous agents across multiple companies. Central
// constellation graph (orchestrator hub + departmental orbits +
// agent satellites) with live telemetry strip on the side.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Activity,
  AlertCircle,
  Building2,
  CheckCircle2,
  Clock,
  FileText,
  FolderOpen,
  Image,
  Network,
  Pencil,
  Plus,
  Radio,
  RefreshCw,
  Trash2,
  Zap,
} from 'lucide-react';
import {
  createTenant,
  deleteTenant,
  getAgents,
  getDeliverablesTree,
  readDeliverable,
  updateTenant,
  wipeDeliverables,
  type DeliverableReadResponse,
  type DeliverableTreeResponse,
  type AgentInfo,
  type Tenant,
  type TenantActivity,
  type TenantCategory,
  type TenantStage,
} from '@/lib/api';
import { SSEClient } from '@/lib/sse';
import { useTenant } from '@/contexts/TenantContext';
import { PixelSigil } from '@/components/PixelSigil';
import type { SSEEvent } from '@/types/api';

type Tab = 'mesh' | 'live' | 'timeline' | 'workbench';

interface AgentActivity {
  name: string;
  state: 'idle' | 'running' | 'done' | 'error';
  lastEventAt: number | null;
  runs: number;
  totalDurationMs: number;
}

const ORCHESTRATOR_NAME = 'orchestrator';
const STATE_DECAY_MS = 4000;

// Per-department accent driving the section header + left stripe in
// the mesh. Mirrors the PixelSigil palette family the unit is
// rendered with so a department reads as a single colour band.
// `callsign` is the 3-char code used in the constellation hub —
// follows mission-control habit of console abbreviations (FLIGHT,
// EECOM, GUIDO…).
const DEPARTMENTS: {
  id: string;
  label: string;
  callsign: string;
  accent: string;
  agents: string[];
}[] = [
  {
    id: 'csuite',
    label: 'C-SUITE',
    callsign: 'CSU',
    accent: '#F9A8D4',
    agents: ['ceo_advisor', 'cto_advisor', 'cfo_advisor'],
  },
  {
    id: 'idea',
    label: 'IDEA → RESEARCH',
    callsign: 'IDA',
    accent: '#FDBA74',
    agents: [
      'idea_generator',
      'idea_validator',
      'customer_researcher',
      'competitor_analyst',
      'market_researcher',
      'market_sentiment_analyst',
      'red_teamer',
      'pivot_strategist',
      'phd_business',
      'forensic_auditor',
      'geospatial_analyst',
    ],
  },
  {
    id: 'business',
    label: 'BUSINESS · GTM',
    callsign: 'GTM',
    accent: '#C084FC',
    agents: [
      'business_developer',
      'product_manager',
      'growth_hacker',
      'pricing_strategist',
      'marketing',
      'content_creator',
      'copywriter',
      'scriptwriter',
    ],
  },
  {
    id: 'revenue',
    label: 'REVENUE',
    callsign: 'REV',
    accent: '#86EFAC',
    agents: [
      'sdr_outbound',
      'account_executive',
      'customer_success',
      'negotiator',
    ],
  },
  {
    id: 'finance',
    label: 'FINANCE · RISK',
    callsign: 'FIN',
    accent: '#FCD34D',
    agents: [
      'finance_controller',
      'risk_analyst',
      'data_analyst',
      'deeptech_financier',
    ],
  },
  {
    id: 'compliance',
    label: 'REGULATORY · COUNSEL',
    callsign: 'REG',
    accent: '#C4B5FD',
    agents: [
      'security',
      'legal_compliance',
      'fintech_counsel',
      'esg_energy_counsel',
      'latam_solar_ngo_counsel',
    ],
  },
  {
    id: 'impact',
    label: 'IMPACT · SOVEREIGN',
    callsign: 'IMP',
    accent: '#A7F3D0',
    agents: [
      'ngo_architect',
      'sovereign_advisor',
      'energy_grid_strategist',
    ],
  },
  {
    id: 'engineering',
    label: 'ENGINEERING',
    callsign: 'ENG',
    accent: '#7DD3FC',
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
  // Sage gold for energy — matches the IMPACT department accent
  // and aligns with energy_grid_strategist + esg_energy_counsel
  // sigil families.
  energy: { label: 'ENERGY', tint: '#A7F3D0' },
  // Cool cyan for nonprofit / NGO — sits with ngo_architect,
  // latam_solar_ngo_counsel, sovereign_advisor visually.
  nonprofit: { label: 'NONPROFIT', tint: '#67E8F9' },
  other: { label: 'OTHER', tint: '#94A3B8' },
};

export default function Orchestrator() {
  const { active: activeTenant, tenants, refresh: refreshTenants, setActive } = useTenant();

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
  const [editingTenant, setEditingTenant] = useState<Tenant | null>(null);
  const [deleteCandidate, setDeleteCandidate] = useState<Tenant | null>(null);

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
      energy: [
        'energy_grid_strategist', 'esg_energy_counsel',
        'geospatial_analyst', 'deeptech_financier',
        'risk_analyst', 'legal_compliance',
        'finance_controller', 'cfo_advisor', 'ceo_advisor',
        'business_developer', 'sovereign_advisor',
        'market_researcher', 'data_analyst', 'forensic_auditor',
      ],
      nonprofit: [
        'ngo_architect', 'latam_solar_ngo_counsel',
        'esg_energy_counsel', 'sovereign_advisor',
        'legal_compliance', 'finance_controller',
        'ceo_advisor', 'marketing', 'content_creator',
        'data_analyst', 'geospatial_analyst',
        'energy_grid_strategist',
      ],
      other: [
        'idea_generator', 'idea_validator', 'customer_researcher',
        'competitor_analyst', 'red_teamer', 'pivot_strategist',
        'product_manager', 'ceo_advisor',
      ],
    };
    // Merge activity bench on top of category bench, dedup-preserving
    // category order. Mirrors backend `Tenant::recommended_bench()`.
    const seen = new Set<string>();
    const merged: string[] = [];
    for (const a of map[activeTenant.category] ?? []) {
      if (!seen.has(a)) {
        seen.add(a);
        merged.push(a);
      }
    }
    for (const act of activeTenant.activities ?? []) {
      for (const a of ACTIVITIES_META[act].bench) {
        if (!seen.has(a)) {
          seen.add(a);
          merged.push(a);
        }
      }
    }
    setRecommended(merged);
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
  const errorCount = useMemo(
    () => events.filter((e) => e.type === 'error' || e.success === false).length,
    [events],
  );
  // Tick every second so the header `SIG` light + last-event-age
  // readouts decay smoothly even when no new events arrive.
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, []);
  const lastEventAge = useMemo<number | null>(() => {
    if (events.length === 0) return null;
    const last = events[events.length - 1];
    const t = (last as { _rxAt?: number })._rxAt;
    if (typeof t !== 'number') return null;
    return Math.max(0, (Date.now() - t) / 1000);
  }, [events]);

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
        tenants={tenants}
        onSwitchTenant={setActive}
        onEditTenant={(t) => setEditingTenant(t)}
        onDeleteTenant={(t) => setDeleteCandidate(t)}
        connected={connected}
        runningAgents={runningAgents}
        totalRuns={totalRuns}
        agentCount={agents.length}
        tab={tab}
        onTab={setTab}
        onNewTenant={() => setShowNewTenant(true)}
        errorCount={errorCount}
        lastEventAge={lastEventAge}
      />

      <CautionWarningStrip
        connected={connected}
        errorCount={errorCount}
        lastEventAge={lastEventAge}
        runningCount={runningAgents.length}
      />

      {tenants.length > 0 && (
        <MissionBar
          tenants={tenants}
          activeId={activeTenant?.id ?? null}
          onSwitchTenant={setActive}
          onEditTenant={(t) => setEditingTenant(t)}
          onDeleteTenant={(t) => setDeleteCandidate(t)}
          onNewTenant={() => setShowNewTenant(true)}
        />
      )}

      {/* Orchestration controls — always visible. Renders an empty
          state when no tenant is active so the operator sees the
          path to fix it. */}
      <QuickActionsPanel
        tenant={activeTenant}
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
        <ConstellationView
          agents={agents}
          activity={activity}
          recommended={recommended}
          tenant={activeTenant}
          selected={selected}
          onSelect={setSelected}
          events={events}
          connected={connected}
        />
      )}

      {tab === 'live' && <LiveView events={events} activity={activity} />}

      {tab === 'timeline' && <TimelineView events={events} agents={agents} />}

      {tab === 'workbench' && <WorkbenchView events={events} />}

      {showNewTenant && (
        <TenantCreateModal
          onClose={() => setShowNewTenant(false)}
          onCreated={async () => {
            await refreshTenants();
            setShowNewTenant(false);
          }}
        />
      )}

      {editingTenant && (
        <TenantCreateModal
          existing={editingTenant}
          onClose={() => setEditingTenant(null)}
          onCreated={async () => {
            await refreshTenants();
            setEditingTenant(null);
          }}
        />
      )}

      {deleteCandidate && (
        <TenantDeleteConfirm
          tenant={deleteCandidate}
          isActive={activeTenant?.id === deleteCandidate.id}
          onCancel={() => setDeleteCandidate(null)}
          onConfirm={async () => {
            const id = deleteCandidate.id;
            // Optimistic UX: clear active first if we're deleting it,
            // so the dashboard doesn't briefly try to fetch a dead id.
            if (activeTenant?.id === id) setActive(null);
            try {
              await deleteTenant(id);
            } finally {
              await refreshTenants();
              setDeleteCandidate(null);
            }
          }}
        />
      )}

      <ConsoleFooter
        tenant={activeTenant}
        connected={connected}
        runningCount={runningAgents.length}
        agentCount={agents.length}
      />
    </div>
  );
}

// ─── Console footer bar ─────────────────────────────────────────────
//
// Persistent strip with the operator-console channels: who's flying,
// what model is on the loop, current procedure, and the console ID.
// Reads as the "bottom rail" of a real flight-control room.

function ConsoleFooter({
  tenant,
  connected,
  runningCount,
  agentCount,
}: {
  tenant: Tenant | null;
  connected: boolean;
  runningCount: number;
  agentCount: number;
}) {
  const cells: Array<{ label: string; value: string; color?: string }> = [
    { label: 'FLIGHT', value: 'OPERATOR' },
    {
      label: 'MISSION',
      value: tenant ? tenant.name.toUpperCase() : '— NONE —',
      color: tenant ? CATEGORY_META[tenant.category].tint : undefined,
    },
    {
      label: 'PROC',
      value:
        runningCount > 0
          ? `${runningCount}/${agentCount} ACTIVE`
          : 'STANDBY',
      color: runningCount > 0 ? '#7DD3FC' : undefined,
    },
    {
      label: 'LINK',
      value: connected ? 'AOS · S-BAND' : 'LOS · ABORT',
      color: connected ? '#86EFAC' : '#F87171',
    },
    { label: 'CONSOLE', value: 'ORCH-1' },
  ];
  return (
    <div
      className="rounded border overflow-hidden flex items-stretch text-[10px]"
      style={{
        background: 'rgba(3, 6, 12, 0.95)',
        borderColor: 'rgba(125, 211, 252, 0.18)',
        fontFamily: 'ui-monospace, monospace',
      }}
    >
      {cells.map((c, i) => (
        <div
          key={c.label}
          className={`px-3 py-1.5 flex items-center gap-2 ${i < cells.length - 1 ? 'border-r' : ''} flex-1 min-w-[140px]`}
          style={{ borderColor: 'rgba(125, 211, 252, 0.1)' }}
        >
          <span style={{ color: '#5BA8D9', letterSpacing: '0.3em', minWidth: 56 }}>
            {c.label}
          </span>
          <span
            className="truncate"
            style={{
              color: c.color ?? '#BAE6FD',
              letterSpacing: '0.12em',
            }}
          >
            {c.value}
          </span>
        </div>
      ))}
    </div>
  );
}

// ─── Mission control HUD ─────────────────────────────────────────────
//
// Header conventions follow real flight-controller console habits:
// MET counts up from arrival (mission elapsed), GMT is the wall clock,
// FLT designator is deterministic from arrival date so the reader can
// reference today's session in chat / logs without leaking secrets.

function formatMET(seconds: number): string {
  const s = Math.floor(seconds);
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = s % 60;
  const pad = (n: number) => n.toString().padStart(2, '0');
  return d > 0
    ? `${d}d ${pad(h)}:${pad(m)}:${pad(ss)}`
    : `${pad(h)}:${pad(m)}:${pad(ss)}`;
}

function formatGMT(d: Date): string {
  const pad = (n: number) => n.toString().padStart(2, '0');
  return `${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())} Z`;
}

/// Day-of-year (1-366) — useful for FLT-XXX designator like Apollo era.
function dayOfYear(d: Date): number {
  const start = Date.UTC(d.getUTCFullYear(), 0, 0);
  const diff = d.getTime() - start;
  return Math.floor(diff / 86_400_000);
}

/// Mission designator: ORCH-<YY>.<DOY> — stable per UTC day,
/// resets at 00:00 GMT. Operators can quote this in chat / logs.
function missionDesignator(d: Date): string {
  const yy = d.getUTCFullYear() % 100;
  return `ORCH-${yy.toString().padStart(2, '0')}.${dayOfYear(d).toString().padStart(3, '0')}`;
}

function CommandHud({
  activeTenant,
  tenants,
  onSwitchTenant,
  onEditTenant,
  onDeleteTenant,
  connected,
  runningAgents,
  totalRuns,
  agentCount,
  tab,
  onTab,
  onNewTenant,
  errorCount,
  lastEventAge,
}: {
  activeTenant: Tenant | null;
  tenants: Tenant[];
  onSwitchTenant: (id: string | null) => void;
  onEditTenant: (t: Tenant) => void;
  onDeleteTenant: (t: Tenant) => void;
  connected: boolean;
  runningAgents: string[];
  totalRuns: number;
  agentCount: number;
  tab: Tab;
  onTab: (t: Tab) => void;
  onNewTenant: () => void;
  errorCount: number;
  lastEventAge: number | null;
}) {
  const [tenantMenuOpen, setTenantMenuOpen] = useState(false);
  const stage = activeTenant ? STAGE_META[activeTenant.stage] : null;
  const cat = activeTenant ? CATEGORY_META[activeTenant.category] : null;

  // Mission elapsed time (MET) — counts up from page mount.
  const startRef = useRef<number>(Date.now());
  const [now, setNow] = useState(() => new Date());
  const [met, setMet] = useState(0);
  useEffect(() => {
    const id = setInterval(() => {
      const d = new Date();
      setNow(d);
      setMet((d.getTime() - startRef.current) / 1000);
    }, 500);
    return () => clearInterval(id);
  }, []);

  // Three-light status cluster (TLM/CMD/SIG) instead of one badge —
  // mirrors the GO/NOGO callsign panels of real flight-control rooms.
  // TLM: SSE downlink alive. CMD: tenant loaded + agents available.
  // SIG: signal recency — green if event in last 30s, amber 30-120s,
  // muted past that, red if SSE dropped.
  const tlmGo = connected;
  const cmdGo = activeTenant !== null && agentCount > 0;
  const sigState: 'go' | 'amber' | 'red' | 'idle' = !connected
    ? 'red'
    : lastEventAge === null
      ? 'idle'
      : lastEventAge < 30
        ? 'go'
        : lastEventAge < 120
          ? 'amber'
          : 'idle';
  const sigColor =
    sigState === 'go'
      ? '#86EFAC'
      : sigState === 'amber'
        ? '#FCD34D'
        : sigState === 'red'
          ? '#F87171'
          : '#5BA8D9';

  // Aggregate mission status — used for the title-bar accent.
  const masterStatus =
    !connected
      ? { label: 'ABORT', color: '#F87171' }
      : errorCount > 0
        ? { label: 'CAUTION', color: '#FCD34D' }
        : !activeTenant
          ? { label: 'HOLD', color: '#FCD34D' }
          : { label: 'GO', color: '#86EFAC' };

  const designator = missionDesignator(now);
  const dayNumber = Math.floor(met / 86400) + 1;

  return (
    <div
      className="rounded border overflow-hidden"
      style={{
        background:
          'linear-gradient(180deg, rgba(12, 16, 24, 0.98), rgba(8, 11, 18, 0.95))',
        borderColor: masterStatus.color + '33',
        boxShadow: `inset 0 1px 0 ${masterStatus.color}22`,
      }}
    >
      {/* ── Top strip: designator · clocks · status cluster ───── */}
      <div
        className="flex items-center justify-between gap-4 px-4 py-2 border-b text-[10px] flex-wrap"
        style={{
          borderColor: 'rgba(125, 211, 252, 0.08)',
          background: 'rgba(3, 6, 12, 0.7)',
          fontFamily: 'ui-monospace, monospace',
        }}
      >
        {/* Identity block */}
        <div className="flex items-center gap-3">
          <Radio
            className="h-3.5 w-3.5"
            style={{
              color: masterStatus.color,
              animation:
                masterStatus.label === 'GO' ? 'pulse 2s infinite' : undefined,
            }}
          />
          <span style={{ color: '#5BA8D9', letterSpacing: '0.35em' }}>
            OCTOPUS · MCC
          </span>
          <span style={{ color: 'rgba(125, 211, 252, 0.25)' }}>│</span>
          <span style={{ color: '#94A3B8', letterSpacing: '0.2em' }}>
            {designator}
          </span>
          <span style={{ color: 'rgba(125, 211, 252, 0.25)' }}>│</span>
          <span style={{ color: '#94A3B8', letterSpacing: '0.2em' }}>
            DAY {dayNumber.toString().padStart(2, '0')}
          </span>
        </div>

        {/* Clocks + status cluster */}
        <div className="flex items-center gap-4 flex-wrap">
          {/* GMT clock */}
          <ClockReadout label="GMT" value={formatGMT(now)} />
          {/* MET clock */}
          <ClockReadout label="MET" value={`T+${formatMET(met)}`} accent />
          {/* Status light cluster */}
          <div
            className="inline-flex items-stretch rounded-sm border overflow-hidden"
            style={{ borderColor: 'rgba(125, 211, 252, 0.25)' }}
          >
            <StatusLight label="TLM" go={tlmGo} />
            <StatusLight label="CMD" go={cmdGo} />
            <StatusLight label="SIG" goColor={sigColor} go={sigState !== 'red' && sigState !== 'idle'} />
          </div>
          {/* Master status */}
          <div
            className="px-2 py-0.5 inline-flex items-center gap-1.5 rounded-sm"
            style={{
              background: masterStatus.color + '15',
              border: `1px solid ${masterStatus.color}66`,
            }}
          >
            <span
              className="inline-block h-1.5 w-1.5 rounded-full"
              style={{
                background: masterStatus.color,
                animation:
                  masterStatus.label === 'GO' || masterStatus.label === 'CAUTION'
                    ? 'pulse 1.6s infinite'
                    : undefined,
              }}
            />
            <span
              style={{
                color: masterStatus.color,
                letterSpacing: '0.3em',
              }}
            >
              {masterStatus.label}
            </span>
          </div>
        </div>
      </div>

      {/* ── Main bar: tenant · stage · tabs ─────────────────────── */}
      <div className="flex items-center justify-between gap-4 flex-wrap px-4 py-3">
        {/* Mission card — clickable when there are multiple tenants */}
        {activeTenant && stage && cat ? (
          <div className="flex items-center gap-3 relative">
            <button
              type="button"
              onClick={() => tenants.length > 1 && setTenantMenuOpen((v) => !v)}
              className="flex items-center gap-3 rounded-sm transition-colors px-1 py-0.5"
              style={{
                cursor: tenants.length > 1 ? 'pointer' : 'default',
                background: tenantMenuOpen ? 'rgba(125, 211, 252, 0.06)' : 'transparent',
              }}
              title={tenants.length > 1 ? 'Switch active tenant' : undefined}
            >
              <div
                className="flex items-center justify-center rounded-sm"
                style={{
                  width: 44,
                  height: 44,
                  background: 'rgba(3, 6, 12, 0.95)',
                  border: `2px solid ${cat.tint}66`,
                }}
              >
                <Building2 className="h-5 w-5" style={{ color: cat.tint }} />
              </div>
              <div className="text-left">
                <div className="flex items-center gap-2">
                  <span
                    className="text-[9px]"
                    style={{
                      color: cat.tint,
                      fontFamily: 'ui-monospace, monospace',
                      letterSpacing: '0.3em',
                    }}
                  >
                    {cat.label}
                  </span>
                  <span
                    className="text-[9px]"
                    style={{
                      color: '#5BA8D9',
                      fontFamily: 'ui-monospace, monospace',
                      letterSpacing: '0.25em',
                    }}
                  >
                    · MISSION
                    {tenants.length > 1 && (
                      <span style={{ color: '#7DD3FC' }}> · {tenants.length} ⌄</span>
                    )}
                  </span>
                </div>
                <p
                  className="text-base"
                  style={{
                    color: '#E0F2FE',
                    fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
                    letterSpacing: '0.12em',
                    fontWeight: 500,
                    lineHeight: 1.1,
                  }}
                >
                  {activeTenant.name.toUpperCase()}
                </p>
              </div>
            </button>
            <div className="ml-2">
              <StageMeter stage={stage} />
            </div>

            {/* Tenant switcher dropdown */}
            {tenantMenuOpen && tenants.length > 1 && (
              <div
                className="absolute top-full left-0 mt-1 rounded border z-50 min-w-[280px]"
                style={{
                  background: 'rgba(8, 11, 18, 0.98)',
                  borderColor: 'rgba(125, 211, 252, 0.4)',
                  boxShadow: '0 8px 24px rgba(0, 0, 0, 0.6)',
                }}
              >
                <div
                  className="px-3 py-2 text-[9px] border-b"
                  style={{
                    color: '#5BA8D9',
                    borderColor: 'rgba(125, 211, 252, 0.12)',
                    fontFamily: 'ui-monospace, monospace',
                    letterSpacing: '0.3em',
                  }}
                >
                  SWITCH MISSION · {tenants.length} TOTAL
                </div>
                <div className="max-h-80 overflow-y-auto">
                  {tenants.map((t) => {
                    const tCat = CATEGORY_META[t.category];
                    const tStage = STAGE_META[t.stage];
                    const isActive = t.id === activeTenant.id;
                    return (
                      <div
                        key={t.id}
                        className="w-full flex items-stretch transition-colors"
                        style={{
                          background: isActive ? tCat.tint + '14' : 'transparent',
                          borderLeft: `3px solid ${isActive ? tCat.tint : 'transparent'}`,
                          fontFamily: 'ui-monospace, monospace',
                        }}
                      >
                        <button
                          type="button"
                          onClick={() => {
                            onSwitchTenant(t.id);
                            setTenantMenuOpen(false);
                          }}
                          className="flex-1 text-left px-3 py-2 flex items-center gap-3 min-w-0"
                          title={isActive ? 'Already active' : 'Switch to this mission'}
                        >
                          <Building2
                            className="h-4 w-4 shrink-0"
                            style={{ color: tCat.tint }}
                          />
                          <div className="flex-1 min-w-0">
                            <div
                              className="text-[12px] truncate"
                              style={{
                                color: isActive ? '#E0F2FE' : '#BAE6FD',
                                letterSpacing: '0.08em',
                                fontWeight: isActive ? 600 : 400,
                              }}
                            >
                              {t.name}
                            </div>
                            <div className="flex items-center gap-2 text-[9px] mt-0.5">
                              <span
                                style={{
                                  color: tCat.tint,
                                  letterSpacing: '0.25em',
                                }}
                              >
                                {tCat.label}
                              </span>
                              <span style={{ color: 'rgba(125, 211, 252, 0.3)' }}>·</span>
                              <span
                                style={{
                                  color: '#94A3B8',
                                  letterSpacing: '0.2em',
                                }}
                              >
                                {tStage.label}
                              </span>
                              {t.activities && t.activities.length > 0 && (
                                <>
                                  <span style={{ color: 'rgba(125, 211, 252, 0.3)' }}>·</span>
                                  <span style={{ color: '#94A3B8' }}>
                                    +{t.activities.length}act
                                  </span>
                                </>
                              )}
                            </div>
                          </div>
                          {isActive && (
                            <span
                              className="text-[9px]"
                              style={{
                                color: tCat.tint,
                                letterSpacing: '0.3em',
                                fontWeight: 600,
                              }}
                            >
                              ●
                            </span>
                          )}
                        </button>
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            setTenantMenuOpen(false);
                            onEditTenant(t);
                          }}
                          className="px-2 flex items-center justify-center"
                          style={{
                            color: '#7DD3FC',
                            borderLeft: '1px solid rgba(125, 211, 252, 0.1)',
                          }}
                          title="Edit this mission"
                        >
                          <Pencil className="h-3.5 w-3.5" />
                        </button>
                        <button
                          type="button"
                          onClick={(e) => {
                            e.stopPropagation();
                            setTenantMenuOpen(false);
                            onDeleteTenant(t);
                          }}
                          className="px-2 flex items-center justify-center"
                          style={{
                            color: '#F87171',
                            borderLeft: '1px solid rgba(125, 211, 252, 0.1)',
                          }}
                          title="Delete this mission"
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </button>
                      </div>
                    );
                  })}
                </div>
                <button
                  type="button"
                  onClick={() => {
                    setTenantMenuOpen(false);
                    onNewTenant();
                  }}
                  className="w-full px-3 py-2 text-[10px] flex items-center gap-2 border-t"
                  style={{
                    borderColor: 'rgba(125, 211, 252, 0.12)',
                    color: '#7DD3FC',
                    fontFamily: 'ui-monospace, monospace',
                    letterSpacing: '0.25em',
                  }}
                >
                  <Plus className="h-3 w-3" /> NEW MISSION
                </button>
              </div>
            )}
          </div>
        ) : (
          <button
            onClick={onNewTenant}
            className="flex items-center gap-2 px-3 py-2 rounded-sm border text-xs"
            style={{
              background: 'rgba(125, 211, 252, 0.06)',
              borderColor: 'rgba(125, 211, 252, 0.4)',
              color: '#7DD3FC',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.2em',
            }}
          >
            <Plus className="h-3.5 w-3.5" /> NEW MISSION
          </button>
        )}

        {/* Tabs */}
        <div
          className="inline-flex rounded-sm p-0.5 border"
          style={{
            background: 'rgba(3, 6, 12, 0.7)',
            borderColor: 'rgba(125, 211, 252, 0.18)',
          }}
          role="tablist"
        >
          {(
            [
              ['mesh', 'CONSTELLATION', Network],
              ['live', 'TELEMETRY', Activity],
              ['timeline', 'CHRONO', Clock],
              ['workbench', 'WORKBENCH', FolderOpen],
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
                  color: active ? '#E0F2FE' : '#5BA8D9',
                  border: active
                    ? '1px solid rgba(125, 211, 252, 0.45)'
                    : '1px solid transparent',
                  fontFamily: 'ui-monospace, monospace',
                  letterSpacing: '0.22em',
                }}
              >
                <Icon className="h-3 w-3" /> {label}
              </button>
            );
          })}
        </div>
      </div>

      {/* ── Telemetry strip ─────────────────────────────────────── */}
      <div
        className="grid grid-cols-2 md:grid-cols-4 gap-px border-t"
        style={{
          borderColor: 'rgba(125, 211, 252, 0.1)',
          background: 'rgba(125, 211, 252, 0.06)',
        }}
      >
        <Stat label="FLEET" value={agentCount.toString()} />
        <Stat
          label="ACTIVE"
          value={runningAgents.length.toString()}
          accent={runningAgents.length > 0}
        />
        <Stat label="OPS" value={totalRuns.toString()} />
        <Stat
          label="EXEC"
          value={
            runningAgents.length > 0
              ? runningAgents.slice(0, 2).join(' + ')
              : '— STANDBY —'
          }
          accent={runningAgents.length > 0}
          mono
        />
      </div>
    </div>
  );
}

// ─── LAUNCH FROM ZERO confirm ───────────────────────────────────────
//
// Destructive bootstrap: wipes every deliverable for this tenant
// AND fires ~22 parallel agents. Two-step confirm: operator must
// type the tenant id literally before LAUNCH enables. Same pattern
// as DELETE TENANT to keep destructive-action UX consistent.

function LaunchFromZeroConfirm({
  tenant,
  onCancel,
  onConfirm,
}: {
  tenant: Tenant;
  onCancel: () => void;
  onConfirm: () => Promise<void> | void;
}) {
  const [typed, setTyped] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const tint = '#FCD34D';
  const ready = typed.trim() === tenant.id;

  const submit = async () => {
    if (!ready) return;
    setSubmitting(true);
    try {
      await onConfirm();
    } finally {
      setSubmitting(false);
    }
  };

  // Estimate of agents that will fire — mirrors the dispatchAll filter
  // logic so the operator knows the blast radius.
  const applicable = QUICK_ACTIONS.filter((a) => {
    if (a.id === 'energy_full') {
      return (
        tenant.category === 'energy' ||
        (tenant.activities ?? []).includes('satellite') ||
        (tenant.activities ?? []).includes('hardware')
      );
    }
    return true;
  });
  const agentTotal = applicable.reduce((sum, a) => sum + a.agents.length, 0);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      style={{ background: 'rgba(3, 6, 12, 0.92)' }}
      onClick={onCancel}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="rounded border p-6 w-full max-w-md space-y-4"
        style={{
          background: 'rgba(12, 16, 24, 0.95)',
          borderColor: tint + '88',
          borderLeft: `4px solid ${tint}`,
          boxShadow: `inset 0 1px 0 ${tint}22`,
          fontFamily: 'ui-monospace, monospace',
        }}
      >
        <div>
          <p
            className="text-[10px]"
            style={{ color: tint, letterSpacing: '0.4em' }}
          >
            ⚠ DESTRUCTIVE BOOTSTRAP · TWO PHASES
          </p>
          <h2
            className="text-xl mt-1"
            style={{ color: '#FDE68A', letterSpacing: '0.18em' }}
          >
            🚀 LAUNCH FROM ZERO
          </h2>
        </div>

        <div
          className="px-3 py-2.5 rounded-sm border text-[11px]"
          style={{
            background: 'rgba(252, 211, 77, 0.05)',
            borderColor: 'rgba(252, 211, 77, 0.25)',
            color: '#FDE68A',
            letterSpacing: '0.04em',
            lineHeight: 1.5,
          }}
        >
          <p style={{ fontWeight: 600 }}>
            {tenant.name}{' '}
            <span style={{ color: '#94A3B8', fontWeight: 'normal' }}>
              · {tenant.id}
            </span>
          </p>
          <ol className="mt-2 list-decimal list-inside space-y-1">
            <li>
              <span style={{ color: '#FCA5A5' }}>WIPE</span> every file under{' '}
              <code style={{ color: '#7DD3FC' }}>
                companies/{tenant.id}/deliverables/
              </code>{' '}
              (unrecoverable)
            </li>
            <li>
              <span style={{ color: '#86EFAC' }}>FIRE</span> {agentTotal}{' '}
              agents across {applicable.length} sprints in parallel
            </li>
            <li>
              Watch CONSTELLATION + WORKBENCH for new artifacts to land
              in ~3–10 min
            </li>
          </ol>
          <p
            className="mt-2"
            style={{ color: '#94A3B8', fontSize: 10 }}
          >
            Memory namespaces, sessions, tenant config and the legacy
            unscoped folder are NOT touched.
          </p>
        </div>

        <div>
          <label
            className="text-[10px] block mb-1"
            style={{ color: '#5BA8D9', letterSpacing: '0.25em' }}
          >
            TYPE <span style={{ color: tint }}>{tenant.id}</span> TO CONFIRM
          </label>
          <input
            type="text"
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            placeholder={tenant.id}
            autoFocus
            className="w-full px-3 py-2 rounded-sm text-sm tabular-nums"
            style={{
              background: 'rgba(3, 6, 12, 0.7)',
              border: `1px solid ${ready ? tint : 'rgba(125, 211, 252, 0.3)'}`,
              color: ready ? '#FDE68A' : '#BAE6FD',
              letterSpacing: '0.06em',
            }}
          />
        </div>

        <div className="flex gap-2 justify-end">
          <button
            type="button"
            onClick={onCancel}
            disabled={submitting}
            className="px-4 py-2 rounded-sm text-[10px]"
            style={{
              background: 'transparent',
              border: '1px solid rgba(125, 211, 252, 0.2)',
              color: '#5BA8D9',
              letterSpacing: '0.25em',
            }}
          >
            CANCEL
          </button>
          <button
            type="button"
            onClick={submit}
            disabled={!ready || submitting}
            className="px-4 py-2 rounded-sm text-[10px]"
            style={{
              background: ready ? '#FCD34D' : 'rgba(252, 211, 77, 0.2)',
              border: `1px solid ${tint}`,
              color: ready ? '#03060c' : '#94A3B8',
              letterSpacing: '0.25em',
              opacity: !ready || submitting ? 0.5 : 1,
              cursor: ready && !submitting ? 'pointer' : 'not-allowed',
              fontWeight: 700,
            }}
          >
            {submitting ? 'LAUNCHING…' : `🚀 WIPE + FIRE ${agentTotal}`}
          </button>
        </div>
      </div>
    </div>
  );
}

// ─── Caution & Warning strip ────────────────────────────────────────
//
// Replicates the C&W panel of a real flight-control console: a row of
// labelled lamps that go amber on caution conditions and red on
// warnings. Master Alarm light at the left amplifies whichever lamp
// is highest-severity. Silence by selection is not yet wired —
// resolve the underlying condition to clear it.

// ─── Mission bar (always-visible tenant switcher) ────────────────────
//
// Pill row of every registered tenant. Active one is highlighted with
// its category accent. Click switches. Edit / delete icons appear on
// hover. Replaces the hidden dropdown — the operator can see every
// company at a glance and switch in one click. Horizontally scrollable
// when there are many.

function MissionBar({
  tenants,
  activeId,
  onSwitchTenant,
  onEditTenant,
  onDeleteTenant,
  onNewTenant,
}: {
  tenants: Tenant[];
  activeId: string | null;
  onSwitchTenant: (id: string | null) => void;
  onEditTenant: (t: Tenant) => void;
  onDeleteTenant: (t: Tenant) => void;
  onNewTenant: () => void;
}) {
  return (
    <div
      className="rounded border overflow-hidden"
      style={{
        background: 'rgba(8, 11, 18, 0.85)',
        borderColor: 'rgba(125, 211, 252, 0.15)',
        fontFamily: 'ui-monospace, monospace',
      }}
    >
      <div
        className="flex items-center gap-2 px-3 py-1.5 border-b text-[10px]"
        style={{
          borderColor: 'rgba(125, 211, 252, 0.08)',
          color: '#5BA8D9',
          letterSpacing: '0.3em',
        }}
      >
        <Building2 className="h-3 w-3" />
        <span>MISSIONS · {tenants.length}</span>
        <span className="ml-auto" style={{ color: '#94A3B8', letterSpacing: '0.1em' }}>
          click to switch · hover for edit/delete
        </span>
      </div>
      <div className="flex items-center gap-2 px-3 py-2 overflow-x-auto">
        {tenants.map((t) => {
          const cat = CATEGORY_META[t.category];
          const stage = STAGE_META[t.stage];
          const isActive = t.id === activeId;
          return (
            <div
              key={t.id}
              className="group relative shrink-0 inline-flex items-stretch rounded-sm border transition-colors"
              style={{
                background: isActive ? cat.tint + '14' : 'rgba(3, 6, 12, 0.5)',
                borderColor: isActive ? cat.tint : 'rgba(125, 211, 252, 0.15)',
                borderLeft: `3px solid ${isActive ? cat.tint : 'transparent'}`,
              }}
            >
              <button
                type="button"
                onClick={() => onSwitchTenant(t.id)}
                className="text-left px-3 py-1.5 flex items-center gap-2 min-w-0"
                title={
                  isActive
                    ? 'Currently active mission'
                    : `Switch to ${t.name} (${cat.label} · ${stage.label})`
                }
              >
                {isActive ? (
                  <span
                    className="inline-block h-1.5 w-1.5 rounded-full shrink-0"
                    style={{
                      background: cat.tint,
                      boxShadow: `0 0 6px ${cat.tint}`,
                      animation: 'pulse 2s infinite',
                    }}
                  />
                ) : (
                  <Building2 className="h-3.5 w-3.5 shrink-0" style={{ color: cat.tint }} />
                )}
                <div className="min-w-0">
                  <div
                    className="text-[11px] leading-tight truncate"
                    style={{
                      color: isActive ? '#E0F2FE' : '#BAE6FD',
                      letterSpacing: '0.06em',
                      fontWeight: isActive ? 600 : 400,
                      maxWidth: 180,
                    }}
                  >
                    {t.name}
                  </div>
                  <div className="flex items-center gap-1.5 text-[9px] mt-0.5">
                    <span style={{ color: cat.tint, letterSpacing: '0.2em' }}>
                      {cat.label}
                    </span>
                    <span style={{ color: 'rgba(125, 211, 252, 0.25)' }}>·</span>
                    <span style={{ color: '#94A3B8', letterSpacing: '0.15em' }}>
                      {stage.label}
                    </span>
                    {t.activities && t.activities.length > 0 && (
                      <>
                        <span style={{ color: 'rgba(125, 211, 252, 0.25)' }}>·</span>
                        <span style={{ color: '#94A3B8' }}>
                          +{t.activities.length}
                        </span>
                      </>
                    )}
                  </div>
                </div>
              </button>
              {/* Hover-revealed action icons */}
              <div
                className="flex items-stretch opacity-0 group-hover:opacity-100 transition-opacity"
                style={{ borderLeft: '1px solid rgba(125, 211, 252, 0.1)' }}
              >
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onEditTenant(t);
                  }}
                  className="px-2 flex items-center justify-center hover:bg-cyan-500/10"
                  style={{ color: '#7DD3FC' }}
                  title="Edit"
                >
                  <Pencil className="h-3 w-3" />
                </button>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onDeleteTenant(t);
                  }}
                  className="px-2 flex items-center justify-center hover:bg-red-500/10"
                  style={{
                    color: '#F87171',
                    borderLeft: '1px solid rgba(125, 211, 252, 0.08)',
                  }}
                  title="Delete"
                >
                  <Trash2 className="h-3 w-3" />
                </button>
              </div>
            </div>
          );
        })}
        {/* New mission CTA */}
        <button
          type="button"
          onClick={onNewTenant}
          className="shrink-0 inline-flex items-center gap-1.5 px-3 py-1.5 rounded-sm border text-[10px]"
          style={{
            background: 'rgba(125, 211, 252, 0.06)',
            borderColor: 'rgba(125, 211, 252, 0.35)',
            borderStyle: 'dashed',
            color: '#7DD3FC',
            letterSpacing: '0.25em',
          }}
          title="Create a new mission"
        >
          <Plus className="h-3 w-3" /> NEW
        </button>
      </div>
    </div>
  );
}

// ─── Quick Actions panel — one-click delegation sprints ─────────────
//
// Curated mission templates that POST a delegation prompt to /webhook
// with the active tenant header. Removes the "what do I type?" friction
// and shows the operator the most useful canned moves for the current
// stage / category. Each card kicks off a parallel multi-agent sprint;
// the constellation animates as the bench picks up the work.

interface QuickAction {
  id: string;
  label: string;
  blurb: string;
  icon: typeof Activity;
  color: string;
  /** Agents to fan out in parallel — each gets its own webhook + session.
   *  Picked from this tenant's bench; the backend will skip any that
   *  aren't actually configured in [agents.<name>] blocks. */
  agents: string[];
  /** Per-agent brief — what work this agent should do for this sprint.
   *  The tenant context is already carried by the X-Octopus-Tenant
   *  header (the daemon prepends a structured preamble), so this only
   *  describes the *work*, not the company. */
  brief: (tenant: Tenant, agent: string) => string;
}

// Per-sprint briefs. Each agent gets a focused, single-responsibility
// prompt that names the deliverable to write via deliverable_write.
// Tenant identity is injected by the gateway (X-Octopus-Tenant header
// → tenant context preamble), so briefs only describe the *work*.
const BRIEFS: Record<string, Record<string, (t: Tenant) => string>> = {
  validate: {
    phd_business: (t) =>
      `Analyze the unit economics of "${t.name}" end-to-end. Surface the 3 most fragile assumptions and how to falsify each in <= 2 weeks. Use memory_recall for prior research on this venture, web_fetch for comparable cohorts, and deliverable_write to save your analysis as "unit-economics.md". Be specific and quantitative.`,
    idea_validator: (t) =>
      `Design the SMALLEST test that would falsify the wedge of "${t.name}" in 2 weeks. State hypothesis, sample size, success threshold, kill criterion. Save via deliverable_write as "validation-test.md".`,
    customer_researcher: (t) =>
      `Produce 5 ICP interview questions for "${t.name}" that would surface the actual JTBD vs the assumed one. Each question has a hypothesis it tests. Save via deliverable_write as "icp-interview-script.md".`,
    competitor_analyst: (t) =>
      `Identify 3 closest analogues to "${t.name}" (successes AND failures). For each: their wedge, their decisive move, the asymmetric advantage they had or lacked. Use web_fetch + knowledge tools. Save via deliverable_write as "competitive-analogues.md".`,
    red_teamer: (t) =>
      `Pre-mortem of "${t.name}" 18 months out. Top 3 failure modes mapped to the failure-mode taxonomy (market / execution / structural / incumbent counter-move / capital). Each with: cited comparable, leading indicator, severity. Save via deliverable_write as "pre-mortem.md".`,
  },
  pre_launch: {
    market_sentiment_analyst: (t) =>
      `Run a SMOKE-test MiroFish simulation (50 agents, 10 rounds) targeting the primary segment of "${t.name}". Use http_request to call MiroFish at MIROFISH_BASE_URL (default http://127.0.0.1:5001). If MiroFish is unreachable, surface that to the operator and stop — do not fabricate. Capture the 3 receipts (graph_id, simulation_id, report_id) and sentiment trajectory T+0/1d/7d/30d. Save via deliverable_write as "sentiment-smoke.md".`,
    red_teamer: (t) =>
      `Incumbent counter-move pre-mortem for "${t.name}" launch in next 6 months. Name the specific incumbent + their cheap response (free-tier bundle / lawsuit / exclusivity / talent-poach / M&A defense). Save via deliverable_write as "counter-move.md".`,
    growth_hacker: (t) =>
      `Design the smallest live experiment that would disconfirm the pre-launch sentiment read for "${t.name}". State channel, sample, expected lift, MDE, kill criterion. Save via deliverable_write as "validation-experiment.md".`,
  },
  capital_plan: {
    deeptech_financier: (t) =>
      `18-month capital stack for "${t.name}" (${t.category}, ${t.stage}). Layers: equity / venture debt / project debt / non-dilutive. Map the IRA / state / EU credits applicable. Section-cited. Save via deliverable_write as "capital-stack.md".`,
    cfo_advisor: (t) =>
      `Cash-flow runway + burn discipline plan for "${t.name}". Monthly burn target, runway scenarios (base / bull / bear), trigger metrics for capital decisions. Save via deliverable_write as "burn-discipline.md".`,
    sovereign_advisor: (t) =>
      `Which sovereign / development-bank doors fit "${t.name}" and when should they be knocked? Mubadala/PIF/Temasek vs CDC/KfW/EIB vs In-Q-Tel. Pre-conditions for each. Save via deliverable_write as "sovereign-doors.md".`,
    latam_solar_ngo_counsel: (t) =>
      `If "${t.name}" added a nonprofit arm to unlock philanthropic + multilateral capital, what vehicle structure works? Cover US 501(c)(3) + LatAm asociación civil + for-profit subsidiary hybrid. Save via deliverable_write as "ngo-vehicle.md".`,
  },
  risk_audit: {
    forensic_auditor: (t) =>
      `Cold-eyed forensic audit of "${t.name}". Triangulate from public exhaust (filings, hiring, web traffic, reviews, court records). Surface 3 weaknesses with evidence trail (≥2 artefacts each, dates within 90-day window). Save via deliverable_write as "forensic-findings.md".`,
    red_teamer: (t) =>
      `Top 3 plan-failure modes the operator is rationalising away for "${t.name}". Severity + leading indicator + decisive disconfirming experiment. Save via deliverable_write as "plan-risks.md".`,
    risk_analyst: (t) =>
      `Risk-tier "${t.name}" by financial / operational / reputational impact. Surface 5 highest-risk items + mitigation path. Save via deliverable_write as "risk-register.md".`,
    security: (t) =>
      `Security surface area of "${t.name}" + 3 highest-impact lowest-effort hardening moves. Save via deliverable_write as "security-hardening.md".`,
  },
  gtm_sprint: {
    marketing: (t) =>
      `30-day GTM strategy for "${t.name}". ICP (firmographic + behavioural + pain), April Dunford positioning frame (alternatives / unique attributes / value / who it's for / market category), media-plan skeleton (channel × audience × creative × spend × dates). Apply neuromarketing skill. Save via deliverable_write as "gtm-strategy.md".`,
    growth_hacker: (t) =>
      `3 acquisition experiments for "${t.name}" with hypothesis + north-star metric + sample size + MDE + kill criterion + measurement plan. Pick channels appropriate to the stage. Save via deliverable_write as "growth-experiments.md".`,
    copywriter: (t) =>
      `Hero headline + sub + primary CTA for "${t.name}" in 3 register variants: mission-control / corporate-trust / disruptor. Each with the neuromarketing mechanic ledger (which principles applied + why). Save via deliverable_write as "hero-copy.md".`,
    content_creator: (t) =>
      `5 SEO-targeted content pieces for "${t.name}" in a pillar+cluster shape. Each with: target intent, SERP feature competing for, internal-link map. Apply neuromarketing skill. Save via deliverable_write as "content-pillar.md".`,
    pricing_strategist: (t) =>
      `Pricing tier structure for "${t.name}" with anchor logic, von Restorff isolation of the recommended tier, willingness-to-pay assumption + falsification test. Save via deliverable_write as "pricing-tiers.md".`,
  },
  energy_full: {
    energy_grid_strategist: (t) =>
      `Revenue stack for "${t.name}" with applicable PPA structures, ISO/RTO market identification, interconnection-queue risk. Save via deliverable_write as "grid-revenue-stack.md".`,
    esg_energy_counsel: (t) =>
      `Applicable disclosure regime for "${t.name}" (CSRD / SEC climate / SB 253) + IRA bonus stack opportunities (§45X / §48 / §45Y / §45Q). Save via deliverable_write as "esg-stack.md".`,
    geospatial_analyst: (t) =>
      `3 most useful satellite data products for "${t.name}" site-selection + performance attribution. Vendor + cost-per-km² + cadence. Save via deliverable_write as "satellite-products.md".`,
    latam_solar_ngo_counsel: (t) =>
      `Vehicle map for "${t.name}" across US + Brasil + Colombia + México + Panamá + Costa Rica. Statute citations + funding playbook (multilateral + climate + bilateral + philanthropic). Save via deliverable_write as "latam-legal-funding.md".`,
    deeptech_financier: (t) =>
      `Project-finance bankability checklist for "${t.name}". What must exist before project debt closes (off-take, insurance, EPC, O&M, technology performance evidence). Save via deliverable_write as "bankability-checklist.md".`,
  },
};

const QUICK_ACTIONS: QuickAction[] = [
  {
    id: 'validate',
    label: 'VALIDATE VENTURE',
    blurb: 'Triangulate against customers, competitors, failure modes',
    icon: CheckCircle2,
    color: '#86EFAC',
    agents: ['phd_business', 'idea_validator', 'customer_researcher', 'competitor_analyst', 'red_teamer'],
    brief: (t, a) => BRIEFS.validate?.[a]?.(t) ?? '',
  },
  {
    id: 'pre_launch',
    label: 'PRE-LAUNCH SENTIMENT',
    blurb: 'MiroFish swarm + go/no-go from sentiment + risks',
    icon: Radio,
    color: '#7DD3FC',
    agents: ['market_sentiment_analyst', 'red_teamer', 'growth_hacker'],
    brief: (t, a) => BRIEFS.pre_launch?.[a]?.(t) ?? '',
  },
  {
    id: 'capital_plan',
    label: 'CAPITAL PLAN',
    blurb: 'Stack the right capital + non-dilutive map',
    icon: Building2,
    color: '#FCD34D',
    agents: ['deeptech_financier', 'cfo_advisor', 'sovereign_advisor', 'latam_solar_ngo_counsel'],
    brief: (t, a) => BRIEFS.capital_plan?.[a]?.(t) ?? '',
  },
  {
    id: 'risk_audit',
    label: 'RISK AUDIT',
    blurb: 'Reverse-engineer + name what the operator misses',
    icon: AlertCircle,
    color: '#F87171',
    agents: ['forensic_auditor', 'red_teamer', 'risk_analyst', 'security'],
    brief: (t, a) => BRIEFS.risk_audit?.[a]?.(t) ?? '',
  },
  {
    id: 'gtm_sprint',
    label: 'GTM SPRINT',
    blurb: 'Marketing + growth + copy + content + pricing',
    icon: Zap,
    color: '#C084FC',
    agents: ['marketing', 'growth_hacker', 'copywriter', 'content_creator', 'pricing_strategist'],
    brief: (t, a) => BRIEFS.gtm_sprint?.[a]?.(t) ?? '',
  },
  {
    id: 'energy_full',
    label: 'ENERGY DEEP DIVE',
    blurb: 'Grid + satellite + ESG + LatAm legal — full stack',
    icon: Network,
    color: '#A7F3D0',
    agents: ['energy_grid_strategist', 'esg_energy_counsel', 'geospatial_analyst', 'latam_solar_ngo_counsel', 'deeptech_financier'],
    brief: (t, a) => BRIEFS.energy_full?.[a]?.(t) ?? '',
  },
];

function QuickActionsPanel({
  tenant,
  onNewTenant,
}: {
  tenant: Tenant | null;
  onNewTenant: () => void;
}) {
  const [busy, setBusy] = useState<string | null>(null);
  const [launchAllBusy, setLaunchAllBusy] = useState(false);
  const [launchAllSummary, setLaunchAllSummary] = useState<{
    fired: number;
    failed: number;
    sprints: number;
    wiped: number;
    at: number;
  } | null>(null);
  const [lastDispatched, setLastDispatched] = useState<{
    id: string;
    at: number;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmingLaunch, setConfirmingLaunch] = useState<Tenant | null>(null);

  // LAUNCH FROM ZERO — true clean bootstrap. Two phases:
  //   1. WIPE all existing deliverables for the tenant (so "from zero"
  //      actually means from zero, not from-where-we-left-off)
  //   2. Fan out every applicable sprint in parallel
  //
  // Each sprint's agents fire in parallel via /api/orchestrate/dispatch
  // (fire-and-forget, returns 202 in ~150ms each). The constellation
  // animates the full bench coming online simultaneously.
  const dispatchAll = useCallback(
    async (t: Tenant) => {
      setLaunchAllBusy(true);
      setError(null);
      setLaunchAllSummary(null);
      const token = localStorage.getItem('zeroclaw_token') ?? '';
      const sprintId = `boot_${Date.now().toString(36)}`;

      // PHASE 1 — wipe existing deliverables under companies/<tenant>/
      let wipedFiles = 0;
      try {
        const wipe = await wipeDeliverables();
        wipedFiles = wipe.files_deleted;
      } catch (err) {
        // Non-fatal: continue even if the wipe fails (e.g. nothing to
        // wipe yet). Surface as a non-blocking warning.
        setError(
          `wipe warning: ${err instanceof Error ? err.message : String(err)} (continuing with dispatch)`,
        );
      }

      // PHASE 2 — collect every applicable agent dispatch.
      const applicable = QUICK_ACTIONS.filter((a) => {
        if (a.id === 'energy_full') {
          return (
            t.category === 'energy' ||
            (t.activities ?? []).includes('satellite') ||
            (t.activities ?? []).includes('hardware')
          );
        }
        return true;
      });

      const all: Array<{ sprint: string; agent: string; brief: string }> = [];
      for (const action of applicable) {
        for (const agent of action.agents) {
          const brief = action.brief(t, agent);
          if (brief) all.push({ sprint: action.id, agent, brief });
        }
      }

      const fired = await Promise.allSettled(
        all.map(async ({ sprint, agent, brief }) => {
          const sessionId = `${sprintId}__${sprint}__${agent}`;
          const res = await fetch(
            `${window.location.origin}/api/orchestrate/dispatch`,
            {
              method: 'POST',
              headers: {
                'Content-Type': 'application/json',
                Authorization: `Bearer ${token}`,
                'X-Octopus-Tenant': t.id,
                'X-Session-Id': sessionId,
              },
              // Direct-agent dispatch: backend routes via
              // run_named_agent → DelegateTool with the gateway
              // observer attached. Bypasses the parent orchestrator
              // LLM entirely (which was looping `delegate · 0ms`).
              body: JSON.stringify({ agent, message: brief }),
            },
          );
          return { sprint, agent, ok: res.ok || res.status === 202 };
        }),
      );

      const okCount = fired.filter(
        (r) => r.status === 'fulfilled' && r.value.ok,
      ).length;
      const failCount = fired.length - okCount;
      setLaunchAllSummary({
        fired: okCount,
        failed: failCount,
        sprints: applicable.length,
        wiped: wipedFiles,
        at: Date.now(),
      });
      if (failCount > 0) {
        setError(`${failCount}/${fired.length} agents failed to start`);
      }
      setLaunchAllBusy(false);
    },
    [],
  );

  // Filter actions that fit the tenant — energy-deep-dive only when
  // category is energy or activities include satellite/hardware.
  const actions = useMemo(() => {
    if (!tenant) return QUICK_ACTIONS;
    return QUICK_ACTIONS.filter((a) => {
      if (a.id === 'energy_full') {
        return (
          tenant.category === 'energy' ||
          (tenant.activities ?? []).includes('satellite') ||
          (tenant.activities ?? []).includes('hardware')
        );
      }
      return true;
    });
  }, [tenant]);

  // Fan out N parallel webhook calls — one per agent — instead of
  // asking a single orchestrator LLM to call delegate(parallel=[...]).
  // Pros: deterministic (no LLM compliance risk), each agent runs in
  // its OWN session_id so the gateway's session-actor pool services
  // them concurrently instead of serializing, the constellation gets
  // distinct per-agent SSE events from the start. The `agents` field
  // on the action definition is the source of truth.
  const dispatch = async (action: QuickAction) => {
    if (!tenant) return;
    setBusy(action.id);
    setError(null);
    const token = localStorage.getItem('zeroclaw_token') ?? '';
    const sprintId = `spr_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;

    // Each agent gets its own session id so the gateway doesn't queue
    // them behind a single session actor. Agent-named sessions also
    // make the workbench / chrono attribution obvious.
    // Fire-and-forget via /api/orchestrate/dispatch — returns 202
    // immediately. The agent loop runs in the background, events
    // stream through SSE so the constellation lights up. No more
    // 30-second /webhook timeouts.
    const fired = await Promise.allSettled(
      action.agents.map(async (agent) => {
        const sessionId = `${sprintId}__${agent}`;
        const brief = action.brief(tenant, agent);
        if (!brief) return { agent, ok: false, error: 'no brief defined' };
        const res = await fetch(
          `${window.location.origin}/api/orchestrate/dispatch`,
          {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              Authorization: `Bearer ${token}`,
              'X-Octopus-Tenant': tenant.id,
              'X-Session-Id': sessionId,
            },
            body: JSON.stringify({
              // Direct-agent dispatch: backend routes this to the
              // named agent's preset (its model + system_prompt +
              // tools) WITHOUT a parent orchestrator LLM. Sidesteps
              // the circuit-breaker loop the parent was hitting.
              agent,
              message: brief,
            }),
          },
        );
        if (!res.ok && res.status !== 202) {
          const txt = await res.text().catch(() => '');
          return { agent, ok: false, error: `HTTP ${res.status} · ${txt.slice(0, 160)}` };
        }
        return { agent, ok: true };
      }),
    );

    const failed = fired
      .map((r) => (r.status === 'fulfilled' ? r.value : { agent: '?', ok: false, error: 'rejected' }))
      .filter((r) => !r.ok);
    if (failed.length === action.agents.length) {
      setError(
        `All ${failed.length} agents failed. First: ${failed[0]?.agent}: ${failed[0]?.error}`,
      );
    } else if (failed.length > 0) {
      setError(
        `${failed.length}/${action.agents.length} agents failed to start: ${failed.map((f) => f.agent).join(', ')}`,
      );
      setLastDispatched({ id: action.id, at: Date.now() });
    } else {
      setLastDispatched({ id: action.id, at: Date.now() });
    }
    setBusy(null);
  };

  // Big, impossible-to-miss header. The accent of the active tenant
  // (or amber when none) tints the bar so the operator's eye lands.
  const accent = tenant
    ? CATEGORY_META[tenant.category].tint
    : '#FCD34D';

  return (
    <div
      className="rounded border overflow-hidden"
      style={{
        background:
          'linear-gradient(180deg, rgba(12, 16, 24, 0.95), rgba(8, 11, 18, 0.92))',
        borderColor: accent + '55',
        borderLeft: `4px solid ${accent}`,
        boxShadow: `inset 0 1px 0 ${accent}22`,
        fontFamily: 'ui-monospace, monospace',
      }}
    >
      {/* Header — large, prominent */}
      <div
        className="flex items-center justify-between gap-3 px-4 py-3 border-b"
        style={{
          borderColor: 'rgba(125, 211, 252, 0.1)',
          background: accent + '08',
        }}
      >
        <div className="flex items-center gap-3">
          <div
            className="flex items-center justify-center rounded-sm"
            style={{
              width: 36,
              height: 36,
              background: 'rgba(3, 6, 12, 0.95)',
              border: `2px solid ${accent}`,
            }}
          >
            <Zap className="h-4 w-4" style={{ color: accent }} />
          </div>
          <div>
            <p
              className="text-[10px]"
              style={{
                color: accent,
                letterSpacing: '0.4em',
                fontWeight: 600,
              }}
            >
              ▶ DEPLOY SPRINT
            </p>
            <p
              className="text-base"
              style={{
                color: '#E0F2FE',
                letterSpacing: '0.12em',
                fontWeight: 500,
                lineHeight: 1.1,
              }}
            >
              {tenant
                ? `ORCHESTRATE ${tenant.name.toUpperCase()}`
                : 'PICK A MISSION FIRST'}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {tenant ? (
            <button
              type="button"
              onClick={() => setConfirmingLaunch(tenant)}
              disabled={busy !== null || launchAllBusy}
              className="px-4 py-2 inline-flex items-center gap-2 rounded-sm border transition-colors"
              style={{
                background: launchAllBusy
                  ? 'rgba(252, 211, 77, 0.18)'
                  : '#FCD34D',
                borderColor: '#FCD34D',
                color: launchAllBusy ? '#FCD34D' : '#03060c',
                letterSpacing: '0.25em',
                fontWeight: 700,
                fontSize: 12,
                cursor: busy || launchAllBusy ? 'progress' : 'pointer',
                opacity: busy && !launchAllBusy ? 0.4 : 1,
              }}
              title="Wipe all existing deliverables for this mission and fire every sprint in parallel"
            >
              {launchAllBusy ? (
                <>⚡ DISPATCHING…</>
              ) : (
                <>🚀 LAUNCH FROM ZERO</>
              )}
            </button>
          ) : (
            <button
              type="button"
              onClick={onNewTenant}
              className="px-3 py-1.5 text-[10px] inline-flex items-center gap-1.5 rounded-sm border"
              style={{
                background: accent + '15',
                borderColor: accent,
                color: accent,
                letterSpacing: '0.25em',
                fontWeight: 600,
              }}
            >
              <Plus className="h-3 w-3" /> NEW MISSION
            </button>
          )}
        </div>
      </div>

      {error && (
        <div
          className="px-3 py-2 text-[10px] border-b"
          style={{
            background: 'rgba(248, 113, 113, 0.08)',
            borderColor: 'rgba(248, 113, 113, 0.2)',
            color: '#FCA5A5',
            letterSpacing: '0.05em',
          }}
        >
          ERR · {error}
        </div>
      )}

      {launchAllSummary && Date.now() - launchAllSummary.at < 30000 && (
        <div
          className="px-4 py-3 border-b flex items-center gap-3"
          style={{
            background: 'rgba(134, 239, 172, 0.08)',
            borderColor: 'rgba(134, 239, 172, 0.25)',
            color: '#86EFAC',
            fontFamily: 'ui-monospace, monospace',
          }}
        >
          <CheckCircle2 className="h-4 w-4 shrink-0" />
          <div className="flex-1">
            <p
              className="text-[11px]"
              style={{ letterSpacing: '0.2em', fontWeight: 600 }}
            >
              {launchAllSummary.wiped > 0 && (
                <span style={{ color: '#FCD34D' }}>
                  WIPED {launchAllSummary.wiped} FILES ·{' '}
                </span>
              )}
              {launchAllSummary.fired} AGENTS FIRED ACROSS {launchAllSummary.sprints}{' '}
              SPRINTS
              {launchAllSummary.failed > 0 && (
                <span style={{ color: '#FCA5A5' }}>
                  {' '}· {launchAllSummary.failed} FAILED
                </span>
              )}
            </p>
            <p
              className="text-[9px] mt-0.5"
              style={{
                color: '#94A3B8',
                letterSpacing: '0.1em',
              }}
            >
              Watch CONSTELLATION for satellite activity · TELEMETRY for the
              event stream · WORKBENCH for deliverables as they land
            </p>
          </div>
        </div>
      )}

      {confirmingLaunch && (
        <LaunchFromZeroConfirm
          tenant={confirmingLaunch}
          onCancel={() => setConfirmingLaunch(null)}
          onConfirm={async () => {
            const t = confirmingLaunch;
            setConfirmingLaunch(null);
            await dispatchAll(t);
          }}
        />
      )}

      {!tenant ? (
        <div
          className="px-4 py-6 text-center"
          style={{
            color: '#94A3B8',
            fontFamily: 'ui-monospace, monospace',
            letterSpacing: '0.05em',
            lineHeight: 1.6,
            fontSize: 12,
          }}
        >
          <p>
            Six 1-click sprints (validate / pre-launch / capital / risk /
            GTM / energy) become available the moment a mission is selected.
          </p>
          <p className="mt-2" style={{ color: '#64748B' }}>
            Each card auto-delegates 4-5 specialists in parallel for the
            active tenant. The constellation animates as the bench picks
            up the work.
          </p>
        </div>
      ) : (
        <div
          className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-px"
          style={{ background: 'rgba(125, 211, 252, 0.06)' }}
        >
          {actions.map((a) => {
            const Icon = a.icon;
            const isBusy = busy === a.id;
            const justDispatched =
              lastDispatched?.id === a.id &&
              Date.now() - lastDispatched.at < 4000;
            const dim = busy !== null && !isBusy;
            return (
              <button
                key={a.id}
                type="button"
                onClick={() => dispatch(a)}
                disabled={isBusy || busy !== null}
                className="text-left p-4 transition-colors disabled:opacity-50 hover:brightness-125"
                style={{
                  background: justDispatched
                    ? a.color + '1c'
                    : 'rgba(3, 6, 12, 0.9)',
                  borderLeft: `3px solid ${justDispatched ? a.color : a.color + '66'}`,
                  cursor: busy ? 'progress' : 'pointer',
                  opacity: dim ? 0.4 : 1,
                }}
                title={`Fans ${a.agents.length} agents in parallel: ${a.agents.join(', ')}`}
              >
                <div className="flex items-center gap-2 mb-1.5">
                  <Icon
                    className="h-4 w-4 shrink-0"
                    style={{ color: a.color }}
                  />
                  <span
                    className="text-[12px]"
                    style={{
                      color: justDispatched ? '#E0F2FE' : a.color,
                      letterSpacing: '0.2em',
                      fontWeight: 600,
                    }}
                  >
                    {a.label}
                  </span>
                  {isBusy && (
                    <span
                      className="ml-auto text-[9px]"
                      style={{ color: a.color, letterSpacing: '0.2em' }}
                    >
                      DISPATCHING {a.agents.length}…
                    </span>
                  )}
                  {justDispatched && !isBusy && (
                    <span
                      className="ml-auto text-[9px] inline-flex items-center gap-1"
                      style={{ color: a.color, letterSpacing: '0.2em' }}
                    >
                      <CheckCircle2 className="h-3 w-3" /> {a.agents.length} FIRED
                    </span>
                  )}
                </div>
                <p
                  className="text-[11px] mb-1"
                  style={{
                    color: '#94A3B8',
                    letterSpacing: '0.04em',
                    lineHeight: 1.4,
                  }}
                >
                  {a.blurb}
                </p>
                <div className="flex flex-wrap gap-1 mt-1">
                  {a.agents.map((ag) => (
                    <span
                      key={ag}
                      className="text-[8px] px-1 py-0.5 rounded-sm"
                      style={{
                        background: a.color + '14',
                        border: `1px solid ${a.color}33`,
                        color: a.color,
                        letterSpacing: '0.05em',
                      }}
                    >
                      {ag}
                    </span>
                  ))}
                </div>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

function CautionWarningStrip({
  connected,
  errorCount,
  lastEventAge,
  runningCount,
}: {
  connected: boolean;
  errorCount: number;
  lastEventAge: number | null;
  runningCount: number;
}) {
  // Discrete conditions:
  //   SIG-LOSS — SSE connection dropped (red)
  //   STALE    — connected but no event in >120 s and at least one ran (amber)
  //   ERR      — at least one error event seen this session (amber/red by count)
  //   QUEUE    — informational, lights when running > 4 (amber)
  const sigLoss = !connected;
  const stale =
    connected && lastEventAge !== null && lastEventAge > 120;
  const errSev: 'off' | 'amber' | 'red' =
    errorCount === 0 ? 'off' : errorCount > 5 ? 'red' : 'amber';
  const queueHigh = runningCount > 4;

  const anyAlarm = sigLoss || errSev === 'red';
  const anyCaution = stale || errSev === 'amber' || queueHigh;
  const masterColor = anyAlarm ? '#F87171' : anyCaution ? '#FCD34D' : '#1E293B';
  const masterActive = anyAlarm || anyCaution;

  return (
    <div
      className="rounded border overflow-hidden flex items-stretch text-[10px]"
      style={{
        background: 'rgba(8, 11, 18, 0.92)',
        borderColor: masterActive ? masterColor + '55' : 'rgba(125, 211, 252, 0.12)',
        fontFamily: 'ui-monospace, monospace',
      }}
    >
      {/* Master Alarm — leftmost, always visible */}
      <div
        className="px-3 py-1.5 flex items-center gap-2 border-r"
        style={{
          borderColor: 'rgba(125, 211, 252, 0.12)',
          background: masterActive ? masterColor + '14' : 'rgba(125, 211, 252, 0.04)',
        }}
      >
        <span
          className="inline-block h-2 w-2 rounded-full"
          style={{
            background: masterColor,
            boxShadow: masterActive ? `0 0 8px ${masterColor}` : 'none',
            animation: anyAlarm ? 'pulse 0.7s infinite' : anyCaution ? 'pulse 1.6s infinite' : undefined,
          }}
        />
        <span
          style={{
            color: masterActive ? masterColor : '#5BA8D9',
            letterSpacing: '0.3em',
            fontWeight: 600,
          }}
        >
          MASTER ALARM
        </span>
      </div>

      {/* Lamps — compact, grid-aligned */}
      <div className="flex flex-1 flex-wrap">
        <CWLamp
          label="SIG-LOSS"
          severity={sigLoss ? 'red' : 'off'}
          detail={sigLoss ? 'DOWNLINK DROP' : 'NOMINAL'}
        />
        <CWLamp
          label="STALE"
          severity={stale ? 'amber' : 'off'}
          detail={
            stale && lastEventAge !== null
              ? `${Math.round(lastEventAge)}S SINCE LAST EVT`
              : lastEventAge !== null
                ? `${Math.round(lastEventAge)}S AGO`
                : 'NO TRAFFIC'
          }
        />
        <CWLamp
          label="ERR"
          severity={errSev}
          detail={`${errorCount} ERRORS THIS SESSION`}
        />
        <CWLamp
          label="QUEUE"
          severity={queueHigh ? 'amber' : 'off'}
          detail={`${runningCount} ACTIVE`}
        />
      </div>
    </div>
  );
}

function CWLamp({
  label,
  severity,
  detail,
}: {
  label: string;
  severity: 'off' | 'amber' | 'red';
  detail: string;
}) {
  const color =
    severity === 'red'
      ? '#F87171'
      : severity === 'amber'
        ? '#FCD34D'
        : 'rgba(91, 168, 217, 0.4)';
  const textColor = severity === 'off' ? '#5BA8D9' : color;
  const active = severity !== 'off';
  return (
    <div
      className="flex-1 min-w-[140px] px-3 py-1.5 flex items-center gap-2 border-r last:border-r-0"
      style={{
        borderColor: 'rgba(125, 211, 252, 0.08)',
        background: active ? color + '0d' : 'transparent',
      }}
    >
      <span
        className="inline-block h-1.5 w-1.5 rounded-full shrink-0"
        style={{
          background: color,
          boxShadow: active ? `0 0 5px ${color}` : 'none',
          animation:
            severity === 'red'
              ? 'pulse 0.8s infinite'
              : severity === 'amber'
                ? 'pulse 1.8s infinite'
                : undefined,
        }}
      />
      <span style={{ color: textColor, letterSpacing: '0.25em', minWidth: 70 }}>
        {label}
      </span>
      <span style={{ color: '#94A3B8', letterSpacing: '0.05em' }} className="truncate text-[9px]">
        {detail}
      </span>
    </div>
  );
}

function ClockReadout({
  label,
  value,
  accent,
}: {
  label: string;
  value: string;
  accent?: boolean;
}) {
  return (
    <div className="inline-flex items-center gap-1.5">
      <span style={{ color: '#5BA8D9', letterSpacing: '0.3em' }}>{label}</span>
      <span
        className="tabular-nums"
        style={{
          color: accent ? '#E0F2FE' : '#BAE6FD',
          letterSpacing: '0.12em',
          fontWeight: accent ? 600 : 400,
        }}
      >
        {value}
      </span>
    </div>
  );
}

function StatusLight({
  label,
  go,
  goColor,
}: {
  label: string;
  go: boolean;
  /** Override the GO colour — used by SIG to flash amber on stale */
  goColor?: string;
}) {
  const color = go ? (goColor ?? '#86EFAC') : 'rgba(91, 168, 217, 0.25)';
  return (
    <div
      className="inline-flex items-center gap-1 px-2 py-0.5"
      style={{
        background: go ? color + '15' : 'transparent',
        borderRight: '1px solid rgba(125, 211, 252, 0.12)',
      }}
    >
      <span
        className="inline-block h-1.5 w-1.5 rounded-full"
        style={{
          background: color,
          boxShadow: go ? `0 0 6px ${color}` : 'none',
          animation: go ? 'pulse 2.4s infinite' : undefined,
        }}
      />
      <span
        style={{
          color: go ? '#BAE6FD' : '#5BA8D9',
          letterSpacing: '0.25em',
          fontSize: 9,
        }}
      >
        {label}
      </span>
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
    <div
      className="px-4 py-2"
      style={{ background: 'rgba(3, 6, 12, 0.85)' }}
    >
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
        className={mono ? 'text-xs truncate tabular-nums' : 'text-lg tabular-nums'}
        style={{
          color: accent ? '#7DD3FC' : '#BAE6FD',
          fontFamily: 'ui-monospace, monospace',
          letterSpacing: mono ? '0.1em' : '0.05em',
          fontWeight: accent ? 600 : 400,
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

// ─── Console panel chrome ──────────────────────────────────────────
//
// Every side-panel block is wrapped in this. Header carries the
// channel ID (PNL-A, PNL-B…) and the panel title in mission-control
// caps. Keeps the layout reading like a real console rack.

function ConsolePanel({
  id,
  title,
  children,
}: {
  id: string;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div
      className="rounded border overflow-hidden"
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
          background: 'rgba(125, 211, 252, 0.04)',
        }}
      >
        <span style={{ color: '#5BA8D9' }}>
          {id} · {title}
        </span>
      </div>
      {children}
    </div>
  );
}

function FleetRow({
  label,
  value,
  total,
  color,
  pulse,
}: {
  label: string;
  value: number;
  total: number;
  color: string;
  pulse?: boolean;
}) {
  const pct = total > 0 ? (value / total) * 100 : 0;
  const active = value > 0;
  return (
    <tr style={{ borderTop: '1px solid rgba(125, 211, 252, 0.05)' }}>
      <td className="px-3 py-1.5">
        <span className="inline-flex items-center gap-1.5">
          <span
            className="inline-block h-1.5 w-1.5 rounded-full"
            style={{
              background: color,
              animation: pulse && active ? 'pulse 1.4s infinite' : undefined,
              boxShadow: active ? `0 0 4px ${color}` : 'none',
            }}
          />
          <span style={{ color: active ? color : '#94A3B8', letterSpacing: '0.2em' }}>{label}</span>
        </span>
      </td>
      <td
        className="px-3 py-1.5 text-right tabular-nums"
        style={{
          color: active ? color : '#5BA8D9',
          fontWeight: active ? 600 : 400,
        }}
      >
        {value.toString().padStart(3, ' ')}
      </td>
      <td className="px-3 py-1.5">
        <div
          className="h-1.5 rounded-sm"
          style={{
            background: 'rgba(125, 211, 252, 0.08)',
            position: 'relative',
            overflow: 'hidden',
          }}
        >
          <div
            style={{
              width: `${pct}%`,
              height: '100%',
              background: color,
              opacity: active ? 0.9 : 0.3,
              transition: 'width 0.4s ease-out',
            }}
          />
        </div>
      </td>
    </tr>
  );
}

function SubRow({
  label,
  value,
  status,
}: {
  label: string;
  value: string;
  status: 'go' | 'amber' | 'red' | 'idle' | 'info';
}) {
  const color =
    status === 'go'
      ? '#86EFAC'
      : status === 'amber'
        ? '#FCD34D'
        : status === 'red'
          ? '#F87171'
          : status === 'info'
            ? '#7DD3FC'
            : '#5BA8D9';
  const pulse = status === 'amber' || status === 'red';
  return (
    <tr style={{ borderTop: '1px solid rgba(125, 211, 252, 0.05)' }}>
      <td className="px-3 py-1.5">
        <span className="inline-flex items-center gap-1.5">
          <span
            className="inline-block h-1.5 w-1.5 rounded-full"
            style={{
              background: color,
              animation: pulse ? 'pulse 1.6s infinite' : undefined,
              boxShadow: status !== 'idle' ? `0 0 4px ${color}` : 'none',
            }}
          />
          <span style={{ color: '#94A3B8', letterSpacing: '0.2em' }}>{label}</span>
        </span>
      </td>
      <td
        className="px-3 py-1.5 text-right tabular-nums"
        style={{
          color: status === 'idle' ? '#5BA8D9' : color,
          letterSpacing: '0.1em',
          fontWeight: status === 'go' || status === 'red' ? 600 : 400,
        }}
      >
        {value}
      </td>
    </tr>
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

      {agent.allowed_tools.length > 0 && (
        <div>
          <p className="text-[9px] mb-1.5" style={labelStyle}>
            TOOLS · {agent.allowed_tools.length}
          </p>
          <div className="flex flex-wrap gap-1">
            {agent.allowed_tools.slice(0, 24).map((tool) => (
              <span
                key={tool}
                className="text-[9px] px-1.5 py-0.5"
                style={{
                  background: 'rgba(125, 211, 252, 0.06)',
                  border: '1px solid rgba(125, 211, 252, 0.18)',
                  color: '#94A3B8',
                  fontFamily: 'ui-monospace, monospace',
                  letterSpacing: '0.04em',
                }}
                title={tool}
              >
                {tool}
              </span>
            ))}
            {agent.allowed_tools.length > 24 && (
              <span
                className="text-[9px] px-1.5 py-0.5"
                style={{
                  color: '#5BA8D9',
                  fontFamily: 'ui-monospace, monospace',
                  letterSpacing: '0.05em',
                }}
              >
                +{agent.allowed_tools.length - 24}
              </span>
            )}
          </div>
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

const ACTIVITIES_META: Record<
  TenantActivity,
  { label: string; tint: string; description: string; bench: string[] }
> = {
  nonprofit: {
    label: 'NONPROFIT / NGO',
    tint: '#67E8F9',
    description:
      'Charitable / asociación civil / fundación arm — even on a for-profit parent. Unlocks philanthropic + multilateral capital.',
    bench: ['ngo_architect', 'latam_solar_ngo_counsel', 'esg_energy_counsel', 'sovereign_advisor'],
  },
  satellite: {
    label: 'SATELLITE / EO',
    tint: '#A5F3FC',
    description:
      'Earth observation, satellite imagery, downstream geospatial analytics, remote sensing, MRV verification.',
    bench: ['geospatial_analyst', 'deeptech_financier', 'esg_energy_counsel'],
  },
  government: {
    label: 'GOVERNMENT / SOVEREIGN',
    tint: '#FDE68A',
    description:
      'Public-sector buyers, sovereign-fund pitches, multilateral procurement, regulated public contracts.',
    bench: ['sovereign_advisor', 'legal_compliance', 'security', 'fintech_counsel', 'risk_analyst'],
  },
  regulated: {
    label: 'REGULATED VERTICAL',
    tint: '#C4B5FD',
    description:
      'Fintech / health / defence / energy / consumer-data — explicit regulator on the line.',
    bench: ['legal_compliance', 'fintech_counsel', 'security', 'risk_analyst', 'esg_energy_counsel'],
  },
  hardware: {
    label: 'HARDWARE',
    tint: '#FDBA74',
    description:
      'Manufactured devices, deployed equipment, energy hardware, mobility — capital-intensive, long lead-times.',
    bench: ['deeptech_financier', 'architect', 'server_architect', 'designer', 'market_researcher'],
  },
};

const CATEGORY_DESCRIPTIONS: Record<TenantCategory, string> = {
  saas: 'Software-as-a-Service · seats / requests / workflows',
  marketplace: 'Two-sided marketplace · GMV, take rate',
  fintech: 'Financial services · regulated, risk-heavy',
  ecommerce: 'Direct-to-consumer / retail',
  agency: 'Services / consulting · time-as-product',
  hardware: 'Physical product · supply chain, certification',
  media: 'Content / publishing / community',
  ai: 'AI-native product · models, GPU costs',
  energy: 'Solar / wind / storage / grid / EV charging · PPAs, ISO markets, IRA stack',
  nonprofit: 'NGO / asociación civil / fundación · grants, climate funds, philanthropy',
  other: 'Custom / undecided',
};

function TenantCreateModal({
  onClose,
  onCreated,
  existing,
}: {
  onClose: () => void;
  onCreated: () => void;
  /** When set, the modal is in EDIT mode: prefills from this tenant
   *  and PATCHes on save instead of POSTing a new one. */
  existing?: Tenant | null;
}) {
  const [name, setName] = useState(existing?.name ?? '');
  const [category, setCategory] = useState<TenantCategory>(existing?.category ?? 'saas');
  const [stage, setStage] = useState<TenantStage>(existing?.stage ?? 'ideation');
  const [activities, setActivities] = useState<TenantActivity[]>(existing?.activities ?? []);
  const [mission, setMission] = useState(existing?.mission ?? '');
  const [description, setDescription] = useState(existing?.description ?? '');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const isEdit = existing !== null && existing !== undefined;

  const toggleActivity = (a: TenantActivity) => {
    setActivities((prev) =>
      prev.includes(a) ? prev.filter((x) => x !== a) : [...prev, a],
    );
  };

  // Recommended bench preview — category set merged with each
  // selected activity's add-on set, dedup-preserving the category
  // order so the primary bench reads first. Mirrors the backend
  // `Tenant::recommended_bench()` so the modal shows the operator
  // exactly what they'll get.
  const recommendedPreview = useMemo<string[]>(() => {
    const categoryMap: Partial<Record<TenantCategory, string[]>> = {
      saas: ['product_manager', 'growth_hacker', 'cto_advisor', 'coder', 'tester'],
      marketplace: ['business_developer', 'product_manager', 'growth_hacker', 'risk_analyst'],
      fintech: ['risk_analyst', 'fintech_counsel', 'security', 'cfo_advisor', 'architect'],
      ecommerce: ['marketing', 'growth_hacker', 'content_creator', 'pricing_strategist'],
      agency: ['business_developer', 'account_executive', 'content_creator', 'designer'],
      hardware: ['architect', 'deeptech_financier', 'designer', 'cfo_advisor'],
      media: ['content_creator', 'scriptwriter', 'marketing', 'growth_hacker'],
      ai: ['cto_advisor', 'architect', 'data_analyst', 'security'],
      energy: [
        'energy_grid_strategist', 'esg_energy_counsel',
        'geospatial_analyst', 'deeptech_financier',
        'sovereign_advisor', 'forensic_auditor',
      ],
      nonprofit: [
        'ngo_architect', 'latam_solar_ngo_counsel',
        'esg_energy_counsel', 'sovereign_advisor',
      ],
      other: ['idea_generator', 'idea_validator', 'red_teamer'],
    };
    const seen = new Set<string>();
    const out: string[] = [];
    for (const a of categoryMap[category] ?? []) {
      if (!seen.has(a)) {
        seen.add(a);
        out.push(a);
      }
    }
    for (const act of activities) {
      for (const a of ACTIVITIES_META[act].bench) {
        if (!seen.has(a)) {
          seen.add(a);
          out.push(a);
        }
      }
    }
    return out;
  }, [category, activities]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      setError('Name is required');
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      if (isEdit && existing) {
        await updateTenant(existing.id, {
          name: name.trim(),
          category,
          stage,
          mission: mission.trim(),
          description: description.trim(),
          activities,
        });
      } else {
        await createTenant({
          name: name.trim(),
          category,
          stage,
          mission: mission.trim(),
          description: description.trim(),
          activities,
        });
      }
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  };

  const tint = CATEGORY_META[category].tint;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      style={{ background: 'rgba(3, 6, 12, 0.92)' }}
      onClick={onClose}
    >
      <form
        onSubmit={submit}
        onClick={(e) => e.stopPropagation()}
        className="rounded border p-6 w-full max-w-2xl max-h-[90vh] overflow-y-auto space-y-4"
        style={{
          background: 'rgba(12, 16, 24, 0.95)',
          borderColor: tint + '88',
          boxShadow: `inset 0 1px 0 ${tint}22`,
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
          {isEdit ? 'EDIT MISSION · UPDATE' : 'NEW COMPANY · DEPLOY'}
        </p>
        <h2
          className="text-xl"
          style={{
            color: '#BAE6FD',
            fontFamily: 'ui-monospace, monospace',
            letterSpacing: '0.18em',
          }}
        >
          {isEdit ? `EDIT TENANT · ${existing!.id.toUpperCase()}` : 'INITIATE TENANT'}
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
          <div className="grid grid-cols-3 sm:grid-cols-4 gap-2">
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
            MISSION (optional · one-liner)
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

        <div>
          <label
            className="text-[10px] block mb-1"
            style={{
              color: '#5BA8D9',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.25em',
            }}
          >
            DESCRIPTION (optional · long-form)
          </label>
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Problem, solution, target market, geography, regulatory footprint, current stage of work, anything sub-agents should know when reasoning about this venture."
            rows={5}
            className="w-full px-3 py-2 rounded text-sm resize-y"
            style={{
              background: 'rgba(3, 6, 12, 0.7)',
              border: '1px solid rgba(125, 211, 252, 0.3)',
              color: '#BAE6FD',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.04em',
              minHeight: 96,
            }}
          />
          <p
            className="text-[9px] mt-1"
            style={{
              color: '#5BA8D9',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.2em',
            }}
          >
            {description.length} CHARS · CARRIED INTO EVERY SUB-AGENT CALL
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
            STAGE
          </label>
          <div className="grid grid-cols-3 sm:grid-cols-6 gap-1">
            {(Object.keys(STAGE_META) as TenantStage[]).map((s) => {
              const meta = STAGE_META[s];
              const active = stage === s;
              return (
                <button
                  type="button"
                  key={s}
                  onClick={() => setStage(s)}
                  className="px-2 py-1.5 rounded text-[9px]"
                  style={{
                    background: active ? tint + '22' : 'rgba(3, 6, 12, 0.5)',
                    border: active
                      ? `1px solid ${tint}`
                      : '1px solid rgba(125, 211, 252, 0.15)',
                    color: active ? tint : '#5BA8D9',
                    fontFamily: 'ui-monospace, monospace',
                    letterSpacing: '0.2em',
                  }}
                >
                  {meta.label}
                </button>
              );
            })}
          </div>
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
            ALSO HAS THESE ACTIVITIES? (multi · adds specialists)
          </label>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            {(Object.keys(ACTIVITIES_META) as TenantActivity[]).map((a) => {
              const meta = ACTIVITIES_META[a];
              const active = activities.includes(a);
              return (
                <button
                  type="button"
                  key={a}
                  onClick={() => toggleActivity(a)}
                  className="text-left p-2.5 rounded transition-colors"
                  style={{
                    background: active ? meta.tint + '15' : 'rgba(3, 6, 12, 0.55)',
                    border: active
                      ? `1px solid ${meta.tint}`
                      : '1px solid rgba(125, 211, 252, 0.15)',
                    fontFamily: 'ui-monospace, monospace',
                  }}
                >
                  <div className="flex items-center gap-2 mb-1">
                    <span
                      className="inline-block h-2.5 w-2.5 rounded-sm shrink-0"
                      style={{
                        background: active ? meta.tint : 'transparent',
                        border: `1px solid ${active ? meta.tint : 'rgba(125, 211, 252, 0.4)'}`,
                        boxShadow: active ? `0 0 5px ${meta.tint}66` : 'none',
                      }}
                    />
                    <span
                      className="text-[10px]"
                      style={{
                        color: active ? meta.tint : '#94A3B8',
                        letterSpacing: '0.25em',
                        fontWeight: 600,
                      }}
                    >
                      {meta.label}
                    </span>
                  </div>
                  <p
                    className="text-[10px] mb-1.5"
                    style={{
                      color: active ? '#CBD5E1' : '#64748B',
                      letterSpacing: '0.02em',
                      lineHeight: 1.4,
                    }}
                  >
                    {meta.description}
                  </p>
                  <div className="flex flex-wrap gap-1">
                    {meta.bench.slice(0, 4).map((agentName) => (
                      <span
                        key={agentName}
                        className="text-[9px] px-1.5 py-0.5"
                        style={{
                          color: active ? meta.tint : '#5BA8D9',
                          background: active ? meta.tint + '0d' : 'transparent',
                          border: `1px solid ${active ? meta.tint + '55' : 'rgba(125, 211, 252, 0.15)'}`,
                          letterSpacing: '0.04em',
                        }}
                      >
                        +{agentName}
                      </span>
                    ))}
                    {meta.bench.length > 4 && (
                      <span
                        className="text-[9px] px-1.5 py-0.5"
                        style={{
                          color: '#5BA8D9',
                          letterSpacing: '0.05em',
                        }}
                      >
                        +{meta.bench.length - 4}
                      </span>
                    )}
                  </div>
                </button>
              );
            })}
          </div>
          <p
            className="text-[9px] mt-1.5"
            style={{
              color: '#94A3B8',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.05em',
            }}
          >
            {activities.length === 0
              ? 'None selected — primary category bench only.'
              : `${activities.length} activity${activities.length === 1 ? '' : 'ies'} → adds specialists below.`}
          </p>
        </div>

        <div>
          <label
            className="text-[10px] block mb-1.5"
            style={{
              color: '#5BA8D9',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.25em',
            }}
          >
            RECOMMENDED BENCH FOR {CATEGORY_META[category].label}
            {activities.length > 0 && (
              <span style={{ color: '#94A3B8' }}>
                {' '}
                + {activities.map((a) => ACTIVITIES_META[a].label.split(' ')[0]).join(' + ')}
              </span>
            )}
          </label>
          <div className="flex flex-wrap gap-1">
            {recommendedPreview.map((agentName) => (
              <span
                key={agentName}
                className="text-[9px] px-1.5 py-0.5"
                style={{
                  background: tint + '12',
                  border: `1px solid ${tint}55`,
                  color: tint,
                  fontFamily: 'ui-monospace, monospace',
                  letterSpacing: '0.04em',
                }}
              >
                {agentName}
              </span>
            ))}
          </div>
          <p
            className="text-[9px] mt-1.5"
            style={{
              color: '#94A3B8',
              fontFamily: 'ui-monospace, monospace',
              letterSpacing: '0.05em',
            }}
          >
            These light up first in CONSTELLATION; the rest of the bench dims.
          </p>
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
            {submitting
              ? isEdit ? 'UPDATING...' : 'DEPLOYING...'
              : isEdit ? 'SAVE CHANGES' : 'DEPLOY'}
          </button>
        </div>
      </form>
    </div>
  );
}

// Re-export delete helper for the future tenant management UI.
export { deleteTenant };

// ─── Workbench tab ──────────────────────────────────────────────────
//
// File browser for everything the bench produces under
// workspace/deliverables/. Three filter pills: DOCS (markdown / text /
// pdf), IMAGES (png/jpg/svg/webp), DATA (json/yaml/csv). Selecting a
// file fetches its content via /api/files/deliverables/raw and renders
// it inline (markdown rendered as text for now, images as <img>,
// JSON pretty-printed). Read-only — no edit / delete here.

type WorkbenchFilter = 'all' | 'docs' | 'images' | 'data';

function workbenchFilterMatches(mime: string, f: WorkbenchFilter): boolean {
  if (f === 'all') return true;
  if (f === 'docs') {
    return (
      mime.startsWith('text/') ||
      mime === 'application/pdf' ||
      mime === 'text/markdown'
    );
  }
  if (f === 'images') return mime.startsWith('image/');
  if (f === 'data') {
    return (
      mime === 'application/json' ||
      mime === 'text/yaml' ||
      mime === 'text/csv' ||
      mime === 'text/toml'
    );
  }
  return false;
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

function WorkbenchView({ events }: { events: SSEEvent[] }) {
  // Active tenant scopes everything: switching tenant in the
  // MissionBar pulls up a fresh tree of just THAT company's
  // artifacts. `apiFetch` already attaches X-Octopus-Tenant
  // automatically, so the server-side filter kicks in for free.
  const { active: activeTenant } = useTenant();
  const [tree, setTree] = useState<DeliverableTreeResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<WorkbenchFilter>('all');
  const [selected, setSelected] = useState<string | null>(null);
  const [content, setContent] = useState<DeliverableReadResponse | null>(null);
  const [contentLoading, setContentLoading] = useState(false);

  // Soft refresh: re-fetch the tree without nuking the selected file
  // (used by SSE auto-refresh so the operator's current preview survives).
  const softRefresh = useCallback(() => {
    getDeliverablesTree()
      .then((data) => setTree(data))
      .catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, []);

  // Hard refresh: clear selection too. Used on mount, tenant switch,
  // and the explicit reload button.
  const refresh = useCallback(() => {
    setLoading(true);
    setError(null);
    setSelected(null);
    setContent(null);
    getDeliverablesTree()
      .then((data) => setTree(data))
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }, []);

  // Reload whenever the active tenant changes. The legacy single-
  // tenant root (when no tenant set) also flows through this branch.
  useEffect(() => {
    refresh();
  }, [refresh, activeTenant?.id]);

  // SSE-driven auto-refresh: when an agent finishes writing a file,
  // the gateway broadcasts a `tool_call` event with name=deliverable_write
  // or file_write. We watch the tail of the event stream and trigger
  // a (debounced) refetch so new artifacts appear without manual reload.
  const lastWriteEventIdRef = useRef<number>(-1);
  const refetchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    // Find the most recent write-ish event index. We only react to NEW
    // ones (index past the last we saw) so flipping back to this tab
    // doesn't trigger N refreshes for backlogged events.
    let latest = -1;
    for (let i = events.length - 1; i >= 0; i--) {
      const e = events[i];
      if (!e) continue;
      const ok =
        (e.type === 'tool_call' && (e.name === 'deliverable_write' || e.name === 'file_write')) ||
        e.type === 'agent_end';
      if (ok) {
        latest = i;
        break;
      }
    }
    if (latest > lastWriteEventIdRef.current) {
      lastWriteEventIdRef.current = latest;
      if (refetchTimerRef.current) clearTimeout(refetchTimerRef.current);
      // Debounce: agents often write 2-3 files in quick succession;
      // batching the refetch keeps the API quiet.
      refetchTimerRef.current = setTimeout(() => {
        softRefresh();
      }, 800);
    }
    return () => {
      if (refetchTimerRef.current) {
        clearTimeout(refetchTimerRef.current);
        refetchTimerRef.current = null;
      }
    };
  }, [events, softRefresh]);

  useEffect(() => {
    if (!selected) {
      setContent(null);
      return;
    }
    setContentLoading(true);
    readDeliverable(selected)
      .then((d) => setContent(d))
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setContentLoading(false));
  }, [selected]);

  // Counts per filter
  const counts = useMemo(() => {
    const c = { all: 0, docs: 0, images: 0, data: 0 };
    if (!tree) return c;
    for (const a of tree.agents) {
      for (const r of a.runs) {
        for (const f of r.files) {
          c.all++;
          if (workbenchFilterMatches(f.mime, 'docs')) c.docs++;
          if (workbenchFilterMatches(f.mime, 'images')) c.images++;
          if (workbenchFilterMatches(f.mime, 'data')) c.data++;
        }
      }
    }
    return c;
  }, [tree]);

  return (
    <div className="grid grid-cols-1 xl:grid-cols-[380px_1fr] gap-4">
      {/* ── Sidebar: tree of agents → runs → files ──────────────── */}
      <aside
        className="rounded border overflow-hidden flex flex-col"
        style={{
          background: 'rgba(12, 16, 24, 0.85)',
          borderColor: 'rgba(125, 211, 252, 0.18)',
          fontFamily: 'ui-monospace, monospace',
          maxHeight: 'calc(100vh - 360px)',
        }}
      >
        <div
          className="px-3 py-2 border-b text-[10px] flex items-center justify-between gap-2"
          style={{
            borderColor: 'rgba(125, 211, 252, 0.1)',
            color: '#5BA8D9',
            letterSpacing: '0.3em',
            background: 'rgba(125, 211, 252, 0.04)',
          }}
        >
          <span className="inline-flex items-center gap-1.5 min-w-0">
            <FolderOpen className="h-3 w-3 shrink-0" />
            <span className="truncate">
              {activeTenant ? activeTenant.name.toUpperCase() : 'NO MISSION'}
            </span>
          </span>
          <button
            type="button"
            onClick={refresh}
            className="shrink-0 px-2 py-0.5 text-[9px] inline-flex items-center gap-1 rounded-sm border"
            style={{
              borderColor: 'rgba(125, 211, 252, 0.3)',
              color: '#7DD3FC',
              letterSpacing: '0.2em',
            }}
            title="Reload tree"
          >
            <RefreshCw className="h-2.5 w-2.5" /> SYNC
          </button>
        </div>

        {/* Filter pills */}
        <div
          className="flex gap-px text-[9px]"
          style={{ background: 'rgba(125, 211, 252, 0.06)' }}
        >
          {(
            [
              ['all', 'ALL', counts.all],
              ['docs', 'DOCS', counts.docs],
              ['images', 'IMAGES', counts.images],
              ['data', 'DATA', counts.data],
            ] as const
          ).map(([id, label, n]) => {
            const active = filter === id;
            return (
              <button
                key={id}
                type="button"
                onClick={() => setFilter(id)}
                className="flex-1 px-2 py-1.5 transition-colors"
                style={{
                  background: active
                    ? 'rgba(125, 211, 252, 0.14)'
                    : 'rgba(3, 6, 12, 0.85)',
                  color: active ? '#E0F2FE' : '#94A3B8',
                  letterSpacing: '0.25em',
                  fontWeight: active ? 600 : 400,
                  borderBottom: active ? '2px solid #7DD3FC' : '2px solid transparent',
                }}
              >
                {label} · {n}
              </button>
            );
          })}
        </div>

        {/* Body */}
        <div className="overflow-y-auto flex-1">
          {loading && (
            <div className="px-3 py-6 text-center text-[10px]" style={{ color: '#5BA8D9' }}>
              ▸ LOADING TREE…
            </div>
          )}
          {error && (
            <div
              className="px-3 py-3 text-[10px] m-2 rounded-sm border"
              style={{
                background: 'rgba(248, 113, 113, 0.08)',
                borderColor: 'rgba(248, 113, 113, 0.25)',
                color: '#FCA5A5',
              }}
            >
              ERR · {error}
            </div>
          )}
          {!loading && !error && tree && tree.agents.length === 0 && (
            <div
              className="px-3 py-6 text-center text-[10px]"
              style={{ color: '#94A3B8', lineHeight: 1.6 }}
            >
              NO ARTIFACTS YET
              <br />
              <span style={{ color: '#5BA8D9' }}>
                deliverables appear here as agents call deliverable_write or file_write
                under workspace/deliverables/
              </span>
            </div>
          )}
          {!loading && !error && tree && tree.agents.length > 0 && (
            <ul className="text-[10px]">
              {tree.agents.map((a) => {
                // Per-agent files in active filter
                const matched = a.runs
                  .flatMap((r) => r.files.map((f) => ({ ...f, run: r })))
                  .filter((f) => workbenchFilterMatches(f.mime, filter));
                if (matched.length === 0) return null;
                return (
                  <li
                    key={a.agent}
                    className="border-b last:border-0"
                    style={{ borderColor: 'rgba(125, 211, 252, 0.08)' }}
                  >
                    <div
                      className="px-3 py-1.5 flex items-center gap-2"
                      style={{
                        background: 'rgba(125, 211, 252, 0.04)',
                        color: '#7DD3FC',
                        letterSpacing: '0.2em',
                        fontWeight: 600,
                      }}
                    >
                      <PixelSigil name={a.agent} size={14} state="idle" />
                      <span className="flex-1 truncate">{a.agent}</span>
                      <span style={{ color: '#94A3B8', fontWeight: 400 }}>
                        {matched.length}
                      </span>
                    </div>
                    <ul>
                      {matched.map((f) => {
                        const isSel = selected === f.rel_path;
                        return (
                          <li key={f.rel_path}>
                            <button
                              type="button"
                              onClick={() => setSelected(f.rel_path)}
                              className="w-full text-left px-3 py-1.5 flex items-center gap-2 transition-colors"
                              style={{
                                background: isSel
                                  ? 'rgba(125, 211, 252, 0.1)'
                                  : 'transparent',
                                borderLeft: `2px solid ${isSel ? '#7DD3FC' : 'transparent'}`,
                                color: isSel ? '#E0F2FE' : '#BAE6FD',
                              }}
                            >
                              {f.mime.startsWith('image/') ? (
                                <Image className="h-3 w-3 shrink-0" style={{ color: '#A5F3FC' }} />
                              ) : (
                                <FileText
                                  className="h-3 w-3 shrink-0"
                                  style={{ color: '#7DD3FC' }}
                                />
                              )}
                              <span className="flex-1 truncate" style={{ letterSpacing: '0.04em' }}>
                                {f.name}
                              </span>
                              <span
                                className="text-[8px]"
                                style={{ color: '#5BA8D9', letterSpacing: '0.15em' }}
                              >
                                {formatBytes(f.size_bytes)}
                              </span>
                            </button>
                            <div
                              className="px-3 pb-1 text-[8px]"
                              style={{ color: '#64748B', letterSpacing: '0.1em' }}
                            >
                              {f.run.date} · {f.run.slug.slice(11) || f.run.slug}
                            </div>
                          </li>
                        );
                      })}
                    </ul>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </aside>

      {/* ── Viewer ──────────────────────────────────────────────── */}
      <div
        className="rounded border overflow-hidden flex flex-col"
        style={{
          background: 'rgba(12, 16, 24, 0.7)',
          borderColor: 'rgba(125, 211, 252, 0.18)',
          fontFamily: 'ui-monospace, monospace',
          minHeight: 480,
          maxHeight: 'calc(100vh - 360px)',
        }}
      >
        <div
          className="px-4 py-2 border-b text-[10px] flex items-center justify-between"
          style={{
            borderColor: 'rgba(125, 211, 252, 0.1)',
            color: '#5BA8D9',
            letterSpacing: '0.3em',
            background: 'rgba(125, 211, 252, 0.04)',
          }}
        >
          <span>
            VIEWER {selected ? `· ${selected}` : ''}
          </span>
          {content && (
            <span style={{ color: '#94A3B8' }}>
              {content.mime} · {formatBytes(content.size_bytes)}
              {content.truncated ? ' · TRUNCATED' : ''}
            </span>
          )}
        </div>
        <div className="overflow-auto flex-1">
          {!selected && (
            <div
              className="h-full flex items-center justify-center text-center text-[11px] p-8"
              style={{ color: '#5BA8D9', letterSpacing: '0.15em' }}
            >
              ▸ SELECT A FILE FROM THE TREE
              <br />
              <span style={{ color: '#94A3B8', letterSpacing: '0.04em', marginTop: 8, display: 'block' }}>
                Documents, images, JSON, CSV all render here
              </span>
            </div>
          )}
          {selected && contentLoading && (
            <div className="p-8 text-[10px]" style={{ color: '#5BA8D9' }}>
              ▸ LOADING…
            </div>
          )}
          {selected && content && !contentLoading && (
            <WorkbenchFileBody data={content} />
          )}
        </div>
      </div>
    </div>
  );
}

function WorkbenchFileBody({ data }: { data: DeliverableReadResponse }) {
  const isImage = data.mime.startsWith('image/');
  const isText = data.encoding === 'utf-8';
  const isJson = data.mime === 'application/json' && isText;
  if (isImage) {
    return (
      <div className="p-4 flex items-center justify-center" style={{ minHeight: 360 }}>
        <img
          src={`data:${data.mime};base64,${data.content}`}
          alt={data.path}
          style={{
            maxWidth: '100%',
            maxHeight: 'calc(100vh - 460px)',
            border: '1px solid rgba(125, 211, 252, 0.2)',
            borderRadius: 4,
          }}
        />
      </div>
    );
  }
  if (isJson) {
    let pretty = data.content;
    try {
      pretty = JSON.stringify(JSON.parse(data.content), null, 2);
    } catch {
      // leave as-is
    }
    return (
      <pre
        className="p-4 text-[11px] tabular-nums whitespace-pre-wrap"
        style={{
          color: '#BAE6FD',
          fontFamily: 'ui-monospace, monospace',
          letterSpacing: '0.02em',
          lineHeight: 1.55,
        }}
      >
        {pretty}
      </pre>
    );
  }
  if (isText) {
    return (
      <pre
        className="p-4 text-[11px] whitespace-pre-wrap"
        style={{
          color: '#E0F2FE',
          fontFamily: 'ui-monospace, monospace',
          letterSpacing: '0.02em',
          lineHeight: 1.6,
        }}
      >
        {data.content}
      </pre>
    );
  }
  return (
    <div
      className="p-8 text-[11px] text-center"
      style={{ color: '#94A3B8', letterSpacing: '0.05em', lineHeight: 1.6 }}
    >
      Binary file ({data.mime}, {formatBytes(data.size_bytes)}). Inline preview
      not supported. Inspect via the host filesystem at{' '}
      <code style={{ color: '#7DD3FC' }}>workspace/deliverables/{data.path}</code>.
    </div>
  );
}

// ─── Delete confirmation modal ──────────────────────────────────────
//
// Two-step confirm: the operator must type the tenant id literally
// before the DELETE button enables. Mirrors GitHub / Stripe destructive-
// action patterns — the typing requirement is the friction that
// catches "I clicked the wrong row" mistakes.

function TenantDeleteConfirm({
  tenant,
  isActive,
  onCancel,
  onConfirm,
}: {
  tenant: Tenant;
  isActive: boolean;
  onCancel: () => void;
  onConfirm: () => Promise<void> | void;
}) {
  const [typed, setTyped] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const tint = '#F87171';
  const ready = typed.trim() === tenant.id;

  const submit = async () => {
    if (!ready) return;
    setSubmitting(true);
    setError(null);
    try {
      await onConfirm();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setSubmitting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4"
      style={{ background: 'rgba(3, 6, 12, 0.92)' }}
      onClick={onCancel}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="rounded border p-6 w-full max-w-md space-y-4"
        style={{
          background: 'rgba(12, 16, 24, 0.95)',
          borderColor: tint + '88',
          borderLeft: `3px solid ${tint}`,
          boxShadow: `inset 0 1px 0 ${tint}22`,
          fontFamily: 'ui-monospace, monospace',
        }}
      >
        <div>
          <p className="text-[10px]" style={{ color: tint, letterSpacing: '0.4em' }}>
            ⚠ DESTRUCTIVE · CANNOT BE UNDONE
          </p>
          <h2
            className="text-xl mt-1"
            style={{
              color: '#FCA5A5',
              letterSpacing: '0.18em',
            }}
          >
            DELETE TENANT
          </h2>
        </div>

        <div
          className="px-3 py-2.5 rounded-sm border"
          style={{
            background: 'rgba(248, 113, 113, 0.06)',
            borderColor: 'rgba(248, 113, 113, 0.25)',
            color: '#FECACA',
            fontSize: 11,
            letterSpacing: '0.04em',
            lineHeight: 1.5,
          }}
        >
          <p className="font-bold mb-1" style={{ color: '#FCA5A5' }}>
            {tenant.name}
            <span style={{ color: '#94A3B8', fontWeight: 'normal' }}> · {tenant.id}</span>
          </p>
          <p>
            Removes the tenant from the registry. The agents' memory namespaces
            are NOT cleared — those persist independently. Knowledge graph,
            session transcripts, and cron jobs that reference this tenant
            continue to exist.
          </p>
          {isActive && (
            <p className="mt-2" style={{ color: '#FCD34D' }}>
              ⚠ This is the currently ACTIVE tenant. The dashboard will
              auto-switch to another (or to none) after deletion.
            </p>
          )}
        </div>

        <div>
          <label
            className="text-[10px] block mb-1"
            style={{ color: '#5BA8D9', letterSpacing: '0.25em' }}
          >
            TYPE <span style={{ color: tint }}>{tenant.id}</span> TO CONFIRM
          </label>
          <input
            type="text"
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            placeholder={tenant.id}
            autoFocus
            className="w-full px-3 py-2 rounded-sm text-sm tabular-nums"
            style={{
              background: 'rgba(3, 6, 12, 0.7)',
              border: `1px solid ${ready ? tint : 'rgba(125, 211, 252, 0.3)'}`,
              color: ready ? '#FECACA' : '#BAE6FD',
              letterSpacing: '0.06em',
            }}
          />
        </div>

        {error && (
          <p className="text-[11px]" style={{ color: '#F87171', letterSpacing: '0.05em' }}>
            ERR · {error}
          </p>
        )}

        <div className="flex gap-2 justify-end">
          <button
            type="button"
            onClick={onCancel}
            disabled={submitting}
            className="px-4 py-2 rounded-sm text-[10px]"
            style={{
              background: 'transparent',
              border: '1px solid rgba(125, 211, 252, 0.2)',
              color: '#5BA8D9',
              letterSpacing: '0.25em',
            }}
          >
            CANCEL
          </button>
          <button
            type="button"
            onClick={submit}
            disabled={!ready || submitting}
            className="px-4 py-2 rounded-sm text-[10px]"
            style={{
              background: ready ? 'rgba(248, 113, 113, 0.15)' : 'rgba(248, 113, 113, 0.05)',
              border: `1px solid ${ready ? tint : 'rgba(248, 113, 113, 0.2)'}`,
              color: ready ? '#FECACA' : '#5BA8D9',
              letterSpacing: '0.25em',
              opacity: !ready || submitting ? 0.5 : 1,
              cursor: ready && !submitting ? 'pointer' : 'not-allowed',
              fontWeight: 600,
            }}
          >
            {submitting ? 'DELETING...' : '⚠ DELETE'}
          </button>
        </div>
      </div>
    </div>
  );
}
