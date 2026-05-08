// Knowledge graph extension: non-profit + hybrid structures.

MERGE (d:Domain {name: "non-profit"});
MERGE (d:Domain {name: "hybrid-structure"});
MERGE (d:Domain {name: "tax-strategy"});

MERGE (f:RegulatoryFramework {name: "501(c)(3)"}) SET f.full_name = "Tax-exempt charity / private foundation", f.jurisdiction = "US", f.qdrant_category = "regulatory_nonprofit";
MERGE (f:RegulatoryFramework {name: "501(c)(4)"}) SET f.full_name = "Social welfare organization", f.jurisdiction = "US", f.qdrant_category = "regulatory_nonprofit";
MERGE (f:RegulatoryFramework {name: "Donor-Advised Fund"}) SET f.full_name = "DAF — sponsored charitable account", f.jurisdiction = "US", f.qdrant_category = "regulatory_nonprofit";
MERGE (f:RegulatoryFramework {name: "Public Benefit Corporation"}) SET f.full_name = "PBC — Delaware mission-balanced corp", f.jurisdiction = "US", f.qdrant_category = "regulatory_nonprofit";
MERGE (f:RegulatoryFramework {name: "Capped-Profit"}) SET f.full_name = "Non-profit parent + capped-return for-profit subsidiary (OpenAI pattern)", f.jurisdiction = "US", f.qdrant_category = "regulatory_nonprofit";
MERGE (f:RegulatoryFramework {name: "Fiscal Sponsorship"}) SET f.full_name = "Existing 501(c)(3) hosts a new project legally", f.jurisdiction = "US", f.qdrant_category = "regulatory_nonprofit";
MERGE (f:RegulatoryFramework {name: "Section 170"}) SET f.full_name = "Charitable contribution deduction", f.jurisdiction = "US", f.qdrant_category = "regulatory_nonprofit";
MERGE (f:RegulatoryFramework {name: "UK CIC"}) SET f.full_name = "Community Interest Company", f.jurisdiction = "UK", f.qdrant_category = "regulatory_nonprofit";

MATCH (f:RegulatoryFramework {name: "501(c)(3)"}), (d:Domain {name: "non-profit"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "501(c)(4)"}), (d:Domain {name: "non-profit"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "Donor-Advised Fund"}), (d:Domain {name: "tax-strategy"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "Public Benefit Corporation"}), (d:Domain {name: "hybrid-structure"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "Capped-Profit"}), (d:Domain {name: "hybrid-structure"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "Fiscal Sponsorship"}), (d:Domain {name: "non-profit"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "Section 170"}), (d:Domain {name: "tax-strategy"}) MERGE (f)-[:GOVERNS]->(d);
MATCH (f:RegulatoryFramework {name: "UK CIC"}), (d:Domain {name: "hybrid-structure"}) MERGE (f)-[:GOVERNS]->(d);

// Connect counsel agents to the new domains
MATCH (a:Agent {name: "legal_compliance"}), (d:Domain {name: "non-profit"}) MERGE (a)-[:CAN_ADVISE]->(d);
MATCH (a:Agent {name: "legal_compliance"}), (d:Domain {name: "tax-strategy"}) MERGE (a)-[:CAN_ADVISE]->(d);
MATCH (a:Agent {name: "legal_compliance"}), (d:Domain {name: "hybrid-structure"}) MERGE (a)-[:CAN_ADVISE]->(d);
MATCH (a:Agent {name: "cfo_advisor"}), (d:Domain {name: "tax-strategy"}) MERGE (a)-[:USES]->(d);
MATCH (a:Agent {name: "cfo_advisor"}), (d:Domain {name: "hybrid-structure"}) MERGE (a)-[:USES]->(d);
MATCH (a:Agent {name: "ceo_advisor"}), (d:Domain {name: "hybrid-structure"}) MERGE (a)-[:USES]->(d);
