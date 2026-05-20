//! Conformance tests: verify every built-in Validator satisfies the trait
//! contract. One central location instead of per-crate boilerplate.

use agentlint_core::Validator;
use agentlint_core::testing::{assert_validator_contract, assert_validator_rejects_empty};

/// Build the full list of native validators (mirrors main.rs assembly).
fn all_validators() -> Vec<(&'static str, Box<dyn Validator>)> {
    let mut v: Vec<(&str, Box<dyn Validator>)> = vec![
        ("claude", Box::new(agentlint_claude::ClaudeValidator)),
        ("cursor", Box::new(agentlint_cursor::CursorValidator)),
        ("codex", Box::new(agentlint_codex::CodexValidator)),
        (
            "opencode/agents",
            Box::new(agentlint_opencode::AgentsMarkdownValidator),
        ),
        (
            "opencode/json",
            Box::new(agentlint_opencode::OpenCodeJsonValidator),
        ),
        ("gemini", Box::new(agentlint_gemini::GeminiValidator)),
        ("pi", Box::new(agentlint_pi::PiValidator)),
        (
            "looprs/commands",
            Box::new(agentlint_looprs::CommandsValidator),
        ),
        ("looprs/hooks", Box::new(agentlint_looprs::HooksValidator)),
        ("looprs/skills", Box::new(agentlint_looprs::SkillsValidator)),
        ("looprs/agents", Box::new(agentlint_looprs::AgentsValidator)),
        ("looprs/rules", Box::new(agentlint_looprs::RulesValidator)),
        ("looprs/config", Box::new(agentlint_looprs::ConfigValidator)),
        (
            "looprs/agent-json",
            Box::new(agentlint_looprs::AgentJsonValidator),
        ),
    ];

    // Declarative (TOML-based) validators.
    for dv in agentlint_plugins::all_validators() {
        v.push(("plugin", dv));
    }

    v
}

#[test]
fn all_validators_satisfy_contract() {
    for (label, validator) in all_validators() {
        // Wrap panics so we know which validator failed.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_validator_contract(validator.as_ref());
        }));
        if let Err(e) = result {
            let msg = e
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| e.downcast_ref::<&str>().copied())
                .unwrap_or("(unknown panic)");
            panic!("conformance failed for {label}: {msg}");
        }
    }
}

/// Validators that must reject empty files.
#[test]
fn rejects_empty_files() {
    let cases: Vec<(&str, Box<dyn Validator>)> = vec![
        ("claude", Box::new(agentlint_claude::ClaudeValidator)),
        ("codex", Box::new(agentlint_codex::CodexValidator)),
        ("gemini", Box::new(agentlint_gemini::GeminiValidator)),
        ("pi", Box::new(agentlint_pi::PiValidator)),
        (
            "opencode/agents",
            Box::new(agentlint_opencode::AgentsMarkdownValidator),
        ),
    ];

    let filenames: &[(&str, &str)] = &[
        ("claude", ".claude/agents/test.md"),
        ("codex", "AGENTS.md"),
        ("gemini", "GEMINI.md"),
        ("pi", "AGENTS.md"),
        ("opencode/agents", "AGENTS.md"),
    ];

    for (label, validator) in &cases {
        let filename = filenames
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, f)| *f)
            .unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_validator_rejects_empty(validator.as_ref(), filename);
        }));
        if let Err(e) = result {
            let msg = e
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| e.downcast_ref::<&str>().copied())
                .unwrap_or("(unknown panic)");
            panic!("rejects_empty failed for {label}: {msg}");
        }
    }
}
