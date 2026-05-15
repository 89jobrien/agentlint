use crate::frontmatter::{self, Field};
use agentlint_core::Diagnostic;
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
                return vec![Diagnostic::error(
                    path,
                    1,
                    1,
                    "missing frontmatter: file must start with '---'",
                )];
            }
            Err(frontmatter::ParseError::UnclosedFence) => {
                return vec![Diagnostic::error(
                    path,
                    1,
                    1,
                    "unclosed frontmatter fence: missing closing '---'",
                )];
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
            diagnostics.push(Diagnostic::error(
                path,
                1,
                1,
                "missing required field 'name'",
            ));
            return;
        }
        Some(f) if f.value.is_empty() => {
            diagnostics.push(Diagnostic::error(
                path,
                f.line,
                1,
                "required field 'name' must not be empty",
            ));
            return;
        }
        Some(f) => f,
    };

    let name = &field.value;

    if name.len() > 64 {
        diagnostics.push(Diagnostic::error(
            path,
            field.line,
            1,
            format!("'name' exceeds 64 characters (got {})", name.len()),
        ));
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        diagnostics.push(Diagnostic::error(
            path,
            field.line,
            1,
            "'name' must contain only lowercase letters, digits, and hyphens",
        ));
    }

    if name.starts_with('-') || name.ends_with('-') {
        diagnostics.push(Diagnostic::error(
            path,
            field.line,
            1,
            "'name' must not start or end with a hyphen",
        ));
    }

    if name.contains("--") {
        diagnostics.push(Diagnostic::error(
            path,
            field.line,
            1,
            "'name' must not contain consecutive hyphens",
        ));
    }

    // name must match the skill's directory name (agentskills.io spec).
    // path is `.claude/skills/<skill-name>/SKILL.md`; parent() gives the skill dir.
    if let Some(dir_name) = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
    {
        if name != dir_name {
            diagnostics.push(Diagnostic::error(
                path,
                field.line,
                1,
                format!("'name' ({name}) must match the skill directory name ({dir_name})"),
            ));
        }
    }
}

fn validate_description(path: &Path, fields: &[Field], diagnostics: &mut Vec<Diagnostic>) {
    match fields.iter().find(|f| f.key == "description") {
        None => {
            diagnostics.push(Diagnostic::error(
                path,
                1,
                1,
                "missing required field 'description'",
            ));
        }
        Some(f) if f.value.is_empty() => {
            diagnostics.push(Diagnostic::error(
                path,
                f.line,
                1,
                "required field 'description' must not be empty",
            ));
        }
        Some(f) if f.value.len() > 1024 => {
            diagnostics.push(Diagnostic::error(
                path,
                f.line,
                1,
                format!(
                    "'description' exceeds 1024 characters (got {})",
                    f.value.len()
                ),
            ));
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
    fn description_too_long_is_error() {
        let long = "a".repeat(1025);
        let src = format!("---\nname: my-skill\ndescription: {long}\n---\n");
        assert_error_contains(
            &SkillsValidator::validate(Path::new(SKILL_MD), &src),
            "exceeds 1024 characters",
        );
    }
}
