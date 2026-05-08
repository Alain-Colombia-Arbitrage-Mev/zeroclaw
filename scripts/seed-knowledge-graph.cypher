// Octopus Labs knowledge graph seed (granular statements).
//
// Each MERGE statement is self-contained so FalkorDB's Cypher planner
// doesn't have to mix MERGE-creates with MATCH-lookups in one query
// (which silently drops nodes in some versions).

// ── Authors ─────────────────────────────────────────────────────────
MERGE (h:Author {name: "Alex Hormozi"}) SET h.focus = "service-business unit economics, offer construction";
MERGE (k:Author {name: "Nick Kolenda"}) SET k.focus = "behavioural persuasion, decision psychology";
MERGE (f:Author {name: "Scott Fox"}) SET f.focus = "online business model selection, lifestyle businesses";

// ── Books ───────────────────────────────────────────────────────────
MERGE (b:Book {title: "$100M Offers"}) SET b.year = 2021, b.qdrant_category = "hormozi", b.chunks = 147;
MERGE (b:Book {title: "Methods of Persuasion"}) SET b.year = 2013, b.qdrant_category = "persuasion", b.chunks = 235;
MERGE (b:Book {title: "Click Millionaires"}) SET b.qdrant_category = "hormozi", b.chunks = 229;
MERGE (b:Book {title: "21 Ways to Raise Fast Cash"}) SET b.qdrant_category = "library", b.chunks = 35;

// ── Author → Book ───────────────────────────────────────────────────
MATCH (h:Author {name: "Alex Hormozi"}), (b:Book {title: "$100M Offers"}) MERGE (h)-[:WROTE]->(b);
MATCH (k:Author {name: "Nick Kolenda"}), (b:Book {title: "Methods of Persuasion"}) MERGE (k)-[:WROTE]->(b);
MATCH (f:Author {name: "Scott Fox"}), (b:Book {title: "Click Millionaires"}) MERGE (f)-[:WROTE]->(b);

// ── Frameworks ──────────────────────────────────────────────────────
MERGE (f:Framework {name: "Value Equation"}) SET f.domain = "offer-construction", f.formula = "(Dream Outcome × Perceived Likelihood) / (Time Delay × Effort & Sacrifice)";
MERGE (f:Framework {name: "Grand Slam Offer"}) SET f.domain = "offer-construction", f.tagline = "An offer so good people feel stupid saying no";
MERGE (f:Framework {name: "Core Four"}) SET f.domain = "lead-generation", f.channels = "warm, cold, content, paid";
MERGE (f:Framework {name: "METHODS"}) SET f.domain = "persuasion", f.steps = "Mold-Elicit-Trigger-Habituate-Optimize-Drive-Sustain";
MERGE (f:Framework {name: "Virtuous Cycle of Price"}) SET f.domain = "pricing", f.summary = "Higher price → higher value perception → premium customers → reinvest → better product";
MERGE (f:Framework {name: "Test Ladder"}) SET f.domain = "validation", f.steps = "desk research → expert calls → smoke test → concierge MVP → paid pilot";
MERGE (f:Framework {name: "MEDDIC"}) SET f.domain = "sales", f.steps = "Metrics, Economic buyer, Decision criteria, Decision process, Identify pain, Champion";
MERGE (f:Framework {name: "Tactical Empathy"}) SET f.domain = "negotiation", f.tactics = "mirror, label, calibrated questions, accusation audit";

// ── Book → Framework ────────────────────────────────────────────────
MATCH (b:Book {title: "$100M Offers"}), (f:Framework {name: "Value Equation"}) MERGE (b)-[:CONTAINS]->(f);
MATCH (b:Book {title: "$100M Offers"}), (f:Framework {name: "Grand Slam Offer"}) MERGE (b)-[:CONTAINS]->(f);
MATCH (b:Book {title: "$100M Offers"}), (f:Framework {name: "Core Four"}) MERGE (b)-[:CONTAINS]->(f);
MATCH (b:Book {title: "$100M Offers"}), (f:Framework {name: "Virtuous Cycle of Price"}) MERGE (b)-[:CONTAINS]->(f);
MATCH (b:Book {title: "Methods of Persuasion"}), (f:Framework {name: "METHODS"}) MERGE (b)-[:CONTAINS]->(f);

// ── Principles ──────────────────────────────────────────────────────
MERGE (p:Principle {name: "Dream Outcome"}) SET p.direction = "increase", p.description = "what the customer truly wants";
MERGE (p:Principle {name: "Perceived Likelihood of Achievement"}) SET p.direction = "increase", p.description = "credibility, proof, guarantees raise this";
MERGE (p:Principle {name: "Time Delay"}) SET p.direction = "decrease", p.description = "shorter time-to-result lifts perceived value";
MERGE (p:Principle {name: "Effort & Sacrifice"}) SET p.direction = "decrease", p.description = "lower friction lifts perceived value";
MERGE (p:Principle {name: "Anchoring"}) SET p.description = "first number warps subsequent judgments";
MERGE (p:Principle {name: "Reciprocity"}) SET p.description = "free generous give creates pre-obligation";
MERGE (p:Principle {name: "Social Proof"}) SET p.description = "people do what they think similar others do";
MERGE (p:Principle {name: "Scarcity & Loss Aversion"}) SET p.description = "loss aversion approx 2x gain hope";
MERGE (p:Principle {name: "Behavioral Consistency"}) SET p.description = "tiny commitments dramatically raise later compliance";
MERGE (p:Principle {name: "Priming"}) SET p.description = "subtle cues shift later decisions";
MERGE (p:Principle {name: "Risk Reversal"}) SET p.description = "guarantee shifts risk from buyer to seller";

// ── Framework → Principle ───────────────────────────────────────────
MATCH (f:Framework {name: "Value Equation"}), (p:Principle {name: "Dream Outcome"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);
MATCH (f:Framework {name: "Value Equation"}), (p:Principle {name: "Perceived Likelihood of Achievement"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);
MATCH (f:Framework {name: "Value Equation"}), (p:Principle {name: "Time Delay"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);
MATCH (f:Framework {name: "Value Equation"}), (p:Principle {name: "Effort & Sacrifice"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);
MATCH (f:Framework {name: "Grand Slam Offer"}), (p:Principle {name: "Scarcity & Loss Aversion"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);
MATCH (f:Framework {name: "Grand Slam Offer"}), (p:Principle {name: "Risk Reversal"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);
MATCH (f:Framework {name: "METHODS"}), (p:Principle {name: "Anchoring"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);
MATCH (f:Framework {name: "METHODS"}), (p:Principle {name: "Reciprocity"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);
MATCH (f:Framework {name: "METHODS"}), (p:Principle {name: "Social Proof"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);
MATCH (f:Framework {name: "METHODS"}), (p:Principle {name: "Scarcity & Loss Aversion"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);
MATCH (f:Framework {name: "METHODS"}), (p:Principle {name: "Behavioral Consistency"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);
MATCH (f:Framework {name: "METHODS"}), (p:Principle {name: "Priming"}) MERGE (f)-[:HAS_PRINCIPLE]->(p);

// ── Domains ─────────────────────────────────────────────────────────
MERGE (d:Domain {name: "pricing"});
MERGE (d:Domain {name: "offer-construction"});
MERGE (d:Domain {name: "lead-generation"});
MERGE (d:Domain {name: "sales"});
MERGE (d:Domain {name: "copy"});
MERGE (d:Domain {name: "onboarding"});
MERGE (d:Domain {name: "negotiation"});
MERGE (d:Domain {name: "validation"});
MERGE (d:Domain {name: "growth"});

// ── Principle → Domain ──────────────────────────────────────────────
MATCH (p:Principle {name: "Anchoring"}), (d:Domain {name: "pricing"}) MERGE (p)-[:APPLIES_TO]->(d);
MATCH (p:Principle {name: "Anchoring"}), (d:Domain {name: "negotiation"}) MERGE (p)-[:APPLIES_TO]->(d);
MATCH (p:Principle {name: "Reciprocity"}), (d:Domain {name: "lead-generation"}) MERGE (p)-[:APPLIES_TO]->(d);
MATCH (p:Principle {name: "Reciprocity"}), (d:Domain {name: "sales"}) MERGE (p)-[:APPLIES_TO]->(d);
MATCH (p:Principle {name: "Social Proof"}), (d:Domain {name: "copy"}) MERGE (p)-[:APPLIES_TO]->(d);
MATCH (p:Principle {name: "Social Proof"}), (d:Domain {name: "sales"}) MERGE (p)-[:APPLIES_TO]->(d);
MATCH (p:Principle {name: "Scarcity & Loss Aversion"}), (d:Domain {name: "offer-construction"}) MERGE (p)-[:APPLIES_TO]->(d);
MATCH (p:Principle {name: "Scarcity & Loss Aversion"}), (d:Domain {name: "copy"}) MERGE (p)-[:APPLIES_TO]->(d);
MATCH (p:Principle {name: "Behavioral Consistency"}), (d:Domain {name: "onboarding"}) MERGE (p)-[:APPLIES_TO]->(d);
MATCH (p:Principle {name: "Behavioral Consistency"}), (d:Domain {name: "sales"}) MERGE (p)-[:APPLIES_TO]->(d);
MATCH (p:Principle {name: "Risk Reversal"}), (d:Domain {name: "offer-construction"}) MERGE (p)-[:APPLIES_TO]->(d);

// ── Agents ──────────────────────────────────────────────────────────
MERGE (a:Agent {name: "pricing_strategist"}) SET a.focus = "tier design, value metric, anchoring";
MERGE (a:Agent {name: "copywriter"}) SET a.focus = "direct-response copy";
MERGE (a:Agent {name: "negotiator"}) SET a.focus = "deal prep, BATNA, tactical empathy";
MERGE (a:Agent {name: "marketing"}) SET a.focus = "brand, GTM, demand gen";
MERGE (a:Agent {name: "growth_hacker"}) SET a.focus = "experiments, loops, attribution";
MERGE (a:Agent {name: "business_developer"}) SET a.focus = "partnerships, distribution";
MERGE (a:Agent {name: "idea_validator"}) SET a.focus = "disconfirmation tests";
MERGE (a:Agent {name: "customer_researcher"}) SET a.focus = "JTBD discovery";
MERGE (a:Agent {name: "ceo_advisor"}) SET a.focus = "weekly synthesis";
MERGE (a:Agent {name: "account_executive"}) SET a.focus = "MEDDIC sales";
MERGE (a:Agent {name: "customer_success"}) SET a.focus = "TTFV, churn signals";
MERGE (a:Agent {name: "sdr_outbound"}) SET a.focus = "pipeline generation";

// ── Agent → Framework (USES) ────────────────────────────────────────
MATCH (a:Agent {name: "pricing_strategist"}), (f:Framework {name: "Value Equation"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "pricing_strategist"}), (f:Framework {name: "Virtuous Cycle of Price"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "copywriter"}), (f:Framework {name: "METHODS"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "copywriter"}), (f:Framework {name: "Value Equation"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "negotiator"}), (f:Framework {name: "Tactical Empathy"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "negotiator"}), (f:Framework {name: "METHODS"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "marketing"}), (f:Framework {name: "METHODS"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "marketing"}), (f:Framework {name: "Grand Slam Offer"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "growth_hacker"}), (f:Framework {name: "Core Four"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "idea_validator"}), (f:Framework {name: "Test Ladder"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "account_executive"}), (f:Framework {name: "MEDDIC"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "account_executive"}), (f:Framework {name: "Tactical Empathy"}) MERGE (a)-[:USES]->(f);

// ── Agent → Domain (OWNS) ───────────────────────────────────────────
MATCH (a:Agent {name: "pricing_strategist"}), (d:Domain {name: "pricing"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "copywriter"}), (d:Domain {name: "copy"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "negotiator"}), (d:Domain {name: "negotiation"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "growth_hacker"}), (d:Domain {name: "growth"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "business_developer"}), (d:Domain {name: "lead-generation"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "idea_validator"}), (d:Domain {name: "validation"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "account_executive"}), (d:Domain {name: "sales"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "customer_success"}), (d:Domain {name: "onboarding"}) MERGE (a)-[:OWNS]->(d);
