use crate::macros::frontmatter_validator;

pub struct SkillsValidator;

frontmatter_validator!(SkillsValidator, required: ["name", "description"]);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentlint_core::testing::{assert_clean, assert_error_contains};
    use std::path::Path;

    const PATH: &str = ".claude/skills/test.md";

    #[test]
    fn valid_skill_no_diagnostics() {
        let src = "---\nname: my-skill\ndescription: does things\n---\nbody\n";
        assert_clean(&SkillsValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn missing_name_is_error() {
        let src = "---\ndescription: does things\n---\n";
        assert_error_contains(
            &SkillsValidator::validate(Path::new(PATH), src),
            "missing required field 'name'",
        );
    }

    #[test]
    fn missing_description_is_error() {
        let src = "---\nname: my-skill\n---\n";
        assert_error_contains(
            &SkillsValidator::validate(Path::new(PATH), src),
            "missing required field 'description'",
        );
    }
}
