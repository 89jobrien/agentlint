use agentlint_core::{Diagnostic, Difficulty, Validator};
use std::path::Path;

mod commands;
mod hooks;
mod skills;

// -------------------------------------------------------------------------
// Known event triggers (from looprs-core domain_event! macro)
// -------------------------------------------------------------------------

pub const KNOWN_TRIGGERS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "InferenceComplete",
    "OnError",
    "OnWarning",
    "DelegationStart",
    "DelegationComplete",
];

// -------------------------------------------------------------------------
// Validators
// -------------------------------------------------------------------------

/// Validates `.looprs/commands/*.yaml` and `.looprs/commands/*.yml`.
pub struct CommandsValidator;

impl Validator for CommandsValidator {
    fn patterns(&self) -> &[&str] {
        &[".looprs/commands/*.yaml", ".looprs/commands/*.yml"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        commands::validate(path, src)
    }
}

/// Validates `.looprs/hooks/*.yaml` and `.looprs/hooks/*.yml`.
pub struct HooksValidator;

impl Validator for HooksValidator {
    fn patterns(&self) -> &[&str] {
        &[".looprs/hooks/*.yaml", ".looprs/hooks/*.yml"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        hooks::validate(path, src)
    }
}

/// Validates `.looprs/skills/**/SKILL.md`.
pub struct SkillsValidator;

impl Validator for SkillsValidator {
    fn patterns(&self) -> &[&str] {
        &[".looprs/skills/**/SKILL.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        skills::validate(path, src)
    }
}

/// Validates `.looprs/agents/*.yaml` and `.looprs/agents/*.yml`.
pub struct AgentsValidator;

impl Validator for AgentsValidator {
    fn patterns(&self) -> &[&str] {
        &[".looprs/agents/*.yaml", ".looprs/agents/*.yml"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        agents::validate(path, src)
    }
}

mod agents {
    use super::*;

    #[derive(serde::Deserialize)]
    struct AgentDef {
        #[serde(default)]
        name: String,
    }

    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        if src.trim().is_empty() {
            diags.push(
                Diagnostic::error(path, 1, 1, "agent file is empty")
                    .with_rule("looprs/agents/empty", Difficulty::Easy),
            );
            return diags;
        }

        let agent: AgentDef = match serde_yaml::from_str(src) {
            Ok(a) => a,
            Err(e) => {
                diags.push(
                    Diagnostic::error(path, 1, 1, format!("invalid YAML: {e}"))
                        .with_rule("looprs/agents/invalid-yaml", Difficulty::Easy),
                );
                return diags;
            }
        };

        if agent.name.trim().is_empty() {
            diags.push(
                Diagnostic::error(path, 1, 1, "missing or empty 'name' field")
                    .with_rule("looprs/agents/missing-name", Difficulty::Easy),
            );
        }

        diags
    }
}

/// Validates `.looprs/rules/*.md`.
pub struct RulesValidator;

impl Validator for RulesValidator {
    fn patterns(&self) -> &[&str] {
        &[".looprs/rules/*.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        rules::validate(path, src)
    }
}

mod rules {
    use super::*;

    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        // Skip README files.
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.eq_ignore_ascii_case("README.md"))
            .unwrap_or(false)
        {
            return vec![];
        }

        let mut diags = Vec::new();

        if src.trim().is_empty() {
            diags.push(
                Diagnostic::error(path, 1, 1, "rule file is empty")
                    .with_rule("looprs/rules/empty", Difficulty::Easy),
            );
            return diags;
        }

        let has_heading = src.lines().any(|l| l.starts_with('#'));
        if !has_heading {
            diags.push(
                Diagnostic::warning(path, 1, 1, "rule file has no markdown heading")
                    .with_rule("looprs/rules/no-heading", Difficulty::Hard),
            );
        }

        diags
    }
}

/// Validates `.looprs/config.json`.
pub struct ConfigValidator;

impl Validator for ConfigValidator {
    fn patterns(&self) -> &[&str] {
        &[".looprs/config.json"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if src.trim().is_empty() {
            return vec![
                Diagnostic::error(path, 1, 1, "config.json is empty")
                    .with_rule("looprs/config/empty", Difficulty::Easy),
            ];
        }
        if let Err(e) = serde_json::from_str::<serde_json::Value>(src) {
            return vec![
                Diagnostic::error(path, 1, 1, format!("invalid JSON: {e}"))
                    .with_rule("looprs/config/invalid-json", Difficulty::Easy),
            ];
        }
        vec![]
    }
}

/// Validates `*.agent.json` files.
pub struct AgentJsonValidator;

impl Validator for AgentJsonValidator {
    fn patterns(&self) -> &[&str] {
        &["*.agent.json"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        agent_json::validate(path, src)
    }
}

mod agent_json {
    use super::*;

    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        if src.trim().is_empty() {
            diags.push(
                Diagnostic::error(path, 1, 1, "agent.json is empty")
                    .with_rule("looprs/agent-json/empty", Difficulty::Easy),
            );
            return diags;
        }

        let val: serde_json::Value = match serde_json::from_str(src) {
            Ok(v) => v,
            Err(e) => {
                diags.push(
                    Diagnostic::error(path, 1, 1, format!("invalid JSON: {e}"))
                        .with_rule("looprs/agent-json/invalid-json", Difficulty::Easy),
                );
                return diags;
            }
        };

        let obj = match val.as_object() {
            Some(o) => o,
            None => {
                diags.push(
                    Diagnostic::error(path, 1, 1, "agent.json must be a JSON object")
                        .with_rule("looprs/agent-json/not-object", Difficulty::Easy),
                );
                return diags;
            }
        };

        if !obj.contains_key("version") {
            diags.push(
                Diagnostic::warning(path, 1, 1, "missing 'version' field")
                    .with_rule("looprs/agent-json/missing-version", Difficulty::Hard),
            );
        }

        if !obj.contains_key("agent_schema") {
            diags.push(
                Diagnostic::warning(path, 1, 1, "missing 'agent_schema' field")
                    .with_rule("looprs/agent-json/missing-schema", Difficulty::Hard),
            );
        }

        diags
    }
}

// -------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentlint_core::testing::assert_validator_contract;
    use std::path::Path;

    #[test]
    fn conformance_commands() {
        assert_validator_contract(&CommandsValidator);
    }

    #[test]
    fn conformance_hooks() {
        assert_validator_contract(&HooksValidator);
    }

    #[test]
    fn conformance_skills() {
        assert_validator_contract(&SkillsValidator);
    }

    #[test]
    fn conformance_agents() {
        assert_validator_contract(&AgentsValidator);
    }

    #[test]
    fn conformance_rules() {
        assert_validator_contract(&RulesValidator);
    }

    #[test]
    fn conformance_config() {
        assert_validator_contract(&ConfigValidator);
    }

    #[test]
    fn conformance_agent_json() {
        assert_validator_contract(&AgentJsonValidator);
    }

    // --- commands ---

    #[test]
    fn valid_command_is_clean() {
        let src = "name: lint\ndescription: Run linter\naction:\n  type: shell\n  command: cargo clippy\n";
        let diags = CommandsValidator.validate(Path::new(".looprs/commands/lint.yaml"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn command_missing_name_errors() {
        let src = "description: Run linter\naction:\n  type: shell\n  command: cargo clippy\n";
        let diags = CommandsValidator.validate(Path::new(".looprs/commands/lint.yaml"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-name")));
    }

    #[test]
    fn command_invalid_action_type_errors() {
        let src = "name: x\ndescription: y\naction:\n  type: unknown\n";
        let diags = CommandsValidator.validate(Path::new(".looprs/commands/x.yaml"), src);
        assert!(diags.iter().any(|d| d.rule.contains("invalid-yaml")));
    }

    #[test]
    fn command_empty_errors() {
        let diags = CommandsValidator.validate(Path::new(".looprs/commands/x.yaml"), "");
        assert!(diags.iter().any(|d| d.rule.contains("empty")));
    }

    // --- hooks ---

    #[test]
    fn valid_hook_is_clean() {
        let src = "name: start\ntrigger: SessionStart\nactions:\n  - type: message\n    text: hi\n";
        let diags = HooksValidator.validate(Path::new(".looprs/hooks/start.yaml"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn hook_missing_trigger_errors() {
        let src = "name: start\nactions:\n  - type: message\n    text: hi\n";
        let diags = HooksValidator.validate(Path::new(".looprs/hooks/start.yaml"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-trigger")));
    }

    #[test]
    fn hook_unknown_trigger_warns() {
        let src = "name: start\ntrigger: FakeEvent\nactions:\n  - type: message\n    text: hi\n";
        let diags = HooksValidator.validate(Path::new(".looprs/hooks/start.yaml"), src);
        assert!(diags.iter().any(|d| d.rule.contains("unknown-trigger")));
    }

    #[test]
    fn hook_empty_actions_errors() {
        let src = "name: start\ntrigger: SessionStart\nactions: []\n";
        let diags = HooksValidator.validate(Path::new(".looprs/hooks/start.yaml"), src);
        assert!(diags.iter().any(|d| d.rule.contains("empty-actions")));
    }

    // --- skills ---

    #[test]
    fn valid_skill_is_clean() {
        let src = "---\nname: test-skill\ntriggers:\n  - test\n---\n# Content\n";
        let diags = SkillsValidator.validate(Path::new(".looprs/skills/test/SKILL.md"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn skill_missing_frontmatter_errors() {
        let src = "# Just content\nNo frontmatter.\n";
        let diags = SkillsValidator.validate(Path::new(".looprs/skills/test/SKILL.md"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-frontmatter")));
    }

    #[test]
    fn skill_missing_name_errors() {
        let src = "---\ntriggers:\n  - test\n---\n# Content\n";
        let diags = SkillsValidator.validate(Path::new(".looprs/skills/test/SKILL.md"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-name")));
    }

    #[test]
    fn skill_missing_triggers_errors() {
        let src = "---\nname: test\n---\n# Content\n";
        let diags = SkillsValidator.validate(Path::new(".looprs/skills/test/SKILL.md"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-triggers")));
    }

    // --- agents ---

    #[test]
    fn valid_agent_is_clean() {
        let src = "name: reviewer\nrole: Senior Reviewer\ntriggers:\n  - review\n";
        let diags = AgentsValidator.validate(Path::new(".looprs/agents/reviewer.yaml"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn agent_missing_name_errors() {
        let src = "role: Reviewer\n";
        let diags = AgentsValidator.validate(Path::new(".looprs/agents/x.yaml"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-name")));
    }

    // --- rules ---

    #[test]
    fn valid_rule_is_clean() {
        let src = "# Security\n\nDon't commit secrets.\n";
        let diags = RulesValidator.validate(Path::new(".looprs/rules/security.md"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn rule_empty_errors() {
        let diags = RulesValidator.validate(Path::new(".looprs/rules/empty.md"), "");
        assert!(diags.iter().any(|d| d.rule.contains("empty")));
    }

    #[test]
    fn rule_no_heading_warns() {
        let src = "Just text, no heading.\n";
        let diags = RulesValidator.validate(Path::new(".looprs/rules/x.md"), src);
        assert!(diags.iter().any(|d| d.rule.contains("no-heading")));
    }

    #[test]
    fn rule_readme_skipped() {
        let diags = RulesValidator.validate(Path::new(".looprs/rules/README.md"), "");
        assert!(diags.is_empty());
    }

    // --- config.json ---

    #[test]
    fn valid_config_is_clean() {
        let diags = ConfigValidator.validate(Path::new(".looprs/config.json"), "{}");
        assert!(diags.is_empty());
    }

    #[test]
    fn config_invalid_json_errors() {
        let diags = ConfigValidator.validate(Path::new(".looprs/config.json"), "{bad");
        assert!(diags.iter().any(|d| d.rule.contains("invalid-json")));
    }

    // --- agent.json ---

    #[test]
    fn valid_agent_json_is_clean() {
        let src = r#"{"version": "1.0", "agent_schema": {}}"#;
        let diags = AgentJsonValidator.validate(Path::new("looprs.agent.json"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn agent_json_missing_version_warns() {
        let src = r#"{"agent_schema": {}}"#;
        let diags = AgentJsonValidator.validate(Path::new("looprs.agent.json"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-version")));
    }

    #[test]
    fn agent_json_missing_schema_warns() {
        let src = r#"{"version": "1.0"}"#;
        let diags = AgentJsonValidator.validate(Path::new("looprs.agent.json"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-schema")));
    }

    #[test]
    fn agent_json_invalid_errors() {
        let diags = AgentJsonValidator.validate(Path::new("x.agent.json"), "{bad");
        assert!(diags.iter().any(|d| d.rule.contains("invalid-json")));
    }
}
