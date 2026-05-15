use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

mod frontmatter;
mod macros;

pub mod agents;
pub mod commands;
pub mod hooks;
pub mod settings;
pub mod skills;

/// Aggregate validator — dispatches to the correct sub-module based on path.
pub struct ClaudeValidator;

impl Validator for ClaudeValidator {
    fn patterns(&self) -> &[&str] {
        &[
            ".claude/agents/**/*.md",
            ".claude/skills/**/*.md",
            ".claude/commands/**/*.md",
            ".claude/hooks/*",
            ".claude/settings.json",
            ".claude/settings.local.json",
        ]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        let path_str = path.to_string_lossy();

        if path_str.contains(".claude/agents/") {
            agents::AgentsValidator::validate(path, src)
        } else if path_str.contains(".claude/skills/") {
            skills::SkillsValidator::validate(path, src)
        } else if path_str.contains(".claude/commands/") {
            commands::CommandsValidator::validate(path, src)
        } else if path_str.contains(".claude/hooks/") {
            hooks::HooksValidator::validate(path, src)
        } else if path_str.ends_with("settings.json") || path_str.ends_with("settings.local.json") {
            settings::SettingsValidator::validate(path, src)
        } else {
            vec![]
        }
    }
}
