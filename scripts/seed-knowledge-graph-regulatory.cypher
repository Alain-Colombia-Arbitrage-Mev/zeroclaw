// Knowledge graph extension: regulatory frameworks across 4 verticals.
// Apply with: bash scripts/apply-kg-seed.sh scripts/seed-knowledge-graph-regulatory.cypher

// ── Regulated domains (extend existing Domain set) ──────────────────
MERGE (d:Domain {name: "clean-energy"});
MERGE (d:Domain {name: "crypto"});
MERGE (d:Domain {name: "blockchain"});
MERGE (d:Domain {name: "payments"});
MERGE (d:Domain {name: "esg-disclosure"});

// ── Regulatory frameworks ───────────────────────────────────────────
MERGE (f:RegulatoryFramework {name: "IRA"}) SET f.full_name = "Inflation Reduction Act of 2022", f.jurisdiction = "US", f.qdrant_category = "regulatory_energy";
MERGE (f:RegulatoryFramework {name: "EU Green Deal"}) SET f.jurisdiction = "EU", f.qdrant_category = "regulatory_energy";
MERGE (f:RegulatoryFramework {name: "CBAM"}) SET f.full_name = "Carbon Border Adjustment Mechanism", f.jurisdiction = "EU", f.qdrant_category = "regulatory_energy", f.financial_phase = 2026;
MERGE (f:RegulatoryFramework {name: "CSRD"}) SET f.full_name = "Corporate Sustainability Reporting Directive", f.jurisdiction = "EU", f.qdrant_category = "regulatory_energy";
MERGE (f:RegulatoryFramework {name: "EU ETS"}) SET f.full_name = "EU Emissions Trading System", f.jurisdiction = "EU", f.qdrant_category = "regulatory_energy";
MERGE (f:RegulatoryFramework {name: "MiCA"}) SET f.full_name = "Markets in Crypto-Assets Regulation", f.jurisdiction = "EU", f.qdrant_category = "regulatory_crypto";
MERGE (f:RegulatoryFramework {name: "Howey Test"}) SET f.full_name = "SEC v. W.J. Howey 1946", f.jurisdiction = "US", f.qdrant_category = "regulatory_crypto";
MERGE (f:RegulatoryFramework {name: "BSA"}) SET f.full_name = "Bank Secrecy Act", f.jurisdiction = "US", f.qdrant_category = "regulatory_finance";
MERGE (f:RegulatoryFramework {name: "FATF Travel Rule"}) SET f.jurisdiction = "Global", f.qdrant_category = "regulatory_crypto";
MERGE (f:RegulatoryFramework {name: "Reg D"}) SET f.full_name = "Securities Act of 1933 Regulation D", f.jurisdiction = "US", f.qdrant_category = "regulatory_finance";
MERGE (f:RegulatoryFramework {name: "PCI-DSS"}) SET f.full_name = "Payment Card Industry Data Security Standard", f.jurisdiction = "Global", f.qdrant_category = "regulatory_finance", f.version = "v4.0.1";
MERGE (f:RegulatoryFramework {name: "PSD2"}) SET f.full_name = "Payment Services Directive 2", f.jurisdiction = "EU", f.qdrant_category = "regulatory_finance";
MERGE (f:RegulatoryFramework {name: "SOX"}) SET f.full_name = "Sarbanes-Oxley Act", f.jurisdiction = "US", f.qdrant_category = "regulatory_finance";
MERGE (f:RegulatoryFramework {name: "OECD Pillar Two"}) SET f.full_name = "Global Minimum Tax 15%", f.jurisdiction = "OECD", f.qdrant_category = "regulatory_finance";
MERGE (f:RegulatoryFramework {name: "Wyoming DAO LLC"}) SET f.jurisdiction = "US-WY", f.qdrant_category = "regulatory_blockchain";
MERGE (f:RegulatoryFramework {name: "Cayman Foundation"}) SET f.jurisdiction = "Cayman", f.qdrant_category = "regulatory_blockchain";
MERGE (f:RegulatoryFramework {name: "OFAC SDN"}) SET f.full_name = "OFAC Specially Designated Nationals list", f.jurisdiction = "US", f.qdrant_category = "regulatory_crypto";
MERGE (f:RegulatoryFramework {name: "California SB 253"}) SET f.full_name = "Climate Corporate Data Accountability Act", f.jurisdiction = "US-CA", f.qdrant_category = "regulatory_energy";

// ── Framework → Domain ──────────────────────────────────────────────
MATCH (f:RegulatoryFramework {name: "IRA"}), (d:Domain {name: "clean-energy"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "EU Green Deal"}), (d:Domain {name: "clean-energy"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "CBAM"}), (d:Domain {name: "clean-energy"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "CSRD"}), (d:Domain {name: "esg-disclosure"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "EU ETS"}), (d:Domain {name: "clean-energy"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "California SB 253"}), (d:Domain {name: "esg-disclosure"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "MiCA"}), (d:Domain {name: "crypto"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "Howey Test"}), (d:Domain {name: "crypto"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "BSA"}), (d:Domain {name: "payments"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "BSA"}), (d:Domain {name: "crypto"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "FATF Travel Rule"}), (d:Domain {name: "crypto"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "Reg D"}), (d:Domain {name: "crypto"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "PCI-DSS"}), (d:Domain {name: "payments"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "PSD2"}), (d:Domain {name: "payments"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "Wyoming DAO LLC"}), (d:Domain {name: "blockchain"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "Cayman Foundation"}), (d:Domain {name: "blockchain"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "OFAC SDN"}), (d:Domain {name: "crypto"}) MERGE (f)-[:GOVERNS]->(d);

// ── New specialist agents ───────────────────────────────────────────
MERGE (a:Agent {name: "fintech_counsel"}) SET a.focus = "BSA, MiCA, Howey, PCI-DSS, MSB licensing";
MERGE (a:Agent {name: "esg_energy_counsel"}) SET a.focus = "IRA stack, CBAM, CSRD, voluntary carbon markets";

// ── Agent → Framework (USES) ────────────────────────────────────────
MATCH (a:Agent {name: "fintech_counsel"}), (f:RegulatoryFramework {name: "BSA"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "fintech_counsel"}), (f:RegulatoryFramework {name: "MiCA"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "fintech_counsel"}), (f:RegulatoryFramework {name: "Howey Test"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "fintech_counsel"}), (f:RegulatoryFramework {name: "PCI-DSS"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "fintech_counsel"}), (f:RegulatoryFramework {name: "Reg D"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "fintech_counsel"}), (f:RegulatoryFramework {name: "FATF Travel Rule"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "fintech_counsel"}), (f:RegulatoryFramework {name: "OFAC SDN"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "fintech_counsel"}), (f:RegulatoryFramework {name: "Wyoming DAO LLC"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "fintech_counsel"}), (f:RegulatoryFramework {name: "Cayman Foundation"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "esg_energy_counsel"}), (f:RegulatoryFramework {name: "IRA"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "esg_energy_counsel"}), (f:RegulatoryFramework {name: "EU Green Deal"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "esg_energy_counsel"}), (f:RegulatoryFramework {name: "CBAM"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "esg_energy_counsel"}), (f:RegulatoryFramework {name: "CSRD"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "esg_energy_counsel"}), (f:RegulatoryFramework {name: "EU ETS"}) MERGE (a)-[:USES]->(f);
MATCH (a:Agent {name: "esg_energy_counsel"}), (f:RegulatoryFramework {name: "California SB 253"}) MERGE (a)-[:USES]->(f);

// ── Agent → Domain (OWNS) ───────────────────────────────────────────
MATCH (a:Agent {name: "fintech_counsel"}), (d:Domain {name: "crypto"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "fintech_counsel"}), (d:Domain {name: "blockchain"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "fintech_counsel"}), (d:Domain {name: "payments"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "esg_energy_counsel"}), (d:Domain {name: "clean-energy"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "esg_energy_counsel"}), (d:Domain {name: "esg-disclosure"}) MERGE (a)-[:OWNS]->(d);
MATCH (a:Agent {name: "legal_compliance"}), (d:Domain {name: "esg-disclosure"}) MERGE (a)-[:CAN_ADVISE]->(d);
MATCH (a:Agent {name: "legal_compliance"}), (d:Domain {name: "payments"}) MERGE (a)-[:CAN_ADVISE]->(d);
MATCH (a:Agent {name: "legal_compliance"}), (d:Domain {name: "crypto"}) MERGE (a)-[:CAN_ADVISE]->(d);
