use crate::frontmatter::{self, Field};
use agentlint_core::{Diagnostic, Difficulty};
use std::path::Path;

pub struct SkillsValidator;

impl SkillsValidator {
    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        // Only the SKILL.md entrypoint carries required frontmatter.
        // Supporting files (references/*, examples/*, scripts/*, etc.) are
        // freeform and must not be flagged for missing frontmatter.
        if path.file_name().and_then(|n| n.to_str()) != Some("SKILL.md") {
            return vec![];
        }

        let fields = match frontmatter::parse(src) {
            Ok(f) => f,
            Err(frontmatter::ParseError::NoFence) => {
                return vec![
                    Diagnostic::error(
                        path,
                        1,
                        1,
                        "missing frontmatter: file must start with '---'",
                    )
                    .with_rule("claude/skills/missing-frontmatter", Difficulty::Easy),
                ];
            }
            Err(frontmatter::ParseError::UnclosedFence) => {
                return vec![
                    Diagnostic::error(
                        path,
                        1,
                        1,
                        "unclosed frontmatter fence: missing closing '---'",
                    )
                    .with_rule("claude/skills/missing-frontmatter", Difficulty::Easy),
                ];
            }
        };

        let mut diagnostics = Vec::new();

        validate_name(path, &fields, &mut diagnostics);
        validate_description(path, &fields, &mut diagnostics);

        diagnostics
    }
}

// ---------------------------------------------------------------------------
// Field validators
// ---------------------------------------------------------------------------

fn validate_name(path: &Path, fields: &[Field], diagnostics: &mut Vec<Diagnostic>) {
    let field = match fields.iter().find(|f| f.key == "name") {
        None => {
            diagnostics.push(
                Diagnostic::error(path, 1, 1, "missing required field 'name'")
                    .with_rule("claude/skills/missing-name", Difficulty::Easy),
            );
            return;
        }
        Some(f) if f.value.is_empty() => {
            diagnostics.push(
                Diagnostic::error(path, f.line, 1, "required field 'name' must not be empty")
                    .with_rule("claude/skills/missing-name", Difficulty::Easy),
            );
            return;
        }
        Some(f) => f,
    };

    let name = &field.value;

    if name.len() > 64 {
        diagnostics.push(
            Diagnostic::error(
                path,
                field.line,
                1,
                format!("'name' exceeds 64 characters (got {})", name.len()),
            )
            .with_rule("claude/skills/invalid-name", Difficulty::Easy),
        );
    }

    // Names may optionally be namespaced: `namespace:slug`.
    // Each segment must contain only lowercase letters, digits, and hyphens.
    // At most one colon is permitted (no nested namespaces).
    let (namespace, slug) = match name.splitn(3, ':').collect::<Vec<_>>().as_slice() {
        [slug] => (None, *slug),
        [ns, slug] => (Some(*ns), *slug),
        _ => {
            diagnostics.push(
                Diagnostic::error(
                    path,
                    field.line,
                    1,
                    "'name' must have at most one namespace separator ':'",
                )
                .with_rule("claude/skills/invalid-name", Difficulty::Easy),
            );
            return;
        }
    };

    let valid_segment = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };

    if let Some(ns) = namespace {
        if !valid_segment(ns) {
            diagnostics.push(
                Diagnostic::error(
                    path,
                    field.line,
                    1,
                    "namespace part of 'name' must contain only lowercase letters, digits, and hyphens",
                )
                .with_rule("claude/skills/invalid-name", Difficulty::Easy),
            );
        }
    }

    if !valid_segment(slug) {
        diagnostics.push(
            Diagnostic::error(
                path,
                field.line,
                1,
                "slug part of 'name' must contain only lowercase letters, digits, and hyphens",
            )
            .with_rule("claude/skills/invalid-name", Difficulty::Easy),
        );
    }

    if slug.starts_with('-') || slug.ends_with('-') {
        diagnostics.push(
            Diagnostic::error(
                path,
                field.line,
                1,
                "'name' slug must not start or end with a hyphen",
            )
            .with_rule("claude/skills/invalid-name", Difficulty::Easy),
        );
    }

    if slug.contains("--") {
        diagnostics.push(
            Diagnostic::error(
                path,
                field.line,
                1,
                "'name' slug must not contain consecutive hyphens",
            )
            .with_rule("claude/skills/invalid-name", Difficulty::Easy),
        );
    }

    // The slug must match the skill's directory name (agentskills.io spec).
    // For `namespace:slug`, only the slug is checked against the dir name.
    // path is `.claude/skills/<skill-name>/SKILL.md`; parent() gives the skill dir.
    if let Some(dir_name) = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        && slug != dir_name
    {
        diagnostics.push(
            Diagnostic::error(
                path,
                field.line,
                1,
                format!("'name' ({name}) must match the skill directory name ({dir_name})"),
            )
            .with_rule("claude/skills/invalid-name", Difficulty::Easy),
        );
    }
}

fn validate_description(path: &Path, fields: &[Field], diagnostics: &mut Vec<Diagnostic>) {
    match fields.iter().find(|f| f.key == "description") {
        None => {
            diagnostics.push(
                Diagnostic::error(path, 1, 1, "missing required field 'description'")
                    .with_rule("claude/skills/missing-description", Difficulty::Easy),
            );
        }
        Some(f) if f.value.is_empty() => {
            diagnostics.push(
                Diagnostic::error(
                    path,
                    f.line,
                    1,
                    "required field 'description' must not be empty",
                )
                .with_rule("claude/skills/missing-description", Difficulty::Easy),
            );
        }
        Some(f) if f.value.len() > 1024 => {
            diagnostics.push(
                Diagnostic::error(
                    path,
                    f.line,
                    1,
                    format!(
                        "'description' exceeds 1024 characters (got {})",
                        f.value.len()
                    ),
                )
                .with_rule("claude/skills/invalid-description", Difficulty::Easy),
            );
        }
        Some(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentlint_core::testing::{assert_clean, assert_error_contains};

    const SKILL_MD: &str = ".claude/skills/my-skill/SKILL.md";

    #[test]
    fn valid_skill_no_diagnostics() {
        let src = "---\nname: my-skill\ndescription: does things\n---\nbody\n";
        assert_clean(&SkillsValidator::validate(Path::new(SKILL_MD), src));
    }

    #[test]
    fn supporting_files_are_skipped() {
        // Non-SKILL.md files under a skill dir must not be validated.
        let src = "no frontmatter here";
        assert_clean(&SkillsValidator::validate(
            Path::new(".claude/skills/my-skill/references/guide.md"),
            src,
        ));
    }

    #[test]
    fn missing_frontmatter_is_error() {
        let src = "no frontmatter here";
        assert_error_contains(
            &SkillsValidator::validate(Path::new(SKILL_MD), src),
            "missing frontmatter",
        );
    }

    #[test]
    fn missing_name_is_error() {
        let src = "---\ndescription: does things\n---\n";
        assert_error_contains(
            &SkillsValidator::validate(Path::new(SKILL_MD), src),
            "missing required field 'name'",
        );
    }

    #[test]
    fn missing_description_is_error() {
        let src = "---\nname: my-skill\n---\n";
        assert_error_contains(
            &SkillsValidator::validate(Path::new(SKILL_MD), src),
            "missing required field 'description'",
        );
    }

    #[test]
    fn name_too_long_is_error() {
        let long = "a".repeat(65);
        let src = format!("---\nname: {long}\ndescription: ok\n---\n");
        assert_error_contains(
            &SkillsValidator::validate(Path::new(".claude/skills/my-skill/SKILL.md"), &src),
            "exceeds 64 characters",
        );
    }

    #[test]
    fn name_with_uppercase_is_error() {
        let src = "---\nname: My-Skill\ndescription: ok\n---\n";
        assert_error_contains(
            &SkillsValidator::validate(Path::new(SKILL_MD), src),
            "lowercase letters",
        );
    }

    #[test]
    fn name_leading_hyphen_is_error() {
        let src = "---\nname: -my-skill\ndescription: ok\n---\n";
        assert_error_contains(
            &SkillsValidator::validate(Path::new(SKILL_MD), src),
            "must not start or end with a hyphen",
        );
    }

    #[test]
    fn name_consecutive_hyphens_is_error() {
        let src = "---\nname: my--skill\ndescription: ok\n---\n";
        assert_error_contains(
            &SkillsValidator::validate(Path::new(SKILL_MD), src),
            "consecutive hyphens",
        );
    }

    #[test]
    fn name_mismatch_dir_is_error() {
        let src = "---\nname: other-name\ndescription: ok\n---\n";
        assert_error_contains(
            &SkillsValidator::validate(Path::new(SKILL_MD), src),
            "must match the skill directory name",
        );
    }

    #[test]
    fn namespaced_name_matching_dir_is_valid() {
        let src = "---\nname: godmode:my-skill\ndescription: ok\n---\n";
        assert_clean(&SkillsValidator::validate(Path::new(SKILL_MD), src));
    }

    #[test]
    fn namespaced_name_slug_mismatch_is_error() {
        let src = "---\nname: godmode:other-skill\ndescription: ok\n---\n";
        assert_error_contains(
            &SkillsValidator::validate(Path::new(SKILL_MD), src),
            "must match the skill directory name",
        );
    }

    #[test]
    fn double_namespace_is_error() {
        let src = "---\nname: a:b:c\ndescription: ok\n---\n";
        assert_error_contains(
            &SkillsValidator::validate(Path::new(SKILL_MD), src),
            "at most one namespace separator",
        );
    }

    #[test]
    fn description_too_long_is_error() {
        let long = "a".repeat(1025);
        let src = format!("---\nname: my-skill\ndescription: {long}\n---\n");
        assert_error_contains(
            &SkillsValidator::validate(Path::new(SKILL_MD), &src),
            "exceeds 1024 characters",
        );
    }
}
