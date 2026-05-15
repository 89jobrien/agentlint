use agentlint_core::Diagnostic;
use agentlint_frontmatter::{FieldRule, FrontmatterValidator};
use std::path::Path;
use std::sync::OnceLock;

pub struct CommandsValidator;

impl CommandsValidator {
    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        static VALIDATOR: OnceLock<FrontmatterValidator> = OnceLock::new();
        VALIDATOR
            .get_or_init(|| {
                FrontmatterValidator::builder()
                    .required(FieldRule::new("name"))
                    .required(FieldRule::new("description"))
                    .build()
            })
            .validate(path, src)
    }
}

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
