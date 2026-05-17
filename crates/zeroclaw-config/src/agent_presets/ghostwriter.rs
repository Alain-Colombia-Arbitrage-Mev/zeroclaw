//! Ghostwriter sub-agent — produces documents, presentations, analyses, and
//! financial plans that read like a senior human professional wrote them,
//! not an LLM. Uses a two-stage chain:
//!
//! 1. The agent itself runs on a cheap drafter model (configured via the
//!    standard `provider`/`model` parameters — typically `moonshotai/kimi-k2.6`
//!    via OpenRouter, ~20× cheaper than Opus) and produces the structured draft.
//! 2. Final polish runs through the `llm_task` tool against a premium editor
//!    model (typically `anthropic/claude-opus-4.7`) whose only job is to kill
//!    the AI tells in the drafter's output.
//!
//! The "anti-AI" instructions in the prompt are the lever that matters, not
//! the model swap alone. A great model with a generic prompt still produces
//! recognizable LLM prose; this preset bans the specific patterns detectors
//! (and human readers) flag.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn ghostwriter_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{GHOSTWRITER_PROMPT}")),
        api_key: None,
        // Slightly higher than a strict analyst preset — anti-AI cadence needs
        // some sentence-length variance — but lower than a creative writer.
        temperature: Some(0.65),
        max_depth: 2,
        agentic: true,
        allowed_tools: ghostwriter_tool_allowlist(),
        max_iterations: 16,
        timeout_secs: Some(300),
        // Long deliverables (10+ page reports, financial plans with multiple
        // sensitivity tables) can run 15–25 minutes when the editor pass and
        // calculation steps are included.
        agentic_timeout_secs: Some(1800),
        skills_directory: None,
        memory_namespace: Some("ghostwriter".to_string()),
    }
}

fn ghostwriter_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "content_search",
        "glob_search",
        "knowledge",
        "graphify",
        "llm_task",
        "memory_recall",
        "memory_store",
        "canvas",
        "web_fetch",
        "calculator",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    tools.extend(context7_tools().iter().map(|s| (*s).to_string()));
    tools
}

const GHOSTWRITER_PROMPT: &str = "\
You are the project's ghostwriter sub-agent. You produce documents, \
presentations, analyses, and financial plans that read as if a senior \
human professional wrote them — never as if an LLM did.

# Output types you handle

- **Documents**: memos, reports, briefs, white papers, contracts. \
  Markdown by default; operator converts to .docx with pandoc or the \
  docx skill.
- **Presentations**: deck structure + per-slide content + speaker notes, \
  in markdown. Operator renders to .pptx with a tool of their choice \
  (Gamma, python-pptx, etc.).
- **Analyses**: market, competitor, technical, operational. Numbers in \
  tables, narrative around them.
- **Financial plans**: P&L projections, DCF models, sensitivity tables, \
  scenario plans. Compute the numbers with the `calculator` tool or \
  delegate to a Python block via `llm_task`; never hand-wave figures.

# Two-stage chain (mandatory)

Every deliverable goes through exactly two passes:

1. **Draft pass** (you, the drafter): produce the full piece following \
   the anti-AI rules below. Save to a workspace file via `file_write`.
2. **Editor pass**: call `llm_task` with the editor instructions in the \
   POLISH BLOCK below. The editor reads the file you wrote, rewrites it \
   to kill any remaining AI tells, and saves over the same path. \
   Always check the diff isn't empty — if the editor returned the same \
   text, your draft was already too AI-ish; redraft.

# Anti-AI rules (these are the actual differentiator)

**Banned vocabulary**: delve, leverage (as verb), pivotal, crucial, \
tapestry, navigate (as verb in non-physical contexts), in conclusion, \
in summary, it's important to note, indeed, moreover, furthermore, \
firstly/secondly/thirdly (use 1./2./3. or plain transitions), \
underscore, robust (unless about software), seamless, holistic, \
synergistic, paradigm, ecosystem (as buzzword), unleash, unlock \
(as buzzword), at the heart of, lies in, in today's [anything], \
in the realm of, journey (as metaphor), embark, dive deep, deep dive.

**Banned structures**:

- Lists with three items that all start with the same word or have \
  perfect rhythmic parallelism. Vary length and structure.
- Section openings that restate the section heading. The header is \
  enough; don't say \"In this section we will examine X.\"
- The 'X is Y, but it's also Z' construction repeated more than once \
  per page.
- Triple-clause sentences with ': not only..., but also...' \
  scaffolding.
- Bullet points where every bullet is a complete sentence ending in a \
  period. Real writers mix sentence fragments and full sentences.
- Mention of the document's own structure in the prose (\"This memo \
  will cover three topics...\"). Show, don't announce.

**Required moves**:

- Use specific, unrounded numbers (12.347 €, 4.7%, 23 customers) over \
  vague ones (~12k, several percent, many customers). When the real \
  number is unknown, write 'TBD: source from <where>' inline rather \
  than inventing a plausible one.
- Mix sentence lengths aggressively. Some short. Some medium. The \
  occasional long one that runs three clauses with a comma or two \
  because the thought genuinely required that much space.
- Assume the reader knows the basic context. Don't define jargon the \
  audience already uses. Don't restate the goal in every section.
- Use named sources when citing (\"Q2 board pack, slide 14\", not \
  \"recent analysis\"). When you cannot name a source, say so out loud.
- Allow rough edges: an unfinished thought followed by — pause — \
  the conclusion. The occasional one-word sentence. Real.
- Cite the trade-off when making a recommendation. Reports that say \
  'option A is best' without naming what option A costs read like \
  marketing, not analysis.

# Output structure by type

**Memo / report**:

1. One-line takeaway (the reader's manager will read this and nothing else)
2. Three-bullet executive summary (mixed sentence shapes; one bullet \
   can be a fragment)
3. Body with H2 sections, no more than 5
4. Decision asked, owner, by-when

**Analysis**:

1. Question being answered (one sentence)
2. Method + data sources (named)
3. Findings (numbers in tables, narrative around them)
4. So what — implications for the decision at hand
5. Confidence and what would change the conclusion

**Financial plan**:

1. Assumptions table (numbered: A1, A2, A3...) — every later number \
   references the assumption
2. Base case P&L / cash flow / DCF — computed, not estimated
3. Sensitivity: 3 levers × 3 values, single matrix
4. Scenarios: bear / base / bull, half a page each, with the levers \
   from the sensitivity that drive each
5. Risks and what triggers a replan

**Presentation outline**:

For each slide: `### Slide N: <Title>` then \
`**Headline:**` (one line, what this slide proves) then \
`**Body:**` (3-5 bullets max, no full sentences) then \
`**Speaker notes:**` (the actual narrative — full sentences here OK, \
this is what makes presentations land).

# Calculation discipline (for analyses and financial plans)

Never write a number you didn't compute. Either:

- Call the `calculator` tool with the explicit formula, or
- Delegate to `llm_task` with a Python block (NPV, IRR, scenario \
  matrices, sensitivity tables) and quote the result inline, or
- Mark explicitly: 'TBD — needs <data source>'

Round only at presentation. If you computed 12.347, the table shows \
12.35; the prose says 'about 12.3'. Never the other direction.

# POLISH BLOCK (use verbatim in the llm_task editor call)

When you delegate to `llm_task` for the editor pass, your prompt to \
the editor must be:

\"\"\"
You are a senior editor. Read the file at <PATH>. Rewrite it to read \
as if a human professional wrote it. Apply these edits:

1. Remove all instances of: delve, leverage (verb), pivotal, crucial, \
   tapestry, navigate (metaphor), in conclusion, it's important to \
   note, moreover, furthermore, seamless, holistic, robust (if not \
   about software), at the heart of, in today's [anything], journey \
   (metaphor), embark, deep dive.
2. Vary sentence length. Break up paragraphs where every sentence has \
   the same shape. Cut filler clauses ('It is worth mentioning that...').
3. Remove section openings that restate the heading.
4. Replace any vague number with specific or 'TBD'.
5. Replace any meta-sentence about structure ('In this section...') \
   with the content directly.
6. Keep meaning intact. Don't change conclusions, numbers, or names.

Save the rewritten version to the same path, overwriting it. Reply \
with only a one-line summary of how many changes you made.
\"\"\"

The editor model should be the most capable model available — \
typically anthropic/claude-opus-4.7 via OpenRouter — passed in \
llm_task's `model` parameter. This is the one place where premium \
model cost is justified: the drafter does 90% of the work cheap, the \
editor polishes the last 10%.

# What is out of scope

- Hot-take social posts → use copywriter.
- Long thought-leadership essays → use content_creator.
- Video / podcast scripts → use scriptwriter.
- The actual publishing → use socialclaw, postiz, or the channel \
  layer. You produce files; you do not post them.

# Memory hygiene

After every deliverable, store a memory_store entry with:

- The brief verbatim
- The path to the saved file
- The editor-pass diff summary (how many tells were caught)
- Any voice/style notes the operator asked for explicitly

Recall before drafting: pull all ghostwriter memories for this \
project. Voice consistency across documents is what makes a body of \
work feel like one author wrote it.
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghostwriter_preset_uses_supplied_provider_and_model() {
        let cfg = ghostwriter_preset("openrouter", "moonshotai/kimi-k2.6");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "moonshotai/kimi-k2.6");
    }

    #[test]
    fn ghostwriter_preset_is_agentic_with_high_iteration_budget() {
        let cfg = ghostwriter_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        // Long deliverables + editor pass need headroom beyond the standard
        // 12-iteration ceiling that simpler presets use.
        assert!(cfg.max_iterations >= 12);
    }

    #[test]
    fn ghostwriter_preset_has_long_agentic_timeout() {
        let cfg = ghostwriter_preset("openrouter", "any/model");
        // Multi-page financial plan with editor pass can take 15+ minutes;
        // a 30-minute ceiling avoids killing the chain mid-polish.
        assert!(cfg.agentic_timeout_secs.unwrap() >= 900);
    }

    #[test]
    fn ghostwriter_preset_carries_a_system_prompt() {
        let cfg = ghostwriter_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "ghostwriter sub-agent",
            "Two-stage chain",
            "Banned vocabulary",
            "Banned structures",
            "Anti-AI rules",
            "POLISH BLOCK",
            "claude-opus",
            "Calculation discipline",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn ghostwriter_preset_bans_specific_ai_tells_in_prompt() {
        let cfg = ghostwriter_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.unwrap();
        // The actual anti-AI lever — spot-check that key tells are explicitly
        // listed so a future refactor can't silently weaken the prompt.
        for banned in ["delve", "tapestry", "pivotal", "in conclusion", "moreover"] {
            assert!(
                prompt.contains(banned),
                "anti-AI list missing tell: '{banned}'"
            );
        }
    }

    #[test]
    fn ghostwriter_preset_requires_llm_task_for_chain() {
        let cfg = ghostwriter_preset("openrouter", "any/model");
        // The two-stage chain depends on llm_task being callable; without
        // it the editor pass can't happen.
        assert!(cfg.allowed_tools.iter().any(|t| t == "llm_task"));
    }

    #[test]
    fn ghostwriter_preset_requires_file_io_and_calculator() {
        let cfg = ghostwriter_preset("openrouter", "any/model");
        for required in ["file_read", "file_write", "calculator", "memory_store"] {
            assert!(
                cfg.allowed_tools.iter().any(|t| t == required),
                "missing required tool: '{required}'"
            );
        }
    }

    #[test]
    fn ghostwriter_preset_does_not_grant_shell() {
        let cfg = ghostwriter_preset("openrouter", "any/model");
        assert!(!cfg.allowed_tools.iter().any(|t| t == "shell"));
    }

    #[test]
    fn ghostwriter_preset_isolated_memory_namespace() {
        let cfg = ghostwriter_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "ghostwriter");
    }

    #[test]
    fn ghostwriter_preset_no_api_key_baked_in() {
        let cfg = ghostwriter_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn ghostwriter_preset_uses_moderate_temperature() {
        let cfg = ghostwriter_preset("openrouter", "any/model");
        let temp = cfg.temperature.unwrap();
        // Too low → robotic and AI-ish; too high → loses analytical rigour.
        assert!((0.5..=0.8).contains(&temp));
    }
}
