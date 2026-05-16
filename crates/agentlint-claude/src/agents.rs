use agentlint_core::{Diagnostic, Difficulty};
use agentlint_frontmatter::{FieldRule, FrontmatterValidator, parse};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Built-in Claude Code tool names (lower-case for case-insensitive comparison).
const BUILTIN_TOOLS: &[&str] = &[
    "bash",
    "edit",
    "read",
    "glob",
    "write",
    "grep",
    "webfetch",
    "websearch",
    "agent",
    "task",
    "notebook",
];

pub struct AgentsValidator;

impl AgentsValidator {
    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        static VALIDATOR: OnceLock<FrontmatterValidator> = OnceLock::new();
        let mut diags = VALIDATOR
            .get_or_init(|| {
                FrontmatterValidator::builder()
                    .required(FieldRule::new("name"))
                    .required(FieldRule::new("description"))
                    .build()
            })
            .validate(path, src);

        // Additional per-field rules (#34, #42).
        if let Ok(fields) = parse(src) {
            for field in &fields {
                match field.key.as_str() {
                    "name" => {
                        let lower = field.value.to_lowercase();
                        if BUILTIN_TOOLS.contains(&lower.as_str()) {
                            diags.push(
                                Diagnostic::error(
                                    path,
                                    field.line,
                                    1,
                                    format!(
                                        "agent name '{}' collides with a built-in Claude Code \
                                         tool name",
                                        field.value
                                    ),
                                )
                                .with_rule("claude/agents/name-collision", Difficulty::Hard),
                            );
                        }
                    }
                    "description" if field.value.len() < 20 => {
                        diags.push(
                            Diagnostic::warning(
                                path,
                                field.line,
                                1,
                                format!(
                                    "agent description is too short ({} chars, minimum 20)",
                                    field.value.len()
                                ),
                            )
                            .with_rule("claude/agents/description-too-short", Difficulty::Painful),
                        );
                    }
                    _ => {}
                }
            }
        }

        diags
    }
}

/// Cross-file duplicate-name check. Given a slice of `(path, src)` pairs for
/// agent files, returns a `claude/agents/duplicate-name` warning for every
/// file whose `name:` frontmatter value appears in more than one file.
pub fn check_duplicate_names<'a>(files: &[(&'a Path, &'a str)]) -> Vec<Diagnostic> {
    // Collect name → vec of (path, line) from each file that has a parseable name.
    let mut name_to_files: HashMap<String, Vec<(&Path, usize)>> = HashMap::new();

    for (path, src) in files {
        if let Ok(fields) = parse(src) {
            for field in &fields {
                if field.key == "name" && !field.value.is_empty() {
                    name_to_files
                        .entry(field.value.clone())
                        .or_default()
                        .push((path, field.line));
                    break; // only the first `name:` field matters
                }
            }
        }
    }

    let mut diags = Vec::new();
    for (name, occurrences) in &name_to_files {
        if occurrences.len() > 1 {
            for (path, line) in occurrences {
                diags.push(
                    Diagnostic::warning(
                        *path,
                        *line,
                        1,
                        format!(
                            "agent name '{name}' is defined in multiple files; \
                             the last loaded definition silently shadows the others"
                        ),
                    )
                    .with_rule("claude/agents/duplicate-name", Difficulty::Hard),
                );
            }
        }
    }
    diags
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentlint_core::testing::{assert_clean, assert_error_at, assert_error_contains};
    use agentlint_core::{Difficulty, Severity};
    use std::path::Path;

    const PATH: &str = ".claude/agents/test.md";

    #[test]
    fn valid_agent_no_diagnostics() {
        let src = "---\nname: my-agent\ndescription: does things well here\n---\nbody\n";
        assert_clean(&AgentsValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn missing_name_is_error() {
        let src = "---\ndescription: does things well here\n---\n";
        assert_error_contains(
            &AgentsValidator::validate(Path::new(PATH), src),
            "missing required field 'name'",
        );
    }

    #[test]
    fn missing_description_is_error() {
        let src = "---\nname: my-agent\n---\n";
        assert_error_contains(
            &AgentsValidator::validate(Path::new(PATH), src),
            "missing required field 'description'",
        );
    }

    #[test]
    fn empty_name_is_error_at_correct_line() {
        let src = "---\nname: \ndescription: a long enough description\n---\n";
        assert_error_at(
            &AgentsValidator::validate(Path::new(PATH), src),
            2,
            "'name'",
        );
    }

    #[test]
    fn optional_fields_do_not_cause_errors() {
        let src = "---\nname: my-agent\ndescription: a long enough description here\ntools: [Bash]\nmodel: claude-opus-4-6\n---\n";
        assert_clean(&AgentsValidator::validate(Path::new(PATH), src));
    }

    // ---- #42: description-too-short ----

    #[test]
    fn description_too_short_emits_warning() {
        let src = "---\nname: my-agent\ndescription: short\n---\n";
        let diags = AgentsValidator::validate(Path::new(PATH), src);
        let hit = diags
            .iter()
            .find(|d| d.rule == "claude/agents/description-too-short");
        assert!(hit.is_some(), "expected description-too-short warning");
        let d = hit.unwrap();
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.difficulty, Difficulty::Painful);
    }

    #[test]
    fn description_exactly_20_chars_is_clean() {
        // 20 chars = "12345678901234567890"
        let src = "---\nname: my-agent\ndescription: 12345678901234567890\n---\n";
        let diags = AgentsValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .all(|d| d.rule != "claude/agents/description-too-short"),
            "20-char description should not trigger warning"
        );
    }

    #[test]
    fn description_19_chars_triggers_warning() {
        let src = "---\nname: my-agent\ndescription: 1234567890123456789\n---\n";
        let diags = AgentsValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "claude/agents/description-too-short"),
            "19-char description should trigger warning"
        );
    }

    // ---- #34: name-collision ----

    #[test]
    fn name_collision_with_builtin_emits_error() {
        let src = "---\nname: bash\ndescription: a long enough description here\n---\n";
        let diags = AgentsValidator::validate(Path::new(PATH), src);
        let hit = diags
            .iter()
            .find(|d| d.rule == "claude/agents/name-collision");
        assert!(hit.is_some(), "expected name-collision error");
        let d = hit.unwrap();
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.difficulty, Difficulty::Hard);
    }

    #[test]
    fn name_collision_is_case_insensitive() {
        let src = "---\nname: Bash\ndescription: a long enough description here\n---\n";
        let diags = AgentsValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "claude/agents/name-collision"),
            "name collision check must be case-insensitive"
        );
    }

    #[test]
    fn non_builtin_name_is_clean() {
        let src = "---\nname: my-custom-agent\ndescription: a long enough description here\n---\n";
        let diags = AgentsValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .all(|d| d.rule != "claude/agents/name-collision"),
            "non-builtin name should not trigger name-collision"
        );
    }

    // ---- #35: duplicate-name ----

    #[test]
    fn duplicate_agent_names_emit_warnings() {
        let src_a = "---\nname: my-agent\ndescription: a long enough description here\n---\n";
        let src_b = "---\nname: my-agent\ndescription: another long enough description\n---\n";
        let files = vec![
            (Path::new(".claude/agents/a.md"), src_a),
            (Path::new(".claude/agents/b.md"), src_b),
        ];
        let diags = check_duplicate_names(&files);
        assert_eq!(
            diags
                .iter()
                .filter(|d| d.rule == "claude/agents/duplicate-name")
                .count(),
            2,
            "expected one warning per file with a duplicate name"
        );
        assert!(
            diags.iter().all(|d| d.severity == Severity::Warning),
            "duplicate-name should be a warning"
        );
        assert!(
            diags.iter().all(|d| d.difficulty == Difficulty::Hard),
            "duplicate-name should be Difficulty::Hard"
        );
    }

    #[test]
    fn distinct_agent_names_no_duplicate_warning() {
        let src_a = "---\nname: agent-one\ndescription: a long enough description here\n---\n";
        let src_b = "---\nname: agent-two\ndescription: another long enough description\n---\n";
        let files = vec![
            (Path::new(".claude/agents/a.md"), src_a),
            (Path::new(".claude/agents/b.md"), src_b),
        ];
        let diags = check_duplicate_names(&files);
        assert!(
            diags
                .iter()
                .all(|d| d.rule != "claude/agents/duplicate-name"),
            "distinct names should produce no duplicate-name warnings"
        );
    }

    #[test]
    fn all_builtin_names_trigger_collision() {
        for tool in BUILTIN_TOOLS {
            let src =
                format!("---\nname: {tool}\ndescription: a long enough description here\n---\n");
            let diags = AgentsValidator::validate(Path::new(PATH), &src);
            assert!(
                diags
                    .iter()
                    .any(|d| d.rule == "claude/agents/name-collision"),
                "expected collision for builtin tool '{tool}'"
            );
        }
    }
}
