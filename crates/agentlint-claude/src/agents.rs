use crate::macros::frontmatter_validator;

pub struct AgentsValidator;

frontmatter_validator!(AgentsValidator, required: ["name", "description"]);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentlint_core::testing::{assert_clean, assert_error_at, assert_error_contains};
    use std::path::Path;

    const PATH: &str = ".claude/agents/test.md";

    #[test]
    fn valid_agent_no_diagnostics() {
        let src = "---\nname: my-agent\ndescription: does things\n---\nbody\n";
        assert_clean(&AgentsValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn missing_name_is_error() {
        let src = "---\ndescription: does things\n---\n";
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
        let src = "---\nname: \ndescription: ok\n---\n";
        assert_error_at(
            &AgentsValidator::validate(Path::new(PATH), src),
            2,
            "'name'",
        );
    }

    #[test]
    fn optional_fields_do_not_cause_errors() {
        let src =
            "---\nname: my-agent\ndescription: ok\ntools: [Bash]\nmodel: claude-opus-4-6\n---\n";
        assert_clean(&AgentsValidator::validate(Path::new(PATH), src));
    }
}
