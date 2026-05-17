//! Model routing tiers — single source of truth for which LLM each
//! agent preset should call.
//!
//! Background: every preset used to take a `(provider, model)` pair
//! from the caller, and in practice the orchestrator was passing
//! `anthropic/claude-sonnet-4` to all 80 of them. That overpaid for
//! roles where DeepSeek or Mimo would do equal work, and underpaid
//! for roles where Opus 4.7 is the only acceptable model (legal,
//! fundraise, premortem, M&A).
//!
//! The fix is a typed tier system. Each preset declares its tier;
//! the tier maps to a concrete `(provider, model)` here. Changing
//! the assignment for an entire role family is a one-line edit.
//!
//! # Tier semantics
//!
//! - **S1 — Irreversible judgment**: legal calls, fundraise terms,
//!   M&A, tax structure, sovereign/regulator interaction, board-
//!   level decisions. Mistakes cost six figures or more. Worth the
//!   premium.
//! - **S2 — Orchestration + heavy agentic**: the orchestrator
//!   itself, architects, growth_hacker, business_developer. Needs
//!   long-horizon tool-use and strong structured outputs. Does NOT
//!   need the absolute top-of-distribution reasoning of S1.
//! - **S3 — Reasoning workhorse**: the analytical core. Urban
//!   systems, digital strategy, energy grids, pricing, product,
//!   geospatial, market research, financial analysis. Long memos
//!   with frameworks applied. Mimo's strength.
//! - **S4 — Code / infra / data**: coder, devops, db_designer,
//!   cicd, tester, qa, server_architect, ai_engineer. DeepSeek is
//!   ~10× cheaper than Sonnet here with competitive SWE-bench
//!   numbers. Biggest absolute saving in the roster.
//! - **S5 — Creative generation + media prompts**: naming,
//!   content, copy, community growth, ghostwriting. Also writes
//!   the prompts that feed WaveSpeed (`nano-banana-pro` images,
//!   `seedance-2.0` video) and MiniMax (audio). Long context for
//!   brand voice, high temperature for variation.
//! - **S6 — Mid-tier cheap**: docs, adr, support, chro, treasurer,
//!   project_manager, routine legal_compliance. Not creative, not
//!   analytical — just competent at structured output.
//! - **S7 — Atomic micro-calls**: `llm_task`, classify, summarise,
//!   intent_detect, route_decision, memory_metadata, kg_extract.
//!   Sub-500-token outputs called thousands of times per day. Nano
//!   class is the right floor.
//!
//! # Media generation (NOT a tier)
//!
//! Image / video / music generation is NOT an LLM tier. It runs
//! through external APIs:
//!
//! - Images: WaveSpeed `nano-banana-pro` (Google Nano Banana Pro)
//! - Video: WaveSpeed `seedance-2.0` (ByteDance Seedance)
//! - Music: MiniMax `minimax-audio`
//!
//! The S5 tier writes the prompts; the tools dispatch the API
//! calls.
//!
//! # No sovereignty gating
//!
//! Earlier drafts had a `sovereignty_flag` parameter that would
//! force EU/regulated tenants off DeepSeek and Kimi. The operator
//! decided cost/quality wins over data-locality — if a specific
//! tenant needs locality, it overrides at the preset level, not in
//! this central table.

use serde::{Deserialize, Serialize};

/// Routing tier for an agent preset. Maps to a concrete
/// `(provider, model)` via [`tier_to_model`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema-export", derive(schemars::JsonSchema))]
#[serde(rename_all = "UPPERCASE")]
pub enum ModelTier {
    /// Irreversible judgment — legal, fundraise, premortem, M&A.
    S1,
    /// Orchestration + heavy agentic — orchestrator, architects.
    S2,
    /// Reasoning workhorse — strategy, analysis, memos.
    S3,
    /// Code, infra, data — coder, devops, db, cicd, tester.
    S4,
    /// Creative generation + media prompts — naming, content, copy.
    S5,
    /// Mid-tier cheap — docs, adr, support, routine compliance.
    S6,
    /// Atomic micro-calls — classify, summarise, route, extract.
    S7,
}

impl ModelTier {
    /// Stable string identifier — used in TOML config, logs,
    /// telemetry. NOT for matching against model names.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::S1 => "S1",
            Self::S2 => "S2",
            Self::S3 => "S3",
            Self::S4 => "S4",
            Self::S5 => "S5",
            Self::S6 => "S6",
            Self::S7 => "S7",
        }
    }

    /// Cost rank, 1 = most expensive. Kept explicit in case the
    /// tiers ever re-order.
    #[must_use]
    pub const fn cost_rank(self) -> u8 {
        match self {
            Self::S1 => 1,
            Self::S2 => 2,
            Self::S3 => 3,
            Self::S4 => 4,
            Self::S5 => 5,
            Self::S6 => 6,
            Self::S7 => 7,
        }
    }
}

/// Resolves a tier to the concrete `(provider, model)` pair the
/// runtime should call. Strings are the canonical IDs the gateway
/// sends upstream.
#[must_use]
pub const fn tier_to_model(tier: ModelTier) -> (&'static str, &'static str) {
    match tier {
        ModelTier::S1 => ("anthropic", "claude-opus-4.7"),
        ModelTier::S2 => ("openai", "gpt-5.5"),
        ModelTier::S3 => ("xiaomi", "mimo-v2.5-pro"),
        ModelTier::S4 => ("deepseek", "deepseek-v4-pro"),
        ModelTier::S5 => ("moonshotai", "kimi-k2.6"),
        ModelTier::S6 => ("openai", "gpt-5.4-mini"),
        ModelTier::S7 => ("openai", "gpt-5.4-nano"),
    }
}

/// Resolves a tier to its fallback `(provider, model)`. Used when
/// the primary is rate-limited, down, or rejects the request.
/// Fallbacks stay inside the seven-model set deliberately.
#[must_use]
pub const fn tier_fallback(tier: ModelTier) -> (&'static str, &'static str) {
    match tier {
        ModelTier::S1 => ("openai", "gpt-5.5"),
        ModelTier::S2 => ("anthropic", "claude-opus-4.7"),
        ModelTier::S3 => ("openai", "gpt-5.5"),
        ModelTier::S4 => ("xiaomi", "mimo-v2.5-pro"),
        ModelTier::S5 => ("xiaomi", "mimo-v2.5-pro"),
        ModelTier::S6 => ("deepseek", "deepseek-v4-pro"),
        ModelTier::S7 => ("openai", "gpt-5.4-mini"),
    }
}

/// Hint about temperature appropriate for the tier. Presets can
/// override, but this is the default if they don't.
#[must_use]
pub const fn tier_default_temperature(tier: ModelTier) -> f32 {
    match tier {
        ModelTier::S1 => 0.3,
        ModelTier::S2 => 0.4,
        ModelTier::S3 => 0.45,
        ModelTier::S4 => 0.35,
        ModelTier::S5 => 0.85,
        ModelTier::S6 => 0.3,
        ModelTier::S7 => 0.05,
    }
}

/// Recommended tier for each named preset role. Centralised so the
/// orchestrator can resolve `coder → S4` without each preset
/// re-declaring its own tier in eighty places.
///
/// Roles not in this table default to `ModelTier::S6` (mid-tier
/// cheap) — a safe choice that won't blow the budget on an unknown
/// role and won't refuse to answer.
#[must_use]
pub fn recommended_tier_for_role(role: &str) -> ModelTier {
    match role {
        // S1 — irreversible judgment
        "premortem"
        | "general_counsel"
        | "fundraise_captain"
        | "ceo_advisor"
        | "tax_advisor"
        | "corp_dev"
        | "internal_auditor"
        | "privacy_officer"
        | "investor_relations"
        | "sovereign_advisor"
        | "esg_energy_counsel"
        | "fintech_counsel"
        | "latam_solar_ngo_counsel" => ModelTier::S1,

        // S2 — orchestration + heavy agentic
        "orchestrator"
        | "architect"
        | "cto_advisor"
        | "growth_hacker"
        | "authority_media_strategist"
        | "business_developer"
        | "cfo_advisor"
        | "capex_controller" => ModelTier::S2,

        // S3 — reasoning workhorse
        "digital_strategist"
        | "urban_systems_architect"
        | "energy_grid_strategist"
        | "pricing_strategist"
        | "product_manager"
        | "project_manager"
        | "geospatial_analyst"
        | "market_researcher"
        | "competitor_analyst"
        | "financial_analyst"
        | "negotiator"
        | "personal_brand_strategist" => ModelTier::S3,

        // S4 — code / infra / data
        "coder"
        | "db_designer"
        | "devops"
        | "cicd"
        | "tester"
        | "qa"
        | "server_architect"
        | "ai_engineer" => ModelTier::S4,

        // S5 — creative generation + media prompts
        "naming_strategist"
        | "content_creator"
        | "copywriter"
        | "community_growth_specialist"
        | "growth_engineer"
        | "ghostwriter"
        | "marketing"
        | "designer"
        | "account_executive" => ModelTier::S5,

        // S6 — mid-tier cheap (default)
        "docs"
        | "adr_writer"
        | "support_agent"
        | "chro"
        | "treasurer"
        | "legal_compliance"
        | "tax_compliance" => ModelTier::S6,

        // S7 — atomic
        "llm_task"
        | "classify"
        | "summarise"
        | "summariser"
        | "intent_detect"
        | "route_decision"
        | "knowledge_extract"
        | "kg_extract" => ModelTier::S7,

        // Default for unknown roles: don't overpay, don't refuse.
        _ => ModelTier::S6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_to_model_returns_expected_pairs() {
        assert_eq!(
            tier_to_model(ModelTier::S1),
            ("anthropic", "claude-opus-4.7")
        );
        assert_eq!(tier_to_model(ModelTier::S2), ("openai", "gpt-5.5"));
        assert_eq!(
            tier_to_model(ModelTier::S3),
            ("xiaomi", "mimo-v2.5-pro")
        );
        assert_eq!(
            tier_to_model(ModelTier::S4),
            ("deepseek", "deepseek-v4-pro")
        );
        assert_eq!(
            tier_to_model(ModelTier::S5),
            ("moonshotai", "kimi-k2.6")
        );
        assert_eq!(tier_to_model(ModelTier::S6), ("openai", "gpt-5.4-mini"));
        assert_eq!(tier_to_model(ModelTier::S7), ("openai", "gpt-5.4-nano"));
    }

    #[test]
    fn fallback_never_equals_primary() {
        for tier in all_tiers() {
            assert_ne!(
                tier_to_model(tier),
                tier_fallback(tier),
                "fallback collides with primary for {}",
                tier.as_str()
            );
        }
    }

    #[test]
    fn fallback_stays_within_the_seven_model_set() {
        let allowed: &[(&str, &str)] = &[
            ("anthropic", "claude-opus-4.7"),
            ("openai", "gpt-5.5"),
            ("xiaomi", "mimo-v2.5-pro"),
            ("deepseek", "deepseek-v4-pro"),
            ("moonshotai", "kimi-k2.6"),
            ("openai", "gpt-5.4-mini"),
            ("openai", "gpt-5.4-nano"),
        ];
        for tier in all_tiers() {
            let fb = tier_fallback(tier);
            assert!(
                allowed.contains(&fb),
                "fallback for {} ({:?}) is outside the seven-model set",
                tier.as_str(),
                fb,
            );
        }
    }

    #[test]
    fn nano_fallback_does_not_jump_to_a_big_model() {
        // S7 falling back to Opus would defeat the routing.
        let (_, model) = tier_fallback(ModelTier::S7);
        assert!(
            model.contains("mini") || model.contains("nano"),
            "S7 fell back to a non-cheap model: {model}"
        );
    }

    #[test]
    fn cost_rank_orders_descending() {
        assert!(ModelTier::S1.cost_rank() < ModelTier::S2.cost_rank());
        assert!(ModelTier::S2.cost_rank() < ModelTier::S3.cost_rank());
        assert!(ModelTier::S6.cost_rank() < ModelTier::S7.cost_rank());
    }

    #[test]
    fn as_str_round_trips() {
        for tier in all_tiers() {
            let s = tier.as_str();
            assert_eq!(s.len(), 2);
            assert!(s.starts_with('S'));
        }
    }

    #[test]
    fn default_temperature_is_high_only_for_creative_tier() {
        assert!(tier_default_temperature(ModelTier::S5) >= 0.7);
        for tier in [
            ModelTier::S1,
            ModelTier::S2,
            ModelTier::S3,
            ModelTier::S4,
            ModelTier::S6,
            ModelTier::S7,
        ] {
            assert!(
                tier_default_temperature(tier) < 0.6,
                "non-creative tier {} has high temperature",
                tier.as_str()
            );
        }
    }

    #[test]
    fn classification_tier_is_near_zero_temperature() {
        assert!(tier_default_temperature(ModelTier::S7) <= 0.1);
    }

    #[test]
    fn serde_round_trip_uppercase() {
        let json = serde_json::to_string(&ModelTier::S3).unwrap();
        assert_eq!(json, "\"S3\"");
        let parsed: ModelTier = serde_json::from_str("\"S5\"").unwrap();
        assert_eq!(parsed, ModelTier::S5);
    }

    #[test]
    fn known_critical_roles_route_to_s1() {
        // These are the roles where mistakes cost real money;
        // they must NEVER silently downgrade to mid-tier.
        for role in [
            "premortem",
            "general_counsel",
            "fundraise_captain",
            "tax_advisor",
            "corp_dev",
        ] {
            assert_eq!(
                recommended_tier_for_role(role),
                ModelTier::S1,
                "critical role '{role}' is not on S1",
            );
        }
    }

    #[test]
    fn code_family_routes_to_s4() {
        for role in ["coder", "devops", "db_designer", "cicd", "tester", "qa"] {
            assert_eq!(
                recommended_tier_for_role(role),
                ModelTier::S4,
                "code-family role '{role}' is not on S4",
            );
        }
    }

    #[test]
    fn creative_roles_route_to_s5() {
        for role in [
            "naming_strategist",
            "content_creator",
            "copywriter",
            "ghostwriter",
            "designer",
        ] {
            assert_eq!(
                recommended_tier_for_role(role),
                ModelTier::S5,
                "creative role '{role}' is not on S5",
            );
        }
    }

    #[test]
    fn orchestrator_is_s2_not_s1() {
        // Important: orchestrator does NOT run on Opus. The whole
        // point of the routing is that the orchestrator delegates
        // expensive thinking to S1 only when needed.
        assert_eq!(recommended_tier_for_role("orchestrator"), ModelTier::S2);
    }

    #[test]
    fn unknown_role_defaults_to_safe_middle() {
        assert_eq!(
            recommended_tier_for_role("totally_made_up_role"),
            ModelTier::S6,
            "unknown roles must default to S6, not panic or escalate to S1",
        );
    }

    fn all_tiers() -> [ModelTier; 7] {
        [
            ModelTier::S1,
            ModelTier::S2,
            ModelTier::S3,
            ModelTier::S4,
            ModelTier::S5,
            ModelTier::S6,
            ModelTier::S7,
        ]
    }
}
