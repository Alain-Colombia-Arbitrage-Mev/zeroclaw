// Tenant create / edit modal — full-screen form for spinning up a new
// company (mission name, category, stage, activities, mission statement,
// description) or editing an existing one. Renders a live preview of the
// recommended agent bench based on the (category × activities) combo so
// the operator sees what they'll get before saving.
//
// Extracted from Orchestrator.tsx (was inline, ~540 LOC) without behaviour
// changes. The CATEGORY_DESCRIPTIONS table moves with it because nothing
// else in the file consumes it.

import { useMemo, useState } from 'react';
import {
  createTenant,
  updateTenant,
  type Tenant,
  type TenantActivity,
  type TenantCategory,
  type TenantStage,
} from '@/lib/api';
import {
  ACTIVITIES_META,
  CATEGORY_META,
  STAGE_META,
} from '../Orchestrator';

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

export default TenantCreateModal;
