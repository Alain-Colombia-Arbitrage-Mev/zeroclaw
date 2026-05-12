use crate::cost::CostTracker;
use crate::cost::types::{BudgetCheck, TokenUsage as CostTokenUsage};
use std::sync::Arc;
use zeroclaw_api::provider::ChatMessage;
use zeroclaw_config::schema::{CostEnforcementConfig, ModelPricing};

// ── Cost tracking via task-local ──

/// Approximation: 1 token ≈ 4 chars (tiktoken-style heuristic, no extra deps).
const CHARS_PER_TOKEN: usize = 4;

/// Conservative ceiling for an LLM response in a single tool-loop turn.
/// Used for the pre-flight cost estimate before we know the real output size.
const DEFAULT_OUTPUT_TOKEN_ESTIMATE: u64 = 1024;

/// Context for cost tracking within the tool call loop.
/// Scoped via `tokio::task_local!` at call sites (channels, gateway).
#[derive(Clone)]
pub struct ToolLoopCostTrackingContext {
    pub tracker: Arc<CostTracker>,
    pub prices: Arc<std::collections::HashMap<String, ModelPricing>>,
    pub enforcement: CostEnforcementConfig,
}

impl ToolLoopCostTrackingContext {
    pub fn new(
        tracker: Arc<CostTracker>,
        prices: Arc<std::collections::HashMap<String, ModelPricing>>,
    ) -> Self {
        Self {
            tracker,
            prices,
            enforcement: CostEnforcementConfig::default(),
        }
    }

    /// Builder: attach the cost enforcement policy.
    pub fn with_enforcement(mut self, enforcement: CostEnforcementConfig) -> Self {
        self.enforcement = enforcement;
        self
    }
}

tokio::task_local! {
    pub static TOOL_LOOP_COST_TRACKING_CONTEXT: Option<ToolLoopCostTrackingContext>;
}

/// 3-tier model pricing lookup: exact id → `provider/model` → suffix after last `/`.
fn lookup_pricing<'a>(
    prices: &'a std::collections::HashMap<String, ModelPricing>,
    provider_name: &str,
    model: &str,
) -> Option<&'a ModelPricing> {
    prices
        .get(model)
        .or_else(|| prices.get(&format!("{provider_name}/{model}")))
        .or_else(|| {
            model
                .rsplit_once('/')
                .and_then(|(_, suffix)| prices.get(suffix))
        })
}

/// Record token usage from an LLM response via the task-local cost tracker.
/// Returns `(total_tokens, cost_usd)` on success, `None` when not scoped or no usage.
pub fn record_tool_loop_cost_usage(
    provider_name: &str,
    model: &str,
    usage: &zeroclaw_providers::traits::TokenUsage,
) -> Option<(u64, f64)> {
    let input_tokens = usage.input_tokens.unwrap_or(0);
    let output_tokens = usage.output_tokens.unwrap_or(0);
    let total_tokens = input_tokens.saturating_add(output_tokens);
    if total_tokens == 0 {
        return None;
    }

    let ctx = TOOL_LOOP_COST_TRACKING_CONTEXT
        .try_with(Clone::clone)
        .ok()
        .flatten()?;
    let pricing = lookup_pricing(&ctx.prices, provider_name, model);
    let cost_usage = CostTokenUsage::new(
        model,
        input_tokens,
        output_tokens,
        pricing.map_or(0.0, |entry| entry.input),
        pricing.map_or(0.0, |entry| entry.output),
    );

    if pricing.is_none() {
        tracing::debug!(
            provider = provider_name,
            model,
            "Cost tracking recorded token usage with zero pricing (no pricing entry found)"
        );
    }

    if let Err(error) = ctx.tracker.record_usage(cost_usage.clone()) {
        tracing::warn!(
            provider = provider_name,
            model,
            "Failed to record cost tracking usage: {error}"
        );
    }

    Some((cost_usage.total_tokens, cost_usage.cost_usd))
}

/// Check budget post-hoc (no estimate). Returns `None` when no cost tracking
/// context is scoped (tests, delegate, CLI without cost config).
pub fn check_tool_loop_budget() -> Option<BudgetCheck> {
    TOOL_LOOP_COST_TRACKING_CONTEXT
        .try_with(Clone::clone)
        .ok()
        .flatten()
        .map(|ctx| {
            ctx.tracker
                .check_budget(0.0)
                .unwrap_or(BudgetCheck::Allowed)
        })
}

/// Estimated cost of an upcoming LLM call, in USD. Uses a chars/4 heuristic for
/// input tokens and a fixed [`DEFAULT_OUTPUT_TOKEN_ESTIMATE`] ceiling for output.
/// Returns `0.0` when no pricing entry matches the model.
pub fn estimate_call_cost_usd(
    prices: &std::collections::HashMap<String, ModelPricing>,
    provider_name: &str,
    model: &str,
    messages: &[ChatMessage],
) -> f64 {
    let pricing = match lookup_pricing(prices, provider_name, model) {
        Some(p) => p,
        None => return 0.0,
    };

    let input_chars: usize = messages
        .iter()
        .map(|m| m.role.len() + m.content.len())
        .sum();
    let est_input_tokens = (input_chars / CHARS_PER_TOKEN) as u64;

    let input_cost = (est_input_tokens as f64 / 1_000_000.0) * pricing.input;
    let output_cost = (DEFAULT_OUTPUT_TOKEN_ESTIMATE as f64 / 1_000_000.0) * pricing.output;
    input_cost + output_cost
}

/// Pre-flight budget check before an LLM call. Estimates the cost from the
/// outgoing messages and consults the tracker. Returns `None` when no cost
/// tracking context is scoped (tests, delegate, CLI without cost config).
pub fn preflight_tool_loop_budget(
    provider_name: &str,
    model: &str,
    messages: &[ChatMessage],
) -> Option<BudgetCheck> {
    let ctx = TOOL_LOOP_COST_TRACKING_CONTEXT
        .try_with(Clone::clone)
        .ok()
        .flatten()?;

    let estimated_cost = estimate_call_cost_usd(&ctx.prices, provider_name, model, messages);
    ctx.tracker.check_budget(estimated_cost).ok()
}

/// Cost enforcement mode currently scoped (if any). Lets callers branch on
/// `warn` / `block` / `route_down` without re-reading the global config.
pub fn current_enforcement_mode() -> Option<String> {
    TOOL_LOOP_COST_TRACKING_CONTEXT
        .try_with(|ctx| ctx.as_ref().map(|c| c.enforcement.mode.clone()))
        .ok()
        .flatten()
}
