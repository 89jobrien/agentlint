use crate::macros::frontmatter_validator;

pub struct CommandsValidator;

frontmatter_validator!(CommandsValidator, required: ["name", "description"]);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentlint_core::testing::{assert_clean, assert_error_contains};
    use std::path::Path;

    const PATH: &str = ".claude/commands/test.md";

    #[test]
    fn valid_command_no_diagnostics() {
        let src = "---\nname: deploy\ndescription: deploys the app\n---\nbody\n";
        assert_clean(&CommandsValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn missing_name_is_error() {
        let src = "---\ndescription: deploys the app\n---\n";
        assert_error_contains(
            &CommandsValidator::validate(Path::new(PATH), src),
            "missing required field 'name'",
        );
    }

    #[test]
    fn missing_description_is_error() {
        let src = "---\nname: deploy\n---\n";
        assert_error_contains(
            &CommandsValidator::validate(Path::new(PATH), src),
            "missing required field 'description'",
        );
    }
}
