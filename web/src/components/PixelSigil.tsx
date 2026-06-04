// Procedural pixel-art sigil generator.
//
// Hashes the agent name into a deterministic 8x8 left-right-mirrored
// glyph (so we get 4×8 = 32 unique cells per sprite). Each cell is a
// solid square — the rendering uses crisp-edge pixels so it reads as
// 8-bit even at large sizes. The colour palette is picked by category
// so engineering agents read blue, finance agents read gold, etc.
//
// This is intentionally cheap (no images shipped, no font dependency)
// and unique per agent. Reuse across the dashboard for consistent
// identity.

const PALETTES: Record<string, [string, string, string]> = {
  // [primary, accent, shadow]
  engineering: ['#7DD3FC', '#0EA5E9', '#075985'],
  business: ['#C084FC', '#9333EA', '#581C87'],
  revenue: ['#86EFAC', '#22C55E', '#14532D'],
  finance: ['#FCD34D', '#F59E0B', '#78350F'],
  csuite: ['#F9A8D4', '#EC4899', '#831843'],
  risk: ['#FCA5A5', '#EF4444', '#7F1D1D'],
  ideation: ['#FDBA74', '#F97316', '#7C2D12'],
  research: ['#A5F3FC', '#06B6D4', '#155E75'],
  // Counsel / regulatory specialists — slate-violet so they read as
  // legal, distinct from the red-tinted "risk" family.
  legal: ['#C4B5FD', '#8B5CF6', '#4C1D95'],
  // Mission-driven / nonprofit / climate — sage so it reads
  // separate from "revenue" green.
  impact: ['#A7F3D0', '#10B981', '#064E3B'],
  // Sovereign / institutional — antique gold, distinct from
  // "finance" yellow.
  sovereign: ['#FDE68A', '#D97706', '#78350F'],
  // Academic / research-grade analysis — indigo.
  academic: ['#A5B4FC', '#6366F1', '#312E81'],
  default: ['#94A3B8', '#64748B', '#1E293B'],
};

// Map agent name → which palette
const ROLE_PALETTES: Record<string, keyof typeof PALETTES> = {
  // Engineering
  coder: 'engineering',
  designer: 'engineering',
  reviewer: 'engineering',
  tester: 'engineering',
  qa: 'engineering',
  cicd: 'engineering',
  devops: 'engineering',
  docs: 'engineering',
  planner: 'engineering',
  architect: 'engineering',
  server_architect: 'engineering',
  db_designer: 'engineering',
  adr_writer: 'engineering',
  // Idea / research
  idea_generator: 'ideation',
  idea_validator: 'research',
  customer_researcher: 'research',
  competitor_analyst: 'research',
  market_researcher: 'research',
  red_teamer: 'risk',
  pivot_strategist: 'ideation',
  // Business / GTM
  marketing: 'business',
  content_creator: 'business',
  scriptwriter: 'business',
  business_developer: 'business',
  product_manager: 'business',
  business_analyst: 'business',
  growth_hacker: 'business',
  pricing_strategist: 'finance',
  // Revenue
  sdr_outbound: 'revenue',
  account_executive: 'revenue',
  customer_success: 'revenue',
  call_support: 'revenue',
  copywriter: 'business',
  negotiator: 'revenue',
  // Finance & risk
  finance_controller: 'finance',
  risk_analyst: 'risk',
  drilling_risk_analyst: 'risk',
  ethical_hacker: 'risk',
  tenant_operator: 'csuite',
  // Vertical-industry operators
  hospital_operations: 'research',
  defense_strategist: 'sovereign',
  mining_energy_analyst: 'impact',
  construction_analyst: 'engineering',
  cfo_advisor: 'finance',
  deeptech_financier: 'finance',
  // C-suite
  ceo_advisor: 'csuite',
  cto_advisor: 'engineering',
  // Security / legal / regulatory counsel
  security: 'risk',
  legal_compliance: 'legal',
  fintech_counsel: 'legal',
  esg_energy_counsel: 'legal',
  // Mission-driven / sovereign / academic specialists
  ngo_architect: 'impact',
  sovereign_advisor: 'sovereign',
  phd_business: 'academic',
  // Investigative / intelligence / data specialists
  forensic_auditor: 'risk',
  geospatial_analyst: 'research',
  energy_grid_strategist: 'impact',
  latam_solar_ngo_counsel: 'legal',
  market_sentiment_analyst: 'research',
  // Data
  data_analyst: 'research',
  // Orchestrator
  orchestrator: 'csuite',
};

function paletteFor(name: string): [string, string, string] {
  const key = ROLE_PALETTES[name] ?? 'default';
  return PALETTES[key] ?? PALETTES.default!;
}

// FNV-1a hash → deterministic + cheap, plenty of bits for 32 cells.
function hashName(name: string): number {
  let h = 2166136261 >>> 0;
  for (let i = 0; i < name.length; i++) {
    h ^= name.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

interface PixelSigilProps {
  name: string;
  size?: number;
  /** State affects rendering: running adds glow, error tints red */
  state?: 'idle' | 'running' | 'done' | 'error';
}

export function PixelSigil({ name, size = 32, state = 'idle' }: PixelSigilProps) {
  const [primary, accent, shadow] = paletteFor(name);
  const hash = hashName(name);
  // 8x8 grid, left half driven by hash, mirrored to right
  const cells: { x: number; y: number; layer: number }[] = [];
  // Pre-compute 32 cells (4 wide × 8 tall) — each gets a 2-bit layer
  // value: 0 empty, 1 shadow, 2 primary, 3 accent.
  const layers: number[][] = Array.from({ length: 8 }, () => Array(8).fill(0));
  let bitstream = hash;
  // Use BigInt-equivalent extension by rehashing for the lower 4 cols.
  let bitstream2 = hashName(name + '#layer');
  for (let y = 0; y < 8; y++) {
    for (let x = 0; x < 4; x++) {
      const bits = (bitstream & 0x3) as 0 | 1 | 2 | 3;
      bitstream >>>= 2;
      // Bias top + bottom rows toward emptiness so the silhouette
      // reads as a creature/face rather than a solid block.
      const edge = y === 0 || y === 7 || (x === 0 && (y === 1 || y === 6));
      const layer = edge && bits < 2 ? 0 : bits;
      layers[y]![x] = layer;
      // Mirror to the right half
      layers[y]![7 - x] = layer;
      if (bitstream === 0) {
        bitstream = bitstream2;
        bitstream2 = hashName(name + '#layer2');
      }
    }
  }
  // Force a small "eye" detail in row 3 cols 2 + 5 if those are empty
  if (layers[3]![2] === 0) layers[3]![2] = 3;
  if (layers[3]![5] === 0) layers[3]![5] = 3;

  for (let y = 0; y < 8; y++) {
    for (let x = 0; x < 8; x++) {
      const layer = layers[y]![x]!;
      if (layer > 0) cells.push({ x, y, layer });
    }
  }

  const colorFor = (layer: number) =>
    state === 'error'
      ? layer === 1
        ? '#7F1D1D'
        : layer === 2
          ? '#EF4444'
          : '#FCA5A5'
      : layer === 1
        ? shadow
        : layer === 2
          ? primary
          : accent;

  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 8 8"
      style={{
        imageRendering: 'pixelated',
        // Hard edges, no anti-aliasing
        shapeRendering: 'crispEdges',
      }}
    >
      {state === 'running' && (
        <rect width="8" height="8" fill={primary} fillOpacity="0.08" />
      )}
      {cells.map((c, i) => (
        <rect
          key={i}
          x={c.x}
          y={c.y}
          width="1"
          height="1"
          fill={colorFor(c.layer)}
        />
      ))}
    </svg>
  );
}

export const ROLE_PALETTE_KEYS = PALETTES;
