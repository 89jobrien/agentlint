//! Conformance tests: verify every built-in Validator satisfies the trait
//! contract. One central location instead of per-crate boilerplate.

use agentlint_core::Validator;
use agentlint_core::behavioral::{claude, cursor, looprs};
use agentlint_core::testing::{assert_validator_contract, assert_validator_rejects_empty};

/// Build the full list of native validators (mirrors main.rs assembly).
fn all_validators() -> Vec<(&'static str, Box<dyn Validator>)> {
    let mut v: Vec<(&str, Box<dyn Validator>)> = vec![
        ("claude", Box::new(claude::ClaudeValidator)),
        ("cursor", Box::new(cursor::CursorValidator)),
        ("looprs/commands", Box::new(looprs::CommandsValidator)),
        ("looprs/hooks", Box::new(looprs::HooksValidator)),
        ("looprs/skills", Box::new(looprs::SkillsValidator)),
        ("looprs/agents", Box::new(looprs::AgentsValidator)),
        ("looprs/rules", Box::new(looprs::RulesValidator)),
        ("looprs/config", Box::new(looprs::ConfigValidator)),
        ("looprs/agent-json", Box::new(looprs::AgentJsonValidator)),
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
    let cases: Vec<(&str, Box<dyn Validator>)> =
        vec![("claude", Box::new(claude::ClaudeValidator))];

    let filenames: &[(&str, &str)] = &[("claude", ".claude/agents/test.md")];

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
