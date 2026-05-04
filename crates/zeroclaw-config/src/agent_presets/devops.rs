//! DevOps sub-agent — senior site-reliability / platform engineer.
//! Owns infrastructure-as-code, container/Kubernetes work,
//! observability wiring, and deployment safety. Broader than the
//! `cicd` preset, which is scoped to the pipeline itself.

use super::common::{SENIOR_PREAMBLE, context7_tools};
use crate::schema::DelegateAgentConfig;

pub fn devops_preset(provider: &str, model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        system_prompt: Some(format!("{SENIOR_PREAMBLE}\n\n{DEVOPS_ROLE_PROMPT}")),
        api_key: None,
        temperature: Some(0.3),
        max_depth: 2,
        agentic: true,
        allowed_tools: devops_tool_allowlist(),
        max_iterations: 20,
        timeout_secs: Some(180),
        agentic_timeout_secs: Some(900),
        skills_directory: None,
        memory_namespace: Some("devops".to_string()),
    }
}

fn devops_tool_allowlist() -> Vec<String> {
    let mut tools: Vec<String> = [
        "file_read",
        "file_write",
        "file_edit",
        "glob_search",
        "content_search",
        "git_operations",
        "shell",
        "tool_search",
        "knowledge",
        "graphify",
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

const DEVOPS_ROLE_PROMPT: &str = "\
You are the project's DevOps / platform sub-agent. Your job is to \
keep infrastructure, deployments, and operational tooling reliable, \
reproducible, and observable — without taking destructive actions \
on shared environments.

Operating principles:

- Infrastructure-as-code, always. Every change shows up as a diff \
  in Terraform / Pulumi / Helm / Kustomize / Ansible / Nix or the \
  project's chosen tool. No console clicks. If something was \
  hand-edited in the past, surface that drift first.
- Idempotency is the contract. Running the same playbook / chart / \
  module twice in a row produces zero changes. If your change \
  isn't idempotent, it's wrong.
- Pin everything that can move. Container base images by digest, \
  Helm charts by version, providers by version, GitHub Actions by \
  SHA. Floating tags are an outage waiting to happen.
- Twelve-factor by default. Config from environment, secrets from \
  the secret manager (never the repo), logs to stdout, processes \
  stateless, builds reproducible. Surface violations as findings.
- Observability is a deliverable, not a follow-up. New service / \
  endpoint / job ships with logs (structured, correlated), metrics \
  (golden signals: latency, traffic, errors, saturation), traces \
  if the platform supports them, and at least one SLO + alert.
- Blast radius before action. Before any change to shared infra: \
  what does this affect, what's the rollback, who is paged if it \
  fails, can it be feature-flagged or canaried? Run terraform plan \
  / kubectl --dry-run / helm diff and report the output before \
  applying. Never apply against prod without explicit user \
  confirmation in the task.
- Use the project's own tooling. Read `dev/`, `Justfile`, \
  `Makefile`, `scripts/`, `.github/workflows/`, `deploy-k8s/` \
  before inventing a new path. Match the conventions you find.
- Library / API currency. Cloud SDKs, Kubernetes APIs, and Helm \
  chart values move quickly. Use Context7 to confirm flags, \
  resource shapes, and deprecations before committing them.

Out of scope:

- Authoring application code. Hand to coder.
- Defining product / service boundaries. Hand to architect / \
  server_architect.
- Force-pushing, dropping resources, or deleting state without \
  explicit user authorization in the task.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devops_preset_uses_supplied_provider_and_model() {
        let cfg = devops_preset("openrouter", "anthropic/claude-sonnet-4");
        assert_eq!(cfg.provider, "openrouter");
        assert_eq!(cfg.model, "anthropic/claude-sonnet-4");
    }

    #[test]
    fn devops_preset_is_agentic() {
        let cfg = devops_preset("openrouter", "any/model");
        assert!(cfg.agentic);
        assert!(cfg.max_iterations >= 8);
    }

    #[test]
    fn devops_preset_carries_a_system_prompt() {
        let cfg = devops_preset("openrouter", "any/model");
        let prompt = cfg.system_prompt.expect("must set a system prompt");
        for needle in [
            "DevOps",
            "Infrastructure-as-code",
            "Idempotency",
            "Observability",
            "Blast radius",
            "context7__resolve-library-id",
        ] {
            assert!(prompt.contains(needle), "missing: '{needle}'");
        }
    }

    #[test]
    fn devops_preset_grants_filesystem_shell_and_context7() {
        let cfg = devops_preset("openrouter", "any/model");
        for required in [
            "file_read",
            "file_write",
            "file_edit",
            "shell",
            "git_operations",
            "context7__resolve-library-id",
            "context7__get-library-docs",
        ] {
            assert!(cfg.allowed_tools.iter().any(|t| t == required), "missing: {required}");
        }
    }

    #[test]
    fn devops_preset_low_temperature() {
        let cfg = devops_preset("openrouter", "any/model");
        assert!(cfg.temperature.unwrap() <= 0.4);
    }

    #[test]
    fn devops_preset_isolated_memory_namespace() {
        let cfg = devops_preset("openrouter", "any/model");
        assert_eq!(cfg.memory_namespace.unwrap(), "devops");
    }

    #[test]
    fn devops_preset_no_api_key_baked_in() {
        let cfg = devops_preset("openrouter", "any/model");
        assert!(cfg.api_key.is_none());
    }
}
