use agentlint_core::{Diagnostic, Validator};
use std::path::{Component, Path};

mod frontmatter;
mod macros;

// ---------------------------------------------------------------------------
// File-kind classifier
// ---------------------------------------------------------------------------

enum ClaudeFileKind {
    Agent,
    Skill,
    Command,
    Hook,
    Settings,
}

/// Classify a path into its Claude harness file kind by walking components.
///
/// Expects paths of the form `.claude/<kind>/...` or `.claude/settings.json`.
/// Returns `None` for paths that don't match any known shape, preventing false
/// positives from substring matches (e.g. `.claude/agents-backup/` would not
/// match `Agent` here).
fn claude_file_kind(path: &Path) -> Option<ClaudeFileKind> {
    let mut components = path.components().peekable();

    // Consume leading `.` or `./` if present.
    if components
        .peek()
        .is_some_and(|c| matches!(c, Component::CurDir))
    {
        components.next();
    }

    // Find the `.claude` component anywhere in the path (handles absolute or
    // relative paths with a prefix).
    let comps: Vec<_> = components.collect();
    let claude_pos = comps.iter().position(|c| c.as_os_str() == ".claude")?;

    match comps.get(claude_pos + 1)?.as_os_str().to_str()? {
        "agents" => Some(ClaudeFileKind::Agent),
        "skills" => Some(ClaudeFileKind::Skill),
        "commands" => Some(ClaudeFileKind::Command),
        "hooks" => Some(ClaudeFileKind::Hook),
        "settings.json" | "settings.local.json" => Some(ClaudeFileKind::Settings),
        _ => None,
    }
}

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
        match claude_file_kind(path) {
            Some(ClaudeFileKind::Agent) => agents::AgentsValidator::validate(path, src),
            Some(ClaudeFileKind::Skill) => skills::SkillsValidator::validate(path, src),
            Some(ClaudeFileKind::Command) => commands::CommandsValidator::validate(path, src),
            Some(ClaudeFileKind::Hook) => hooks::HooksValidator::validate(path, src),
            Some(ClaudeFileKind::Settings) => settings::SettingsValidator::validate(path, src),
            None => vec![],
        }
    }
}
