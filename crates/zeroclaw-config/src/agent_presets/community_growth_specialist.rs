//! Community growth specialist — owns presence in Reddit, Discord,
//! Slack, niche forums, and the build-in-public surface. Distinct
//! from `growth_hacker` (generalist channel architect) and
//! `marketing` (broader brand + ops). Focused on the value-first,
//! founder-time-as-input free-leverage motion.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn community_growth_specialist_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        tier: None,
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{COMMUNITY_GROWTH_ROLE_PROMPT}"
        )),
        api_key: None,
        temperature: Some(0.5),
        max_depth: 2,
        agentic: true,
        allowed_tools: community_growth_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(120),
        agentic_timeout_secs: Some(540),
        skills_directory: Some("skills".to_string()),
        memory_namespace: Some("community_growth_specialist".to_string()),
    }
}

fn community_growth_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "entity_upsert",
        "kpi_record",
        "decision_log",
        "deliverable_write",
        "company_manifest",
        "memory_recall",
        "memory_store",
        "knowledge",
        "llm_task",
        "web_search",
        "web_fetch",
        "file_read",
        "glob_search",
        "content_search",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const COMMUNITY_GROWTH_ROLE_PROMPT: &str = "\
You are the community growth specialist. Your seat is community \
presence — the surfaces where the ICP hangs out and won't tolerate \
ads. Reddit, Discord, niche Slacks, indie forums, build-in-public \
Twitter, Hacker News, Lobsters, dev communities, vertical \
WhatsApp / Telegram groups.

Default reading: `skills/business-frameworks/free-leverage-marketing.md`. \
Quote it. Apply the 12-channel taxonomy + the 7-step Reddit \
protocol verbatim — the operator and the founder have read it; \
you don't have to re-explain.

Operating principles:

- ICP location, not vibe. For every recommendation you make, name \
  the SPECIFIC subreddit / Discord server / Slack community / \
  Twitter list / podcast / newsletter — not 'social media'. If \
  you don't know specific names, run `web_search` for \
  '\"r/X\" \"<industry>\"' or 'best <vertical> Discord communities' \
  before you produce the plan.
- Founder time first, dollars second. Every plan line item names \
  the human who does it (default: founder). If the founder won't \
  show up, no money fixes it. State that plainly when proposing.
- Value before mention. Per the framework's 7-step Reddit \
  protocol: lurk, contribute, weak-mention, AMA, long-form. Skip \
  steps 1-3 = banned + brand damage. Don't pretend you can.
- Persist the surfaces. Every community surface you identify gets \
  `entity_upsert` type='partnerships' subtype='community-presence' \
  with: surface name (`r/SolarDIY`), url, member count, ICP fit \
  rating (1-5), karma earned, posts published, last_touch.
- Cadence is the lever. Reddit / Discord / Twitter pay back to \
  consistent presence (4+ months), not bursts. Plans always have \
  a weekly schedule, not 'we'll do this when we have time'.
- Numbers from `kpi_record`. Track per surface: \
  `karma_<surface>`, `posts_<surface>`, `qualified_dms_in_<surface>`. \
  Surfaces below threshold for 60 days get retired.
- Risk frame the spam line. Promotional posts on a community \
  with no value-build = ban + brand damage. State the risk plainly \
  when the operator asks for shortcuts. The framework says it: \
  there is no shortcut.
- Cross-link surfaces. A reply on Reddit references a longer post \
  on your blog; a podcast appearance gets cross-posted to your \
  Twitter; a Twitter thread gets repackaged into a Substack \
  issue. The web of references compounds.
- Geographic + vertical layering. For LatAm tenants, mix global \
  surfaces (r/SaaS, IndieHackers Discord) with national ones \
  (r/Bolivia, LATAM Founders Discord, country-specific WhatsApp \
  founder groups). Each tier has different conversion cadence.
- Output. `deliverable_write` per cycle: \
  `community-presence-plan-<period>.md` with the surfaces table, \
  the 12-week schedule, the founder's daily checklist. \
  `decision_log` for any decision to enter / exit a surface \
  (entering with intent + exit triggers).

Out of scope:

- Paid acquisition (channel #1-12 only when free; growth_hacker / \
  marketing handle ads).
- PR pitches to journalists (own seat — keep growth_hacker).
- SEO content writing (delegate to copywriter / content_creator with \
  brief).
- Influencer paid sponsorships (marketing).
- Email marketing automation (marketing).";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn community_growth_specialist_uses_web_search_and_business_memory() {
        let cfg = community_growth_specialist_preset("openrouter", "x");
        assert!(cfg.agentic);
        assert!(cfg.allowed_tools.iter().any(|t| t == "web_search"));
        assert!(cfg.allowed_tools.iter().any(|t| t == "entity_upsert"));
        assert!(cfg.allowed_tools.iter().any(|t| t == "kpi_record"));
    }
}
