//! Exercises the built-in declarative plugin definitions end to end.

use super::*;
use crate::Validator;
use std::path::Path;

const LOOPRS_TOML: &str = include_str!("../../../../plugins/agentlint.looprs.toml");

#[test]
fn looprs_toml_parses() {
    let plugin = load_plugin_str(LOOPRS_TOML).unwrap();
    assert_eq!(plugin.plugin.name, "looprs");
    assert_eq!(plugin.plugin.prefix, "looprs");
    assert!(!plugin.validator.is_empty());
}

#[test]
fn looprs_creates_validators() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    assert!(validators.len() >= 7);
}

// --- Commands ---

#[test]
fn commands_empty_errors() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("commands")))
        .unwrap();
    let diags = v.validate(Path::new(".looprs/commands/x.yaml"), "");
    assert!(diags.iter().any(|d| d.rule.contains("empty")));
}

#[test]
fn commands_missing_name_errors() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("commands")))
        .unwrap();
    let src = "description: Run linter\naction:\n  type: shell\n  command: cargo clippy\n";
    let diags = v.validate(Path::new(".looprs/commands/lint.yaml"), src);
    assert!(diags.iter().any(|d| d.rule.contains("missing-name")));
}

#[test]
fn commands_valid_is_clean() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("commands")))
        .unwrap();
    let src =
        "name: lint\ndescription: Run linter\naction:\n  type: shell\n  command: cargo clippy\n";
    let diags = v.validate(Path::new(".looprs/commands/lint.yaml"), src);
    assert!(diags.is_empty(), "unexpected: {diags:?}");
}

#[test]
fn commands_invalid_action_type_errors() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("commands")))
        .unwrap();
    let src = "name: x\ndescription: y\naction:\n  type: unknown\n";
    let diags = v.validate(Path::new(".looprs/commands/x.yaml"), src);
    assert!(
        diags.iter().any(|d| d.rule.contains("invalid-action-type")),
        "expected invalid-action-type, got: {diags:?}"
    );
}

// --- Hooks ---

#[test]
fn hooks_valid_is_clean() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("hooks")))
        .unwrap();
    let src = "name: start\ntrigger: SessionStart\nactions:\n  - type: message\n    text: hi\n";
    let diags = v.validate(Path::new(".looprs/hooks/start.yaml"), src);
    assert!(diags.is_empty(), "unexpected: {diags:?}");
}

#[test]
fn hooks_unknown_trigger_warns() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("hooks")))
        .unwrap();
    let src = "name: start\ntrigger: FakeEvent\nactions:\n  - type: message\n    text: hi\n";
    let diags = v.validate(Path::new(".looprs/hooks/start.yaml"), src);
    assert!(
        diags.iter().any(|d| d.rule.contains("unknown-trigger")),
        "expected unknown-trigger, got: {diags:?}"
    );
}

#[test]
fn hooks_empty_actions_errors() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("hooks")))
        .unwrap();
    let src = "name: start\ntrigger: SessionStart\nactions: []\n";
    let diags = v.validate(Path::new(".looprs/hooks/start.yaml"), src);
    assert!(diags.iter().any(|d| d.rule.contains("empty-actions")));
}

// --- Skills ---

#[test]
fn skills_valid_is_clean() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("skills")))
        .unwrap();
    let src = "---\nname: test-skill\ntriggers:\n  - test\n---\n# Content\n";
    let diags = v.validate(Path::new(".looprs/skills/test/SKILL.md"), src);
    assert!(diags.is_empty(), "unexpected: {diags:?}");
}

#[test]
fn skills_missing_frontmatter_errors() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("skills")))
        .unwrap();
    let src = "# Just content\nNo frontmatter.\n";
    let diags = v.validate(Path::new(".looprs/skills/test/SKILL.md"), src);
    assert!(diags.iter().any(|d| d.rule.contains("missing-frontmatter")));
}

// --- Rules (markdown) ---

#[test]
fn rules_valid_is_clean() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("rules")))
        .unwrap();
    let src = "# Security\n\nDon't commit secrets.\n";
    let diags = v.validate(Path::new(".looprs/rules/security.md"), src);
    assert!(diags.is_empty(), "unexpected: {diags:?}");
}

#[test]
fn rules_readme_skipped() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("rules")))
        .unwrap();
    let diags = v.validate(Path::new(".looprs/rules/README.md"), "");
    assert!(diags.is_empty());
}

// --- Agent JSON ---

#[test]
fn agent_json_valid_is_clean() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("agent.json")))
        .unwrap();
    let src = r#"{"version": "1.0", "agent_schema": {}}"#;
    let diags = v.validate(Path::new("looprs.agent.json"), src);
    assert!(diags.is_empty(), "unexpected: {diags:?}");
}

#[test]
fn agent_json_missing_version_warns() {
    let validators = validators_from_str(LOOPRS_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("agent.json")))
        .unwrap();
    let src = r#"{"agent_schema": {}}"#;
    let diags = v.validate(Path::new("looprs.agent.json"), src);
    assert!(diags.iter().any(|d| d.rule.contains("missing-version")));
}

// Claude plugin tests

const CLAUDE_TOML: &str = include_str!("../../../../plugins/agentlint.claude.toml");

#[test]
fn claude_toml_parses() {
    let plugin = load_plugin_str(CLAUDE_TOML).unwrap();
    assert_eq!(plugin.plugin.name, "claude");
    assert_eq!(plugin.plugin.prefix, "claude");
    assert!(plugin.validator.len() >= 7);
}

#[test]
fn claude_agents_valid_is_clean() {
    let validators = validators_from_str(CLAUDE_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("agents")))
        .unwrap();
    let src = "---\nname: my-agent\ndescription: does things well enough here\n---\nbody\n";
    let diags = v.validate(Path::new(".claude/agents/test.md"), src);
    assert!(diags.is_empty(), "unexpected: {diags:?}");
}

#[test]
fn claude_agents_missing_name_errors() {
    let validators = validators_from_str(CLAUDE_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("agents")))
        .unwrap();
    let src = "---\ndescription: does things well enough here\n---\n";
    let diags = v.validate(Path::new(".claude/agents/test.md"), src);
    assert!(diags.iter().any(|d| d.rule.contains("missing-name")));
}

#[test]
fn claude_agents_name_collision_errors() {
    let validators = validators_from_str(CLAUDE_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("agents")))
        .unwrap();
    let src = "---\nname: Bash\ndescription: does things well enough here\n---\n";
    let diags = v.validate(Path::new(".claude/agents/test.md"), src);
    assert!(
        diags.iter().any(|d| d.rule.contains("name-collision")),
        "expected name-collision, got: {diags:?}"
    );
}

#[test]
fn claude_agents_description_too_short_warns() {
    let validators = validators_from_str(CLAUDE_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("agents")))
        .unwrap();
    let src = "---\nname: my-agent\ndescription: short\n---\n";
    let diags = v.validate(Path::new(".claude/agents/test.md"), src);
    assert!(
        diags
            .iter()
            .any(|d| d.rule.contains("description-too-short")),
        "expected description-too-short, got: {diags:?}"
    );
}

#[test]
fn claude_hooks_shebang_check() {
    let validators = validators_from_str(CLAUDE_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("hooks")))
        .unwrap();
    let diags = v.validate(Path::new(".claude/hooks/my-hook"), "echo hello\n");
    assert!(diags.iter().any(|d| d.rule.contains("missing-shebang")));
}

#[test]
fn claude_hooks_shebang_present_is_clean() {
    let validators = validators_from_str(CLAUDE_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("hooks")))
        .unwrap();
    let diags = v.validate(
        Path::new(".claude/hooks/my-hook"),
        "#!/usr/bin/env nu\necho hi\n",
    );
    assert!(diags.is_empty(), "unexpected: {diags:?}");
}

#[test]
fn claude_settings_unknown_key_errors() {
    let validators = validators_from_str(CLAUDE_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("settings")))
        .unwrap();
    let diags = v.validate(Path::new(".claude/settings.json"), r#"{"theme": "dark"}"#);
    assert!(
        diags.iter().any(|d| d.rule.contains("unknown-key")),
        "expected unknown-key, got: {diags:?}"
    );
}

#[test]
fn claude_settings_valid_is_clean() {
    let validators = validators_from_str(CLAUDE_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("settings")))
        .unwrap();
    let diags = v.validate(
        Path::new(".claude/settings.json"),
        r#"{"permissions": {}, "model": "sonnet"}"#,
    );
    assert!(diags.is_empty(), "unexpected: {diags:?}");
}

#[test]
fn claude_meta_no_heading_warns() {
    let validators = validators_from_str(CLAUDE_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| *p == "CLAUDE.md"))
        .unwrap();
    let diags = v.validate(Path::new("CLAUDE.md"), "Just text, no heading.\n");
    assert!(
        diags
            .iter()
            .any(|d| d.rule.contains("claude-md-no-heading")),
        "expected no-heading, got: {diags:?}"
    );
}

#[test]
fn claude_meta_too_long_warns() {
    let validators = validators_from_str(CLAUDE_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| *p == "CLAUDE.md"))
        .unwrap();
    let src = "# Title\n".to_string() + &"line\n".repeat(501);
    let diags = v.validate(Path::new("CLAUDE.md"), &src);
    assert!(
        diags.iter().any(|d| d.rule.contains("claude-md-too-long")),
        "expected too-long, got: {diags:?}"
    );
}

#[test]
fn claude_skills_name_too_long_errors() {
    let validators = validators_from_str(CLAUDE_TOML).unwrap();
    let v = validators
        .iter()
        .find(|v| v.patterns().iter().any(|p| p.contains("skills")))
        .unwrap();
    let long = "a".repeat(65);
    let src = format!("---\nname: {long}\ndescription: ok\n---\n");
    let diags = v.validate(Path::new(".claude/skills/test/SKILL.md"), &src);
    assert!(
        diags.iter().any(|d| d.rule.contains("name-too-long")),
        "expected name-too-long, got: {diags:?}"
    );
}
