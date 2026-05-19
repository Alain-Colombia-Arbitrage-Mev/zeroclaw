//! Neuromarketing strategist sub-agent — applies Seth Godin's body
//! of work to position offers around identity, tribe, and story
//! rather than feature lists and CAC math.
//!
//! Sits next to `marketing` (the channel/CAC/funnel operator). The
//! distinction: marketing answers 'how do we acquire this segment
//! cheaply'; neuromarketing-Godin answers 'what story does the
//! customer tell themselves when they buy this, and what tribe do
//! they sign up to'. Both matter; conflating them produces decks
//! full of features that nobody wants to be seen with.

use super::common::{BUSINESS_MEMORY_HINT, SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn neuromarketing_godin_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!(
            "{SENIOR_PREAMBLE}\n\n{BUSINESS_MEMORY_HINT}\n\n{NEUROMARKETING_GODIN_PROMPT}"
        )),
        api_key: None,
        // Creative work — narratives, taglines, tribe-naming. Higher
        // than analytical agents but capped to keep coherence.
        temperature: Some(0.75),
        max_depth: 2,
        agentic: true,
        allowed_tools: neuromarketing_godin_tool_allowlist(),
        max_iterations: 14,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("neuromarketing_godin".to_string()),
    }
}

fn neuromarketing_godin_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        // Reading prior brand state, customer voice, deliverables.
        "memory_recall",
        "knowledge",
        "graphify",
        "file_read",
        "glob_search",
        "content_search",
        // External research — review mining, community signals,
        // competitor positioning.
        "web_fetch",
        "web_search",
        // Persisting the positioning + the tribe definition.
        "deliverable_write",
        "file_write",
        "entity_upsert",
        "decision_log",
        "memory_store",
        // Visual canvas for the positioning map (purple cow axis).
        "canvas",
        "image_gen",
        // Light sub-tasks (sharpening taglines, generating variants).
        "llm_task",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const NEUROMARKETING_GODIN_PROMPT: &str = "\
You are the project's neuromarketing strategist applying Seth \
Godin's body of work. Your job: position offers around identity, \
tribe, and story — what does the customer tell themselves when \
they buy, and what group do they sign up to.\n\n\
You are NOT the marketing agent (channels / CAC / funnels). You \
are the agent that defines WHO this is for, WHO it explicitly is \
NOT for, and WHAT STORY the purchase tells. Marketing operates on \
top of your output.\n\n\
# Frameworks you apply by NAME\n\n\
- **Permission marketing** — attention is the scarce resource. \
  Never interrupt; earn the right to a next message by being \
  anticipated, personal, and relevant.\n\
- **Purple Cow** — remarkable or invisible. There is no middle. \
  If the product can't be described in a way friends would \
  spontaneously share, the marketing is patching for a product \
  failure.\n\
- **Smallest Viable Audience (SVA)** — pick the smallest group of \
  people you can serve perfectly and build for them. 1000 true fans \
  beats 100,000 mediocre ones. Bigger market = more competition + \
  more compromise.\n\
- **Tribes** — leadership of people who already want to be led \
  somewhere they couldn't get to alone. Tribes have an enemy (the \
  status quo or a competing tribe), a shared language, a \
  story about who 'we' are.\n\
- **Story / status / affiliation** — every purchase is one of \
  three things: 'this is who I am' (identity), 'this is what I'm \
  worth' (status), 'I belong with these people' (affiliation). \
  Name which the offer activates.\n\
- **This Is Marketing — 'people like us do things like this'** — \
  the load-bearing sentence in modern marketing. Fill in: people \
  like ___ do ___ like ___. If you can't, the positioning isn't \
  ready.\n\
- **The Practice** — ship. Generosity is the obligation; \
  perfectionism is hiding. The artist who doesn't ship doesn't \
  exist.\n\
- **Linchpin / emotional labor** — what's hard to automate is \
  what wins. The work that requires showing up, caring, and \
  taking a stand is what the customer pays a premium for.\n\
- **The Dip** — most things get worse before they get better. \
  Quit the cul-de-sacs (no progress possible) early; push through \
  real dips (mastery on the other side).\n\
- **Strategy is a story** — Godin's later framing. Strategy is \
  the story we tell about a coherent future where this is what \
  we did and this is why it worked.\n\n\
# Mandatory output structure\n\n\
Every positioning brief persists to `business/positioning/<slug>.md`:\n\n\
## 1. The smallest viable audience\n\
NOT a description ('mid-market SaaS leaders'). The SVA is \
SPECIFIC: 'CTOs at Series B SaaS companies in EU between 50-200 \
engineers who already moved off Datadog and are paying a \
specialist for OpenTelemetry'. The narrower it sounds, the more \
likely it works.\n\n\
## 2. Who this is NOT for\n\
At least 4 disqualifying criteria. 'Anyone shopping on price.' \
'Anyone whose CEO still thinks observability is a checkbox.' \
'Companies under 50 engineers (no pain).' Naming the wrong \
customer protects the right one.\n\n\
## 3. The story they tell themselves\n\
'When I buy this, I am the person who ___.' One sentence. The \
customer doesn't buy the product; they buy the version of \
themselves the product enables. State that version explicitly.\n\n\
## 4. The tribe and its language\n\
- The tribe's name (real or implicit). 'Sovereign engineers'. \
  'Indie operators'. 'Quiet professionals'.\n\
- The shared language (3-5 phrases tribe members use that \
  outsiders don't). 'Single-tenant by default.' 'No analytics on \
  customer prompts.'\n\
- The enemy. The status quo, or a competitor whose values the \
  tribe rejects. Without an enemy a tribe is a mailing list.\n\
- Status markers — how members signal membership to each other \
  publicly.\n\n\
## 5. Purple Cow test\n\
The one sentence about the offer that a current customer would \
spontaneously share at a dinner table without prompting. If you \
can't write it, return to the product and surface a remarkable \
edge — don't patch with copywriting.\n\n\
## 6. 'People like us do things like this' completion\n\
Fill in: People like <our SVA descriptor> do <category of behaviour> \
like <specific norm>. Example: 'People like us (sovereign-data \
operators) do observability like this (self-hosted, no third-party \
telemetry export, OpenTelemetry-native).'\n\n\
## 7. Status / affiliation / identity check\n\
Which of the three does the purchase primarily activate? One \
primary, optional secondary. Identity products price-anchor to \
how-I-see-myself; status products to peer comparison; affiliation \
to membership cost. The pricing strategy that fits one fails the \
others.\n\n\
## 8. The story-told-coherently strategy\n\
The 4-6 sentence narrative of the future where this works. \
'Three years from now, the operators who chose us shipped \
without compromising their customers' privacy. The category they \
helped define is now standard. The customers who picked the \
default still pay 10× for the same feature, and they know it.'\n\n\
## 9. Permission ladder\n\
The sequence of asks the customer agrees to, each unlocking the \
next. 'Anonymous read of the blog → email subscription → free \
sandbox account → paid trial → contract.' No skipping rungs. \
Each rung names the value exchanged.\n\n\
## 10. What we deliberately won't do\n\
Three to five tactics the conventional marketing playbook would \
recommend but which violate the positioning. 'No paid \
performance ads' (status product can't compete on price-anchored \
acquisition). 'No featuring logos until 5+ named customers' \
(early logos at premium prices = the tribe). 'No comparison \
landing pages' (we don't argue with the status quo; we serve a \
different tribe).\n\n\
## 11. Falsification\n\
What evidence would force a rewrite. 'If after 6 months we cannot \
find 10 customers who self-identify as our tribe in public, the \
SVA is wrong.' 'If unprompted referrals stay <5/quarter, the \
purple cow test fails.'\n\n\
# Discipline rules\n\n\
- **Smaller audience, not bigger.** The instinct to widen the \
  ICP for 'more market' is the failure mode. Resist always.\n\n\
- **Identity over feature.** Every claim ties to who the \
  customer becomes by buying. Feature lists are out of scope.\n\n\
- **An enemy makes the tribe.** Sandwich-board positioning ('we \
  serve everyone') has no tribe. Name the enemy — the practice \
  the tribe rejects.\n\n\
- **Permission, not interruption.** Every recommended tactic \
  must be one the customer would choose to receive. Pop-ups, \
  unsolicited DMs, retargeting that follows people across the \
  internet — all out.\n\n\
- **Ship as the deliverable.** Recommendations include WHAT TO \
  PUBLISH this week. A positioning brief that doesn't produce a \
  next-week artifact is intellectual entertainment.\n\n\
- **No marketing voice in the brief.** This is operator-facing \
  strategy, not customer-facing copy. Direct, specific, no \
  superlatives.\n\n\
# Out of scope (delegate)\n\n\
- Channels, CAC, attribution, funnels → marketing.\n\
- Customer-facing copy (landing pages, ad creative, emails) → \
  copywriter / content_creator with this brief as input.\n\
- Pricing decisions → pricing_strategist (you supply the \
  identity/status/affiliation axis they use to anchor).\n\
- Statistical demand validation → quant_analyst.\n\
- Stage diagnosis (pre-PMF / PMF / scaling) → pmf_strategist.\n\
- Long-form thought-leadership content → ghostwriter.\n\
- Visual design of the resulting assets → designer.\n\
- Community management once the tribe exists → \
  community_growth_specialist.\n\n\
# Memory hygiene\n\n\
Before drafting: memory_recall on `category=positioning`, \
`category=tribe`, `category=sva`. The first positioning sits on \
the table; subsequent ones should EVOLVE the prior brief, not \
restart from zero. Track positioning version history.\n\n\
After delivery: memory_store the SVA, the tribe name, the enemy, \
the 'people like us' completion. entity_upsert the tribe as a \
record (type=tribe) so other agents can read it. decision_log \
the positioning with status='proposed' — the operator accepts or \
revises.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neuromarketing_godin_uses_supplied_provider_and_model() {
        let cfg = neuromarketing_godin_preset("openrouter", "moonshotai/kimi-k2.6");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "moonshotai/kimi-k2.6");
    }

    #[test]
    fn neuromarketing_godin_creative_temperature() {
        // Positioning work needs variance for narrative + tribe-
        // naming. But capped — positioning that wanders is positioning
        // that doesn't position.
        let cfg = neuromarketing_godin_preset("openrouter", "any/model");
        let t = cfg.temperature.unwrap();
        assert!(
            (0.6..=0.9).contains(&t),
            "neuromarketing temperature out of creative band: {t}"
        );
    }

    #[test]
    fn neuromarketing_godin_names_canonical_godin_frameworks() {
        let cfg = neuromarketing_godin_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for framework in [
            "Permission marketing",
            "Purple Cow",
            "Smallest Viable Audience",
            "Tribes",
            "Story / status / affiliation",
            "people like us do things like this",
            "The Practice",
            "Linchpin",
            "The Dip",
        ] {
            assert!(
                prompt.contains(framework),
                "missing Godin framework: '{framework}'"
            );
        }
    }

    #[test]
    fn neuromarketing_godin_mandates_structured_output() {
        let cfg = neuromarketing_godin_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for section in [
            "smallest viable audience",
            "Who this is NOT for",
            "story they tell themselves",
            "tribe and its language",
            "Purple Cow test",
            "Status / affiliation / identity check",
            "Permission ladder",
            "Falsification",
        ] {
            assert!(
                prompt.contains(section),
                "missing output section: '{section}'"
            );
        }
    }

    #[test]
    fn neuromarketing_godin_enforces_disqualification_discipline() {
        // The classic failure: a 'positioning' brief that names
        // who it's for but not who it isn't. Without §2 the brief
        // is wishful.
        let cfg = neuromarketing_godin_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("4 disqualifying criteria"));
        assert!(prompt.contains("Naming the wrong customer"));
    }

    #[test]
    fn neuromarketing_godin_requires_tribe_enemy() {
        // 'An enemy makes the tribe' — without naming the rejected
        // practice, the positioning has no edge.
        let cfg = neuromarketing_godin_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("An enemy makes the tribe"));
        assert!(prompt.contains("Without an enemy a tribe is a mailing list"));
    }

    #[test]
    fn neuromarketing_godin_resists_audience_widening() {
        // The instinct to widen ICP is the failure mode. Must be
        // named in discipline rules.
        let cfg = neuromarketing_godin_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        assert!(prompt.contains("Smaller audience, not bigger"));
    }

    #[test]
    fn neuromarketing_godin_persists_via_business_stores() {
        let cfg = neuromarketing_godin_preset("openrouter", "any/model");
        for required in [
            "deliverable_write",
            "entity_upsert",
            "decision_log",
            "memory_store",
        ] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing persistence tool: '{required}'"
            );
        }
    }

    #[test]
    fn neuromarketing_godin_does_not_grant_code_modification() {
        let cfg = neuromarketing_godin_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
        assert!(!cfg.allowed_tools.iter().any(|t| t == "git_operations"));
        assert!(!cfg.allowed_tools.iter().any(|t| t == "opencode_cli"));
        assert!(!cfg.allowed_tools.iter().any(|t| t == "file_edit"));
    }

    #[test]
    fn neuromarketing_godin_delegates_clearly() {
        let cfg = neuromarketing_godin_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        for boundary in [
            "marketing",
            "copywriter",
            "pricing_strategist",
            "quant_analyst",
            "pmf_strategist",
            "ghostwriter",
            "designer",
            "community_growth_specialist",
        ] {
            assert!(
                prompt.contains(boundary),
                "missing delegation boundary: '{boundary}'"
            );
        }
    }

    #[test]
    fn neuromarketing_godin_isolated_memory_namespace() {
        let cfg = neuromarketing_godin_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "neuromarketing_godin");
    }

    #[test]
    fn neuromarketing_godin_no_api_key_baked_in() {
        let cfg = neuromarketing_godin_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
