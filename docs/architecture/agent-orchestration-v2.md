# Agent Orchestration v2 — design doc

Audience: maintainers + reviewers who'll sign off on the changes before
implementation. This is a roadmap, not landed work.

## Goal

Today the orchestrator is a single agent that calls `delegate` against
a roster of sub-agents (CEO, CFO, NGO architect, …). Each sub-agent
runs in isolation with its own `memory_namespace` and never sees the
work of its siblings until the orchestrator collates the outputs.

The user-facing gap: advisors give parallel monologues, not a
deliberation. "Crear empresa" produces N independent docs, none of
which references the others. The CFO's runway model can't push back on
the CTO's hire plan. The strategic-bets section in the manifest never
gets challenged by a red-teamer before it ships.

This doc proposes three increments — F2 (shared memory), F3 (support
agents), F4 (debate orchestration) — that incrementally close that
gap without rebuilding the runtime.

---

## F2 — Shared memory between sub-agents

### Problem

`crates/zeroclaw-config/src/agent_presets/*.rs` sets one
`memory_namespace` per role: `cfo_advisor`, `ceo_advisor`,
`ngo_architect`, … `crates/zeroclaw-memory/src/namespaced.rs` wraps the
backend so every recall is scoped to that namespace. Result: when the
CFO writes a runway-table to memory, the CEO advisor never finds it on
their next turn — even within the same "crear empresa" run.

### Proposal

Introduce a **second** memory namespace per agent, on top of the
existing role-private one:

- `role_namespace`: unchanged. Per-role private state.
  (`cfo_advisor`, `ceo_advisor`, …)
- `shared_namespace`: new. Per-company workspace where any advisor can
  publish chunks others can read. Suggested default: the active
  tenant slug. So inside Ancestro AI's run, every advisor reads from
  AND writes to `tenant:ancestroai:shared`.

Recall semantics:

```rust
fn recall(...) -> Vec<Entry> {
    let role = role_recall(query, role_namespace);  // private
    let shared = shared_recall(query, shared_namespace, max_shared);
    merge_and_rerank(role, shared)
}
```

Write semantics: a new `scope` field on `memory_store` —
`"role" | "shared"`. Default `role` to preserve current behavior;
advisors who want their output visible to siblings pass
`scope: "shared"`.

### Interface changes

| File | Change |
|---|---|
| `crates/zeroclaw-config/src/schema.rs` | `DelegateAgentConfig` gains `shared_memory_namespace: Option<String>` — defaults to `Some("company_shared")` when a tenant is active |
| `crates/zeroclaw-memory/src/namespaced.rs` | Wrapper now keeps both namespaces; recall queries both and merges by score |
| `crates/zeroclaw-runtime/src/tools/memory.rs` | `memory_store` action gains `scope` parameter (`role`/`shared`); default `role` |
| Advisor presets | SENIOR_PREAMBLE gets a line: "Publish findings the bench needs via `memory_store scope='shared'`. Keep private deliberation in `scope='role'`." |

### Anti-goals

- No full graph of dependencies between agents. Shared memory is a
  bulletin board, not an orchestrator IR.
- No cross-tenant visibility. `tenant_id` payload filter on top of the
  shared namespace still applies.

### Effort estimate

~1 session (S). Touches 3 files, ~80 LOC, plus tests for the
namespaced backend (memory_recall returns merged + scope-correct).

---

## F3 — Support agents (email + chat)

### Problem

`crates/zeroclaw-channels/src/email_channel.rs` exists and listens on
IMAP. `discord.rs`, `telegram.rs`, `whatsapp.rs`, `mochat.rs` exist for
chat. But none of them route to a dedicated "support" preset — they
fall through to whatever default agent the orchestrator picks. For an
external user emailing `support@yourdomain.com`, that means a 5–10
minute round-trip through the full advisor delegation chain.

### Proposal

Two new presets in `crates/zeroclaw-config/src/agent_presets/`:

#### `support_email`

```rust
DelegateAgentConfig {
    provider: "openrouter",
    model: "deepseek/deepseek-v4-pro",
    temperature: 0.4,           // factual, low creativity
    agentic: true,
    max_iterations: 6,          // 1–2 round-trips, not advisor depth
    timeout_secs: 90,
    allowed_tools: [
        "memory_recall",        // search ticket history + product docs
        "knowledge",            // company FAQs in the KG
        "deliverable_write",    // ticket transcript / resolution memo
        "ask_user",             // request missing info from operator
        // explicitly NOT: web_fetch, file_write, shell, delegate
    ],
    memory_namespace: "support_email",
    skills_directory: "skills/customer-support",
}
```

Role prompt outline:
- Identify customer intent in <50 tokens.
- Recall the last 3–5 interactions from this email address from
  memory.
- Pull product/policy answers from the knowledge graph.
- Draft a response: empathetic, specific, no hallucinated specs.
- If we can't answer from cached context, escalate to a human (set a
  `escalate_to_human` event, don't send a guess).

#### `support_chat`

Same shape as `support_email` but:
- Lower `max_iterations: 4` — chat expects 1-shot replies.
- Lower `timeout_secs: 30` — users won't wait 90 s.
- Higher `temperature: 0.55` — slightly more conversational.
- Adds `web_search_tool` (chat customers sometimes want real-time
  lookups like "is your status page up?").

### Wiring

The channels already accept an `agent_target` config field. Update
`~/.zeroclaw/config.toml`:

```toml
[channels.email_channel]
enabled = true
imap_host = "imap.fastmail.com"
imap_port = 993
imap_user_env = "ZC_SUPPORT_EMAIL"
imap_pass_env = "ZC_SUPPORT_EMAIL_PASS"
poll_interval_secs = 60
agent_target = "support_email"        # NEW — route inbound here

[channels.telegram]
enabled = true
bot_token_env = "TELEGRAM_BOT_TOKEN"
agent_target = "support_chat"         # NEW — route inbound here
```

If a channel already implements `agent_target`, no code change needed
on the channel side. If not, the channel orchestrator route
(`crates/zeroclaw-channels/src/orchestrator/mod.rs:5691` and similar
sites) gets a per-channel agent override.

### Effort estimate

~1 session (S). New preset files (~150 LOC each), schema field
threading in 2 channels, integration test that an inbound message ends
up at the right preset. Audit `email_channel.rs` for `agent_target`
support first — may already exist.

---

## F4 — Debate orchestration

### Problem

The orchestrator delegates and collates. There's no mechanism for
agent A to read agent B's draft and disagree, no resolution mechanism
when they disagree, no audit trail of how the final answer was
reached.

### Proposal: a `debate` tool

New tool in `crates/zeroclaw-runtime/src/tools/debate.rs`. Exposed to
the orchestrator only.

#### Tool surface

```json
{
  "name": "debate",
  "description": "Run a structured round-robin discussion across N \
sub-agents. Each round: every participant produces a position; then \
every participant reads everyone else's position and produces a \
critique; finally a judge synthesizes. Use for strategic decisions \
where parallel monologues miss interactions.",
  "parameters": {
    "topic": "string — the question to deliberate",
    "participants": "array — agent names from [agents.*]",
    "rounds": "integer — propose+critique cycles (default 2, max 4)",
    "judge": "string — agent name that produces the synthesis (default 'ceo_advisor')",
    "context": "string — shared brief everyone sees in round 0"
  }
}
```

#### Execution sketch

```
Round 0 — Propose
  for each participant in parallel:
    participant.execute(role_prompt + topic + context)
    → write position to shared memory under `debate:<id>:r0:<agent>`

Round 1+ — Critique
  for each participant in parallel:
    participant.execute(role_prompt + topic + context +
                        all_prior_positions + critique_instruction)
    → write critique to shared memory under `debate:<id>:r<n>:<agent>`

Final — Synthesize
  judge.execute(role_prompt + topic + context + all_rounds +
                synthesize_instruction + decision_record_schema)
  → return decision record (json):
    {
      "decision": "...",
      "rationale": "...",
      "dissents": [...],
      "confidence": 0.0..1.0,
      "open_questions": [...]
    }
```

The decision record is persisted via `company_manifest
action='append_narrative'` so the manifest carries the audit trail of
how each strategic bet was decided. Reversible: a later debate can
revisit the same `topic` and produce a new decision record;
`company_manifest` keeps both.

#### Cost model

A debate with 4 participants × 2 rounds + 1 judge = **9 LLM calls** per
decision. At DeepSeek V4 Pro pricing ($0.435 input / $0.87 output per
1M) and an average prompt of 8 k tokens / 1 k output: ~$0.04 per
debate. The cost-tracker pre-flight (already shipped) shows this to
the operator before each `debate` invocation.

#### Termination guards

- Hard cap `rounds <= 4` (a debate that needs 5+ rounds isn't
  converging — escalate to a human instead).
- Critique rounds cap output tokens at 500 to keep position drift
  bounded.
- Judge sees all rounds but its output token cap is per-decision
  record schema (~600 tokens).

#### Failure modes & mitigation

| Failure | Mitigation |
|---|---|
| One participant times out | Drop them from the round; continue with the rest. Judge gets a note. |
| Judge can't synthesize (LLM error) | Fall back to "majority position wins" deterministic synthesis. Decision record carries `synthesis_method: "fallback_majority"`. |
| Participants converge on a wrong consensus | Add a mandatory `red_teamer` participant on every debate by default; their critique is treated as a dissent in the record. |
| Cost spike | Pre-flight budget block (already implemented) refuses the debate when the projected cost exceeds the configured limit. |

### Interface changes

| File | Change |
|---|---|
| `crates/zeroclaw-runtime/src/tools/debate.rs` | New file, the tool impl |
| `crates/zeroclaw-runtime/src/tools/mod.rs` | Register `DebateTool` |
| `crates/zeroclaw-config/src/agent_presets/orchestrator.rs` | Add `"debate"` to the orchestrator's allowed_tools |
| `docs/book/src/agents/debate.md` | Operator-facing docs |

### Effort estimate

~3 sessions (M). Session 1: tool surface + happy-path execution.
Session 2: judge synthesis + decision record persistence. Session 3:
failure modes, red-teamer auto-inject, observability.

---

## Sequencing & dependencies

```
F2 (shared memory)
  └─ F4 needs this: debate participants need to read each other's
     positions, which means writing them to a shared store.

F3 (support agents)
  └─ Independent of F2/F4. Can ship anytime.

F4 (debate)
  └─ Requires F2.
```

Recommended order: **F2 → F3 → F4**.

Reasoning: F2 unblocks F4 architecturally. F3 is the highest user-
facing impact per LOC (operator wires up an inbound email and the
system answers customers). F4 is the most exciting but also the
biggest risk surface — ship it after the supporting infrastructure is
in place.

---

## Out of scope (for this v2)

- **Plan-then-execute**: an explicit planner agent that writes a DAG
  before delegating. Worth exploring in v3.
- **Agent self-improvement loops**: post-decision reflection writes
  back to the agent's role prompt. Risky; defer.
- **Per-tenant agent rosters**: different companies want different
  bench compositions. Today the roster is global. Worth adding once
  multi-tenant prod traffic forces the question.

---

## Open questions

1. **Should shared memory be per-tenant or per-session?**
   Per-tenant means an agent reading shared memory tomorrow sees what
   the bench wrote today — useful for continuity, risky if positions
   evolve. Per-session is cleaner but breaks "last week's CFO model
   should inform this week's runway debate."
   Lean: per-tenant, with TTL on debate-round chunks (60 days).

2. **Who reads the dissents?**
   The synthesizer judge sees them; the persisted decision record
   carries them. Should the next debate on a related topic auto-
   surface them? Probably yes — `memory_recall` on the new topic
   would naturally pull related decision records via similarity.

3. **Do we need explicit roles in the debate (proposer / critic / red-team)?**
   The proposal above uses symmetric round-robin. An alternative is
   asymmetric: one proposes, two critique, one red-teams, one
   synthesizes. Lean: symmetric first; tune if outputs cluster.
