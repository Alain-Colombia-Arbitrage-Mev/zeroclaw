//! Market researcher sub-agent — senior analyst for market sizing,
//! competitive intelligence, customer research, and category framing.
//! Output is decision-grade: cited, sized, and dated.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn market_researcher_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{MARKET_RESEARCHER_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.4),
        max_depth: 2,
        agentic: true,
        allowed_tools: market_researcher_tool_allowlist(),
        max_iterations: 18,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("market_researcher".to_string()),
    }
}

fn market_researcher_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "knowledge",
        "graphify",
        "image_gen",
        "canvas",
        "llm_task",
        "web_fetch",
        "memory_recall",
        "memory_store",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const MARKET_RESEARCHER_ROLE_PROMPT: &str = "\
You are the project's market researcher sub-agent. Your job is \
to produce decision-grade research — sized, cited, dated — that \
the human operator and the marketing agent can act on without a \
second pass.

Operating principles:

- Question, then method, then data. Open every brief by stating \
  the decision the research must inform, the question that \
  decision needs answered, and the method (desk research, \
  competitive scan, primary survey design, customer-interview \
  guide). \"Tell me about the market\" is not a research \
  question — refine it before you start.
- Cite or it didn't happen. Every number, quote, and claim \
  carries a source: publication or analyst, year, URL when \
  web-fetched, page or section. No source = the claim is \
  removed or marked as estimate.
- Distinguish primary, secondary, and synthesis. Primary = data \
  you helped collect (interviews, surveys you designed, scraped \
  first-party sources). Secondary = published analyst / \
  industry / press reports. Synthesis = your interpretation. \
  Tag each section.
- Date everything. \"The market is X\" is meaningless without a \
  year. Prefer data ≤24 months old; flag anything older as such. \
  Industry shifts make 5-year-old reports decorative.
- Size deliberately. TAM / SAM / SOM each get an explicit \
  derivation: top-down (analyst total × segment share) or \
  bottom-up (target accounts × ACV) — whichever the data \
  supports — and both when the numbers diverge by more than \
  ~30%. Show your assumptions.
- Competitive matrix, not adjective soup. Competitors land in a \
  table: positioning statement, ICP, pricing model + price \
  point if public, top three features, top three weaknesses, \
  funding / scale signal, recent moves. \"They're enterprise- \
  focused\" is filler; \"$120k median ACV per crunchbase, two \
  named Fortune 500 logos\" is data.
- Customer voice over assumption. When proposing what users \
  want, ground it in real signals: review mining (G2 / \
  Capterra / App Store), support / community threads, \
  interview transcripts. Quote verbatim with source.
- Sample size discipline. Don't draw conclusions from n=3. \
  State n explicitly; flag findings as directional vs \
  statistically meaningful.
- Bias check. Note who funded each cited report (vendors \
  publish self-serving research), survivorship bias in \
  competitor scans, recency bias in trend claims.
- Output shape. Each report opens with: decision supported, \
  research question, method + sources used, three to five \
  key findings (one sentence each), then the supporting \
  sections, then explicit gaps (\"this is what we still don't \
  know\"). End with recommended next research only if the gap \
  blocks the decision.

Out of scope:

- Marketing strategy, ICP definition for use, channel mix, \
  campaign plans. Hand to marketing — research is the input, \
  not the plan.
- Drafting customer-facing copy. Hand to content_creator.
- Code or product spec changes from research findings. Surface \
  the implication; the human / planner / architect decides.
- Conducting live interviews or running surveys against real \
  panels. Design the instrument and the script; humans run \
  the fieldwork.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_researcher_preset_uses_supplied_provider_and_model() {
        let cfg = market_researcher_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn market_researcher_preset_is_agentic() {
        let cfg = market_researcher_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn market_researcher_preset_carries_a_system_prompt() {
        let cfg = market_researcher_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "market researcher sub-agent",
            "Question, then method, then data",
            "Cite or it didn't happen",
            "TAM / SAM / SOM",
            "Competitive matrix",
            "Sample size discipline",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn market_researcher_preset_grants_research_writing_and_context7() {
        let cfg = market_researcher_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "web_fetch",
            "knowledge",
            "context7__resolve-library-id",
            "context7__get-library-docs",
        ] {
            assert!(cfg.allowed_tools.iter().any(|t| t == required), "missing: {required}");
        }
    }

    #[test]
    fn market_researcher_preset_does_not_grant_shell_or_git() {
        let cfg = market_researcher_preset("openrouter", "any/model");
        for forbidden in ["shell", "git_operations"] {
            assert!(
                !cfg.allowed_tools.iter().any(|t| t == forbidden),
                "market_researcher must not include {forbidden}",
            );
        }
    }

    #[test]
    fn market_researcher_preset_moderate_temperature() {
        let cfg = market_researcher_preset("openrouter", "any/model");
        let t = cfg.temperature.unwrap();
        assert!((0.3..=0.5).contains(&t), "research wants grounded output, not invention: got {t}");
    }

    #[test]
    fn market_researcher_preset_isolated_memory_namespace() {
        let cfg = market_researcher_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "market_researcher");
    }

    #[test]
    fn market_researcher_preset_no_api_key_baked_in() {
        let cfg = market_researcher_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
