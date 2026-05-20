//! Dump selected agent presets as ready-to-paste TOML.
//!
//! Run: `cargo run --release --example dump_presets -p zeroclaw-config > /tmp/new-agents.toml`
//!
//! The output is operator-safe: secrets stay out (`api_key` is always
//! omitted), comments label each block, and the prompts are written
//! with TOML triple-quoted strings so escaping stays human-readable.

use zeroclaw_config::agent_presets::{
    bayesian_priors_keeper_preset, blue_ocean_strategist_preset, calibration_scorer_preset,
    decision_scientist_preset, neuromarketing_godin_preset, pmf_strategist_preset,
    quant_analyst_preset, scenario_planner_preset, self_funding_growth_strategist_preset,
    unicorn_captain_preset, wellness_dispenza_preset,
};
use zeroclaw_config::schema::DelegateAgentConfig;

fn dump(name: &str, cfg: DelegateAgentConfig) {
    println!("[agents.{name}]");
    println!("provider = \"{}\"", cfg.provider);
    println!("model = \"{}\"", cfg.model);
    if let Some(t) = cfg.temperature {
        println!("temperature = {t}");
    }
    println!("agentic = {}", cfg.agentic);
    println!("max_depth = {}", cfg.max_depth);
    println!("max_iterations = {}", cfg.max_iterations);
    if let Some(s) = cfg.timeout_secs {
        println!("timeout_secs = {s}");
    }
    if let Some(s) = cfg.agentic_timeout_secs {
        println!("agentic_timeout_secs = {s}");
    }
    if let Some(ns) = cfg.memory_namespace.as_ref() {
        println!("memory_namespace = \"{ns}\"");
    }
    if let Some(sd) = cfg.skills_directory.as_ref() {
        println!("skills_directory = \"{sd}\"");
    }
    println!("allowed_tools = [");
    for t in &cfg.allowed_tools {
        println!("    \"{t}\",");
    }
    println!("]");
    if let Some(prompt) = cfg.system_prompt.as_ref() {
        // Triple-SINGLE-quote TOML literal string. Backslashes are NOT
        // interpreted as escape sequences inside `'''...'''` — so
        // sequences like `\$10M` (legal in Rust source after string
        // unescape) survive the round-trip without TOML rejecting
        // them as invalid escapes.
        //
        // Defensive: a prompt that genuinely contains the sequence
        // `'''` would prematurely end the literal. None of the
        // current presets do, but split such runs with a zero-width
        // joiner-equivalent — here, a comment break — if discovered.
        let safe = prompt.replace("'''", "''\\u{200B}'");
        println!("system_prompt = '''\n{safe}\n'''");
    }
    println!();
}

fn main() {
    // Provider + model per preset matches the operator's existing
    // config tier conventions (Mimo S3 / GPT-5 S2).
    let presets: Vec<(&str, DelegateAgentConfig)> = vec![
        (
            "pmf_strategist",
            pmf_strategist_preset("openrouter", "xiaomi/mimo-v2.5-pro"),
        ),
        (
            "quant_analyst",
            quant_analyst_preset("openrouter", "xiaomi/mimo-v2.5-pro"),
        ),
        (
            "decision_scientist",
            decision_scientist_preset("openrouter", "xiaomi/mimo-v2.5-pro"),
        ),
        (
            "calibration_scorer",
            calibration_scorer_preset("openrouter", "xiaomi/mimo-v2.5-pro"),
        ),
        (
            "scenario_planner",
            scenario_planner_preset("openrouter", "xiaomi/mimo-v2.5-pro"),
        ),
        (
            "bayesian_priors_keeper",
            bayesian_priors_keeper_preset("openrouter", "xiaomi/mimo-v2.5-pro"),
        ),
        (
            "neuromarketing_godin",
            neuromarketing_godin_preset("openrouter", "xiaomi/mimo-v2.5-pro"),
        ),
        (
            "wellness_dispenza",
            wellness_dispenza_preset("openrouter", "xiaomi/mimo-v2.5-pro"),
        ),
        (
            "blue_ocean_strategist",
            blue_ocean_strategist_preset("openrouter", "xiaomi/mimo-v2.5-pro"),
        ),
        (
            "unicorn_captain",
            unicorn_captain_preset("openrouter", "openai/gpt-5"),
        ),
        (
            "self_funding_growth_strategist",
            self_funding_growth_strategist_preset("openrouter", "openai/gpt-5"),
        ),
    ];

    println!("# === New agent presets (auto-generated 2026-05-19) ===");
    println!("# Source: integration/2026-05-19 branch preset constructors.");
    println!("# Each block carries the full system_prompt and allowed_tools so");
    println!("# the daemon registers the agent as fully wired, NOT as a stub.");
    println!();

    for (name, cfg) in presets {
        dump(name, cfg);
    }
}
