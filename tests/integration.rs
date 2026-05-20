//! End-to-end integration tests: temp directory with multi-platform files,
//! dispatched through `agentlint_core::run()` with the full validator set.

use std::fs;
use std::path::PathBuf;

use agentlint_core::{RunConfig, Validator, run};
use tempfile::TempDir;

/// Assemble the same native validators as main.rs (minus docs and plugins).
fn validators() -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(agentlint_claude::ClaudeValidator),
        Box::new(agentlint_cursor::CursorValidator),
        Box::new(agentlint_codex::CodexValidator),
        Box::new(agentlint_opencode::AgentsMarkdownValidator),
        Box::new(agentlint_opencode::OpenCodeJsonValidator),
        Box::new(agentlint_gemini::GeminiValidator),
        Box::new(agentlint_pi::PiValidator),
        Box::new(agentlint_looprs::CommandsValidator),
        Box::new(agentlint_looprs::HooksValidator),
        Box::new(agentlint_looprs::SkillsValidator),
        Box::new(agentlint_looprs::AgentsValidator),
        Box::new(agentlint_looprs::RulesValidator),
        Box::new(agentlint_looprs::ConfigValidator),
        Box::new(agentlint_looprs::AgentJsonValidator),
    ]
}

/// Substantive markdown content that passes the min-lines / min-chars / heading
/// checks used by codex, gemini, pi, and opencode validators.
const VALID_MD: &str = "\
# Agent Instructions

This document provides guidance for the AI coding agent.

## Rules

- Follow the coding standards defined in the project.
- Run tests before committing changes.
- Use descriptive commit messages.

## Architecture

The project uses a hexagonal architecture with ports and adapters.
";

/// Valid Claude agent frontmatter file.
const VALID_CLAUDE_AGENT: &str = "\
---
name: test-agent
description: A test agent for integration testing
---

Do something useful.
";

/// Valid Cursor rule file (.mdc with frontmatter).
const VALID_CURSOR_RULE: &str = "\
---
name: test-rule
description: A test cursor rule
---

Follow these rules when editing code.
";

/// Valid opencode.json content.
const VALID_OPENCODE_JSON: &str = r#"{"model": "gpt-4"}"#;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn all_valid_files_produce_zero_diagnostics() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Claude agent
    let agent_dir = root.join(".claude/agents");
    fs::create_dir_all(&agent_dir).unwrap();
    fs::write(agent_dir.join("helper.md"), VALID_CLAUDE_AGENT).unwrap();

    // Cursor rule
    let cursor_dir = root.join(".cursor/rules");
    fs::create_dir_all(&cursor_dir).unwrap();
    fs::write(cursor_dir.join("style.mdc"), VALID_CURSOR_RULE).unwrap();

    // AGENTS.md (codex + opencode + pi)
    fs::write(root.join("AGENTS.md"), VALID_MD).unwrap();

    // GEMINI.md
    fs::write(root.join("GEMINI.md"), VALID_MD).unwrap();

    // SYSTEM.md (pi)
    fs::write(root.join("SYSTEM.md"), VALID_MD).unwrap();

    // opencode.json
    fs::write(root.join("opencode.json"), VALID_OPENCODE_JSON).unwrap();

    let validators = validators();
    let config = RunConfig::default();
    let result = run(&[PathBuf::from(root)], &validators, &config);

    assert!(
        result.diagnostics.is_empty(),
        "expected zero diagnostics for valid files, got: {:#?}",
        result.diagnostics
    );
    assert!(
        result.files_checked > 0,
        "should have checked at least one file"
    );
}

#[test]
fn empty_agents_md_produces_errors_from_multiple_validators() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    fs::write(root.join("AGENTS.md"), "").unwrap();

    let validators = validators();
    let config = RunConfig::default();
    let result = run(&[PathBuf::from(root)], &validators, &config);

    // codex, opencode, and pi all claim AGENTS.md and reject empty content.
    let rules: Vec<&str> = result
        .diagnostics
        .iter()
        .filter_map(|d| d.rule.as_deref())
        .collect();

    assert!(
        rules.iter().any(|r| r.starts_with("codex/")),
        "expected codex diagnostic, got rules: {rules:?}"
    );
    assert!(
        rules.iter().any(|r| r.starts_with("opencode/")),
        "expected opencode diagnostic, got rules: {rules:?}"
    );
    assert!(
        rules.iter().any(|r| r.starts_with("pi/")),
        "expected pi diagnostic, got rules: {rules:?}"
    );
}

#[test]
fn binary_file_in_matched_path_is_skipped() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    let agent_dir = root.join(".claude/agents");
    fs::create_dir_all(&agent_dir).unwrap();
    // Write non-UTF-8 bytes — the runner should skip this file.
    fs::write(agent_dir.join("binary.md"), &[0xFF, 0xFE, 0x00, 0x01, 0x80]).unwrap();

    let validators = validators();
    let config = RunConfig::default();
    let result = run(&[PathBuf::from(root)], &validators, &config);

    // Binary file should either be skipped entirely or produce a read-error
    // diagnostic — but NOT a validator panic.
    let has_validator_panic = result
        .diagnostics
        .iter()
        .any(|d| d.message.contains("panic"));
    assert!(!has_validator_panic, "binary file caused a panic");
}

#[test]
fn unrecognised_file_is_ignored() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    fs::write(root.join("random.txt"), "nothing to see here").unwrap();
    fs::write(root.join("notes.log"), "some log output").unwrap();

    let validators = validators();
    let config = RunConfig::default();
    let result = run(&[PathBuf::from(root)], &validators, &config);

    assert!(
        result.diagnostics.is_empty(),
        "unrecognised files should produce no diagnostics, got: {:#?}",
        result.diagnostics
    );
    assert_eq!(
        result.files_checked, 0,
        "no matched files should be checked"
    );
}
