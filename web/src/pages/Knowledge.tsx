// Knowledge — render the Graphify knowledge graph for the active
// workspace. Uses `react-force-graph-2d` for the layout because the
// existing Three.js bundle is already in the app's vendor chunk and
// a 2D force layout reads better than 3D for code-structure graphs
// (most viewers spend their time scanning labels).

import { useEffect, useMemo, useRef, useState } from 'react';
import ForceGraph2D from 'react-force-graph-2d';
import { Network, RefreshCcw, AlertCircle, Zap } from 'lucide-react';
import {
  buildKnowledgeGraph,
  getKnowledgeGraph,
  type GraphifyGraph,
} from '../lib/api';

interface GraphNode {
  id: string;
  label: string;
  type?: string;
  raw: Record<string, unknown>;
}

interface GraphEdge {
  source: string;
  target: string;
  label?: string;
  raw: Record<string, unknown>;
}

interface NormalizedGraph {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

const TYPE_COLOR: Record<string, string> = {
  module: '#22d3ee',
  function: '#a78bfa',
  class: '#f472b6',
  file: '#fbbf24',
  concept: '#86efac',
  entity: '#fda4af',
  decision: '#7dd3fc',
  pattern: '#c084fc',
};

function pickLabel(raw: Record<string, unknown>, fallback: string): string {
  const candidates = ['label', 'name', 'title', 'id'];
  for (const key of candidates) {
    const v = raw[key];
    if (typeof v === 'string' && v.length > 0) return v;
  }
  return fallback;
}

function pickType(raw: Record<string, unknown>): string | undefined {
  const candidates = ['type', 'node_type', 'kind', 'category'];
  for (const key of candidates) {
    const v = raw[key];
    if (typeof v === 'string' && v.length > 0) return v;
  }
  return undefined;
}

function pickEdgeEnd(raw: Record<string, unknown>, key: string): string | null {
  const v = raw[key];
  if (typeof v === 'string') return v;
  if (typeof v === 'number') return String(v);
  return null;
}

/**
 * Graphify writes graph.json with a flexible shape — historically
 * `{nodes: [{id, label, type, ...}], edges: [{source, target, ...}]}`,
 * but networkx also emits `{nodes: [...], links: [...]}`. Normalize
 * both into a single shape the force-graph component can consume.
 */
function normalizeGraph(raw: GraphifyGraph): NormalizedGraph | null {
  if ('empty' in raw && raw.empty) return null;
  const rawNodes = Array.isArray((raw as { nodes?: unknown }).nodes)
    ? ((raw as { nodes: Array<Record<string, unknown>> }).nodes)
    : [];
  const rawEdges =
    Array.isArray((raw as { edges?: unknown }).edges)
      ? ((raw as { edges: Array<Record<string, unknown>> }).edges)
      : Array.isArray((raw as { links?: unknown }).links)
      ? ((raw as { links: Array<Record<string, unknown>> }).links)
      : [];

  const nodes: GraphNode[] = rawNodes
    .map((n, i) => {
      const id = pickEdgeEnd(n, 'id') ?? String(i);
      return {
        id,
        label: pickLabel(n, id),
        type: pickType(n),
        raw: n,
      };
    });

  const validIds = new Set(nodes.map((n) => n.id));
  const edges: GraphEdge[] = [];
  for (const e of rawEdges) {
    const source = pickEdgeEnd(e, 'source') ?? pickEdgeEnd(e, 'from');
    const target = pickEdgeEnd(e, 'target') ?? pickEdgeEnd(e, 'to');
    if (!source || !target) continue;
    if (!validIds.has(source) || !validIds.has(target)) continue;
    const label =
      typeof e.label === 'string'
        ? e.label
        : typeof e.relation === 'string'
        ? e.relation
        : typeof e.type === 'string'
        ? e.type
        : undefined;
    const edge: GraphEdge = label !== undefined
      ? { source, target, label, raw: e }
      : { source, target, raw: e };
    edges.push(edge);
  }

  return { nodes, edges };
}

export default function Knowledge() {
  const [graph, setGraph] = useState<GraphifyGraph | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [hoveredNode, setHoveredNode] = useState<GraphNode | null>(null);
  const [building, setBuilding] = useState(false);
  const [buildError, setBuildError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 800, h: 600 });

  const refresh = () => {
    setLoading(true);
    setError(null);
    getKnowledgeGraph()
      .then((data) => setGraph(data))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  };

  const triggerBuild = async () => {
    setBuilding(true);
    setBuildError(null);
    try {
      const result = await buildKnowledgeGraph();
      if (!result.success) {
        const detail =
          result.error ??
          result.stderr ??
          `Graphify exited with code ${result.exit_code ?? 'unknown'}`;
        setBuildError(detail);
        return;
      }
      refresh();
    } catch (e: unknown) {
      setBuildError(e instanceof Error ? e.message : String(e));
    } finally {
      setBuilding(false);
    }
  };

  useEffect(refresh, []);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const update = () => {
      const r = el.getBoundingClientRect();
      setSize({ w: Math.max(320, r.width), h: Math.max(360, r.height) });
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const normalized = useMemo(() => (graph ? normalizeGraph(graph) : null), [graph]);

  const stats = useMemo(() => {
    if (!normalized) return null;
    const types = new Map<string, number>();
    for (const n of normalized.nodes) {
      const k = n.type ?? 'untyped';
      types.set(k, (types.get(k) ?? 0) + 1);
    }
    return {
      total: normalized.nodes.length,
      edges: normalized.edges.length,
      types: Array.from(types.entries()).sort((a, b) => b[1] - a[1]),
    };
  }, [normalized]);

  return (
    <div className="space-y-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold flex items-center gap-2" style={{ color: 'var(--pc-text-primary)' }}>
            <Network className="h-6 w-6" style={{ color: 'var(--pc-accent)' }} />
            Knowledge Graph
          </h1>
          <p className="text-sm mt-1" style={{ color: 'var(--pc-text-muted)' }}>
            Graphify-derived semantic graph of the active workspace.
            {stats && (
              <>
                {' '}
                <span style={{ color: 'var(--pc-text-secondary)' }}>
                  {stats.total} nodes / {stats.edges} edges
                </span>
              </>
            )}
          </p>
        </div>
        <button
          onClick={refresh}
          className="btn-electric flex items-center gap-2 px-3 py-1.5 text-sm"
          disabled={loading}
        >
          <RefreshCcw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
          {loading ? 'Loading…' : 'Refresh'}
        </button>
      </div>

      {error && (
        <div className="card p-4 flex items-start gap-3" style={{ borderColor: 'rgba(248, 113, 113, 0.4)' }}>
          <AlertCircle className="h-5 w-5 mt-0.5" style={{ color: '#f87171' }} />
          <div>
            <p className="font-medium" style={{ color: '#fca5a5' }}>Failed to load graph</p>
            <p className="text-sm mt-1" style={{ color: 'var(--pc-text-secondary)' }}>{error}</p>
          </div>
        </div>
      )}

      {!error && graph && 'empty' in graph && graph.empty && (
        <div className="card p-6 space-y-4">
          <div>
            <p className="font-medium mb-2" style={{ color: 'var(--pc-text-primary)' }}>
              No knowledge graph generated yet
            </p>
            <p className="text-sm" style={{ color: 'var(--pc-text-secondary)' }}>
              Build the graph now (runs <code className="font-mono text-[11px]">graphify update .</code> in
              the workspace). First run can take a few minutes if semantic extraction is enabled.
            </p>
          </div>

          <button
            onClick={triggerBuild}
            disabled={building}
            className="btn-electric flex items-center gap-2 px-4 py-2 text-sm"
          >
            <Zap className={`h-4 w-4 ${building ? 'animate-pulse' : ''}`} />
            {building ? 'Building graph… (may take minutes)' : 'Build graph now'}
          </button>

          {buildError && (
            <div className="flex items-start gap-2 text-sm" style={{ color: '#fca5a5' }}>
              <AlertCircle className="h-4 w-4 mt-0.5" />
              <div>
                <p className="font-medium">Build failed</p>
                <pre className="text-xs whitespace-pre-wrap mt-1" style={{ color: 'var(--pc-text-secondary)' }}>
                  {buildError}
                </pre>
                <p className="text-xs mt-2" style={{ color: 'var(--pc-text-muted)' }}>
                  Verify <code className="font-mono">graphify</code> is on the daemon's PATH
                  (<code className="font-mono">pip install graphifyy</code>), or run the
                  command manually from a terminal.
                </p>
              </div>
            </div>
          )}

          <details className="text-xs" style={{ color: 'var(--pc-text-muted)' }}>
            <summary className="cursor-pointer">Manual alternatives</summary>
            <pre
              className="text-xs p-3 mt-2 rounded-md overflow-x-auto"
              style={{ background: 'rgba(0,0,0,0.4)', color: 'var(--pc-text-secondary)' }}
            >{`# From a terminal in the workspace
pip install graphifyy
graphify update .

# Or via the agent in chat
zeroclaw chat 'use the graphify tool with action=init to build the graph'`}</pre>
          </details>
        </div>
      )}

      {normalized && (
        <div className="grid grid-cols-1 lg:grid-cols-[1fr_280px] gap-4">
          <div
            ref={containerRef}
            className="card relative overflow-hidden"
            style={{ height: 'calc(100vh - 240px)', minHeight: '480px', background: 'rgba(8, 10, 16, 0.85)' }}
          >
            <ForceGraph2D
              graphData={{
                nodes: normalized.nodes.map((n) => ({ id: n.id, name: n.label, type: n.type, raw: n.raw })),
                links: normalized.edges.map((e) => ({ source: e.source, target: e.target, label: e.label })),
              }}
              width={size.w}
              height={size.h}
              backgroundColor="rgba(8, 10, 16, 0)"
              nodeRelSize={4}
              linkColor={() => 'rgba(255, 255, 255, 0.18)'}
              linkDirectionalArrowLength={3}
              linkDirectionalArrowRelPos={0.92}
              nodeCanvasObject={(node, ctx, globalScale) => {
                const n = node as { id: string; name: string; type?: string; x?: number; y?: number };
                if (n.x === undefined || n.y === undefined) return;
                const color = (n.type && TYPE_COLOR[n.type]) || '#a1a1aa';
                ctx.beginPath();
                ctx.arc(n.x, n.y, 4, 0, 2 * Math.PI);
                ctx.fillStyle = color;
                ctx.fill();
                if (globalScale > 1.2) {
                  const fontSize = 10 / globalScale;
                  ctx.font = `${fontSize}px system-ui, -apple-system, sans-serif`;
                  ctx.fillStyle = 'rgba(244, 244, 245, 0.85)';
                  ctx.textAlign = 'center';
                  ctx.textBaseline = 'top';
                  const max = 28;
                  const text = n.name.length > max ? n.name.slice(0, max - 1) + '…' : n.name;
                  ctx.fillText(text, n.x, n.y + 6);
                }
              }}
              onNodeHover={(node) => {
                if (!node) return setHoveredNode(null);
                const n = node as unknown as { id: string; name: string; type?: string; raw: Record<string, unknown> };
                setHoveredNode({ id: n.id, label: n.name, type: n.type, raw: n.raw });
              }}
              cooldownTime={4000}
              warmupTicks={60}
            />
            {hoveredNode && (
              <div
                className="absolute top-3 left-3 max-w-sm pointer-events-none p-3 rounded-lg backdrop-blur-md border"
                style={{
                  background: 'rgba(10, 10, 14, 0.85)',
                  borderColor: 'rgba(255, 255, 255, 0.1)',
                }}
              >
                <p className="text-[10px] uppercase tracking-[0.2em] mb-1" style={{ color: 'var(--pc-text-muted)' }}>
                  {hoveredNode.type ?? 'node'}
                </p>
                <p className="text-sm font-medium break-all" style={{ color: 'var(--pc-text-primary)' }}>
                  {hoveredNode.label}
                </p>
              </div>
            )}
          </div>

          <aside className="card p-4 space-y-4 self-start">
            <div>
              <h2 className="text-xs uppercase tracking-[0.2em] font-semibold mb-2" style={{ color: 'var(--pc-text-muted)' }}>
                Node types
              </h2>
              {stats && stats.types.length > 0 ? (
                <ul className="space-y-1.5">
                  {stats.types.map(([type, count]) => (
                    <li key={type} className="flex items-center gap-2 text-sm">
                      <span
                        className="inline-block h-2.5 w-2.5 rounded-full"
                        style={{ background: TYPE_COLOR[type] ?? '#a1a1aa' }}
                      />
                      <span style={{ color: 'var(--pc-text-secondary)' }}>{type}</span>
                      <span className="ml-auto" style={{ color: 'var(--pc-text-muted)' }}>{count}</span>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-sm" style={{ color: 'var(--pc-text-muted)' }}>No type metadata.</p>
              )}
            </div>
            <div>
              <h2 className="text-xs uppercase tracking-[0.2em] font-semibold mb-2" style={{ color: 'var(--pc-text-muted)' }}>
                Tips
              </h2>
              <ul className="text-xs space-y-1.5" style={{ color: 'var(--pc-text-secondary)' }}>
                <li>Scroll to zoom, drag to pan.</li>
                <li>Hover a node to see its full label.</li>
                <li>Re-run <code className="font-mono text-[11px]" style={{ color: 'var(--pc-accent)' }}>graphify init .</code> after large code changes to refresh.</li>
              </ul>
            </div>
          </aside>
        </div>
      )}
    </div>
  );
}
