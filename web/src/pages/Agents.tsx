import { useEffect, useMemo, useRef, useState } from 'react';
import ForceGraph2D from 'react-force-graph-2d';
import { Bot, Search, Wrench, Cpu, Layers, Database, LayoutGrid, Network } from 'lucide-react';
import { getAgents, type AgentInfo } from '@/lib/api';
import { t } from '@/lib/i18n';

type ViewMode = 'cards' | 'graph';

const NODE_COLOR = {
  agent: '#7DD3FC',
  tool: '#A78BFA',
  namespace: '#86EFAC',
  skills: '#FBBF24',
  provider: '#F472B6',
} as const;

interface AgentGraphNode {
  id: string;
  name: string;
  kind: keyof typeof NODE_COLOR;
  size: number;
}

interface AgentGraphLink {
  source: string;
  target: string;
  rel: string;
}

function buildAgentGraph(agents: AgentInfo[]): {
  nodes: AgentGraphNode[];
  links: AgentGraphLink[];
} {
  const nodes = new Map<string, AgentGraphNode>();
  const links: AgentGraphLink[] = [];
  const add = (id: string, name: string, kind: AgentGraphNode['kind'], size = 4) => {
    if (!nodes.has(id)) nodes.set(id, { id, name, kind, size });
  };

  for (const a of agents) {
    const aid = `agent:${a.name}`;
    add(aid, a.name, 'agent', 8);

    const pid = `provider:${a.provider}`;
    add(pid, a.provider, 'provider', 5);
    links.push({ source: aid, target: pid, rel: 'uses_provider' });

    if (a.memory_namespace) {
      const nid = `ns:${a.memory_namespace}`;
      add(nid, a.memory_namespace, 'namespace', 5);
      links.push({ source: aid, target: nid, rel: 'memory' });
    }
    if (a.skills_directory) {
      const sid = `skills:${a.skills_directory}`;
      add(sid, a.skills_directory, 'skills', 4);
      links.push({ source: aid, target: sid, rel: 'skills' });
    }
    for (const tool of a.allowed_tools) {
      const tid = `tool:${tool}`;
      add(tid, tool, 'tool', 3);
      links.push({ source: aid, target: tid, rel: 'tool' });
    }
  }

  return { nodes: Array.from(nodes.values()), links };
}

export default function Agents() {
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [view, setView] = useState<ViewMode>(() => {
    try {
      return (localStorage.getItem('octopus_agents_view') as ViewMode) || 'cards';
    } catch {
      return 'cards';
    }
  });
  const [hovered, setHovered] = useState<AgentGraphNode | null>(null);
  const graphRef = useRef<HTMLDivElement>(null);
  const [graphSize, setGraphSize] = useState({ w: 800, h: 600 });

  useEffect(() => {
    getAgents()
      .then(setAgents)
      .catch((err) => setError(err.message))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    try {
      localStorage.setItem('octopus_agents_view', view);
    } catch {
      /* ignore */
    }
  }, [view]);

  useEffect(() => {
    if (view !== 'graph') return;
    const el = graphRef.current;
    if (!el) return;
    const measure = () => {
      const r = el.getBoundingClientRect();
      setGraphSize({ w: Math.max(320, r.width), h: Math.max(360, r.height) });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [view]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return agents;
    return agents.filter(
      (a) =>
        a.name.toLowerCase().includes(q) ||
        a.model.toLowerCase().includes(q) ||
        a.provider.toLowerCase().includes(q) ||
        a.system_prompt_summary.toLowerCase().includes(q),
    );
  }, [agents, search]);

  const graph = useMemo(() => buildAgentGraph(filtered), [filtered]);

  const stats = useMemo(() => {
    const counts = new Map<string, number>();
    for (const n of graph.nodes) counts.set(n.kind, (counts.get(n.kind) ?? 0) + 1);
    return Array.from(counts.entries()).sort((a, b) => b[1] - a[1]);
  }, [graph]);

  if (error) {
    return (
      <div className="p-6 animate-fade-in">
        <div
          className="rounded-2xl border p-4"
          style={{
            background: 'rgba(239, 68, 68, 0.08)',
            borderColor: 'rgba(239, 68, 68, 0.2)',
            color: '#f87171',
          }}
        >
          {t('agents.load_error')}: {error}
        </div>
      </div>
    );
  }

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

  return (
    <div className="p-6 space-y-6 animate-fade-in">
      <div className="flex items-end justify-between gap-4 flex-wrap">
        <div>
          <h1
            className="text-2xl font-semibold tracking-tight"
            style={{ color: 'var(--pc-text-primary)' }}
          >
            {t('agents.title')}
          </h1>
          <p className="text-sm mt-1" style={{ color: 'var(--pc-text-muted)' }}>
            {agents.length} {t('agents.subtitle')}
          </p>
        </div>
        <div className="flex items-center gap-3 flex-wrap">
          <div
            className="inline-flex rounded-xl p-1 border"
            style={{ background: 'var(--pc-bg-elevated)', borderColor: 'var(--pc-border)' }}
            role="tablist"
          >
            <button
              role="tab"
              aria-selected={view === 'cards'}
              onClick={() => setView('cards')}
              className="px-3 py-1.5 text-xs font-medium rounded-lg inline-flex items-center gap-1.5 transition-all"
              style={{
                background: view === 'cards' ? 'var(--pc-accent-glow)' : 'transparent',
                color: view === 'cards' ? 'var(--pc-accent-light)' : 'var(--pc-text-muted)',
                border:
                  view === 'cards' ? '1px solid var(--pc-accent-dim)' : '1px solid transparent',
              }}
            >
              <LayoutGrid className="h-3.5 w-3.5" /> Cards
            </button>
            <button
              role="tab"
              aria-selected={view === 'graph'}
              onClick={() => setView('graph')}
              className="px-3 py-1.5 text-xs font-medium rounded-lg inline-flex items-center gap-1.5 transition-all"
              style={{
                background: view === 'graph' ? 'var(--pc-accent-glow)' : 'transparent',
                color: view === 'graph' ? 'var(--pc-accent-light)' : 'var(--pc-text-muted)',
                border:
                  view === 'graph' ? '1px solid var(--pc-accent-dim)' : '1px solid transparent',
              }}
            >
              <Network className="h-3.5 w-3.5" /> Graph
            </button>
          </div>
          <div className="relative w-full max-w-sm">
            <Search
              className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4"
              style={{ color: 'var(--pc-text-faint)' }}
            />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={t('agents.search')}
              className="input-electric w-full pl-10 pr-4 py-2.5 text-sm"
            />
          </div>
        </div>
      </div>

      {view === 'cards' &&
        (filtered.length === 0 ? (
          <div
            className="rounded-2xl border p-8 text-center text-sm"
            style={{
              background: 'var(--pc-bg-elevated)',
              borderColor: 'var(--pc-border)',
              color: 'var(--pc-text-muted)',
            }}
          >
            {t('agents.empty')}
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {filtered.map((agent) => {
              const isOpen = expanded === agent.name;
              return (
                <button
                  key={agent.name}
                  onClick={() => setExpanded(isOpen ? null : agent.name)}
                  className="text-left rounded-2xl border p-5 transition-all hover:border-(--pc-accent-dim)"
                  style={{
                    background: 'var(--pc-bg-elevated)',
                    borderColor: isOpen ? 'var(--pc-accent-dim)' : 'var(--pc-border)',
                    cursor: 'pointer',
                  }}
                >
                  <div className="flex items-start gap-3 mb-3">
                    <div
                      className="shrink-0 h-10 w-10 rounded-xl flex items-center justify-center"
                      style={{
                        background: 'var(--pc-accent-glow)',
                        border: '1px solid var(--pc-accent-dim)',
                      }}
                    >
                      <Bot className="h-5 w-5" style={{ color: 'var(--pc-accent)' }} />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2 flex-wrap">
                        <h3
                          className="font-semibold tracking-tight truncate"
                          style={{ color: 'var(--pc-text-primary)' }}
                        >
                          {agent.name}
                        </h3>
                        {agent.agentic && (
                          <span
                            className="text-[10px] uppercase tracking-wider px-1.5 py-0.5 rounded"
                            style={{
                              background: 'var(--pc-accent-glow)',
                              color: 'var(--pc-accent-light)',
                              border: '1px solid var(--pc-accent-dim)',
                            }}
                          >
                            agentic
                          </span>
                        )}
                      </div>
                      <p
                        className="text-xs mt-0.5 font-mono truncate"
                        style={{ color: 'var(--pc-text-muted)' }}
                        title={`${agent.provider} / ${agent.model}`}
                      >
                        {agent.provider} · {agent.model}
                      </p>
                    </div>
                  </div>

                  {agent.system_prompt_summary && (
                    <p
                      className="text-sm leading-relaxed line-clamp-3 mb-3"
                      style={{ color: 'var(--pc-text-secondary)' }}
                    >
                      {agent.system_prompt_summary}
                    </p>
                  )}

                  <div
                    className="flex flex-wrap gap-x-4 gap-y-1 text-[11px]"
                    style={{ color: 'var(--pc-text-faint)' }}
                  >
                    <span className="inline-flex items-center gap-1">
                      <Layers className="h-3 w-3" /> depth {agent.max_depth}
                    </span>
                    <span className="inline-flex items-center gap-1">
                      <Cpu className="h-3 w-3" /> iter {agent.max_iterations}
                    </span>
                    {agent.allowed_tools.length > 0 && (
                      <span className="inline-flex items-center gap-1">
                        <Wrench className="h-3 w-3" /> {agent.allowed_tools.length} tools
                      </span>
                    )}
                    {agent.memory_namespace && (
                      <span className="inline-flex items-center gap-1">
                        <Database className="h-3 w-3" /> {agent.memory_namespace}
                      </span>
                    )}
                  </div>

                  {isOpen && (
                    <div
                      className="mt-4 pt-4 border-t space-y-2 text-xs"
                      style={{ borderColor: 'var(--pc-border)', color: 'var(--pc-text-muted)' }}
                    >
                      {agent.allowed_tools.length > 0 && (
                        <div>
                          <span style={{ color: 'var(--pc-text-faint)' }}>
                            {t('agents.allowed_tools')}:
                          </span>{' '}
                          <span className="font-mono">{agent.allowed_tools.join(', ')}</span>
                        </div>
                      )}
                      {agent.skills_directory && (
                        <div>
                          <span style={{ color: 'var(--pc-text-faint)' }}>
                            {t('agents.skills_dir')}:
                          </span>{' '}
                          <span className="font-mono">{agent.skills_directory}</span>
                        </div>
                      )}
                      {!agent.has_system_prompt && (
                        <div style={{ color: 'var(--pc-text-faint)' }}>
                          {t('agents.no_prompt')}
                        </div>
                      )}
                    </div>
                  )}
                </button>
              );
            })}
          </div>
        ))}

      {view === 'graph' && (
        <div className="grid grid-cols-1 lg:grid-cols-[1fr_240px] gap-4">
          <div
            ref={graphRef}
            className="rounded-2xl border relative overflow-hidden"
            style={{
              background: 'rgba(3, 6, 12, 0.92)',
              borderColor: 'var(--pc-border)',
              height: 'calc(100vh - 220px)',
              minHeight: '480px',
            }}
          >
            <ForceGraph2D
              graphData={graph}
              width={graphSize.w}
              height={graphSize.h}
              backgroundColor="rgba(3, 6, 12, 0)"
              nodeRelSize={4}
              linkColor={() => 'rgba(125, 211, 252, 0.18)'}
              linkDirectionalArrowLength={3}
              linkDirectionalArrowRelPos={0.94}
              nodeCanvasObject={(node, ctx, scale) => {
                const n = node as AgentGraphNode & { x?: number; y?: number };
                if (n.x === undefined || n.y === undefined) return;
                const color = NODE_COLOR[n.kind];
                ctx.beginPath();
                ctx.arc(n.x, n.y, n.size, 0, 2 * Math.PI);
                ctx.fillStyle = color;
                ctx.fill();
                if (n.kind === 'agent') {
                  ctx.beginPath();
                  ctx.arc(n.x, n.y, n.size + 3, 0, 2 * Math.PI);
                  ctx.strokeStyle = 'rgba(125, 211, 252, 0.5)';
                  ctx.lineWidth = 1.2;
                  ctx.stroke();
                }
                if (scale > 0.9 || n.kind === 'agent') {
                  const fontSize = (n.kind === 'agent' ? 12 : 10) / scale;
                  ctx.font = `${fontSize}px ui-monospace, SFMono-Regular, Menlo, monospace`;
                  ctx.fillStyle = 'rgba(244, 244, 245, 0.85)';
                  ctx.textAlign = 'center';
                  ctx.textBaseline = 'top';
                  const label = n.name.length > 24 ? n.name.slice(0, 23) + '…' : n.name;
                  ctx.fillText(label, n.x, n.y + n.size + 2);
                }
              }}
              onNodeHover={(node) => {
                if (!node) return setHovered(null);
                setHovered(node as unknown as AgentGraphNode);
              }}
              cooldownTime={4000}
              warmupTicks={60}
            />
            {hovered && (
              <div
                className="absolute top-3 left-3 max-w-sm pointer-events-none p-3 rounded-lg backdrop-blur-md border"
                style={{
                  background: 'rgba(10, 10, 14, 0.85)',
                  borderColor: 'rgba(125, 211, 252, 0.2)',
                }}
              >
                <p
                  className="text-[10px] uppercase tracking-[0.2em] mb-1"
                  style={{ color: NODE_COLOR[hovered.kind] }}
                >
                  {hovered.kind}
                </p>
                <p
                  className="text-sm font-mono break-all"
                  style={{ color: 'var(--pc-text-primary)' }}
                >
                  {hovered.name}
                </p>
              </div>
            )}
          </div>

          <aside
            className="rounded-2xl border p-4 self-start space-y-4"
            style={{ background: 'var(--pc-bg-elevated)', borderColor: 'var(--pc-border)' }}
          >
            <div>
              <h2
                className="text-xs uppercase tracking-[0.2em] font-semibold mb-2"
                style={{ color: 'var(--pc-text-muted)' }}
              >
                Node types
              </h2>
              <ul className="space-y-1.5">
                {stats.map(([kind, count]) => (
                  <li key={kind} className="flex items-center gap-2 text-sm">
                    <span
                      className="inline-block h-2.5 w-2.5 rounded-full"
                      style={{ background: NODE_COLOR[kind as keyof typeof NODE_COLOR] }}
                    />
                    <span style={{ color: 'var(--pc-text-secondary)' }}>{kind}</span>
                    <span className="ml-auto" style={{ color: 'var(--pc-text-muted)' }}>
                      {count}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
            <div>
              <h2
                className="text-xs uppercase tracking-[0.2em] font-semibold mb-2"
                style={{ color: 'var(--pc-text-muted)' }}
              >
                Total
              </h2>
              <p className="text-sm" style={{ color: 'var(--pc-text-secondary)' }}>
                {graph.nodes.length} nodes · {graph.links.length} edges
              </p>
            </div>
            <div>
              <h2
                className="text-xs uppercase tracking-[0.2em] font-semibold mb-2"
                style={{ color: 'var(--pc-text-muted)' }}
              >
                Legend
              </h2>
              <ul className="text-xs space-y-1.5" style={{ color: 'var(--pc-text-secondary)' }}>
                <li>Scroll to zoom · drag to pan</li>
                <li>Hover a node for details</li>
                <li>Edges: agent → provider / namespace / skills / tool</li>
              </ul>
            </div>
          </aside>
        </div>
      )}
    </div>
  );
}
