use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

pub struct CodexValidator;

impl Validator for CodexValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if src.trim().is_empty() {
            vec![Diagnostic::error(path, 1, 1, "AGENTS.md is empty")]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn non_empty_is_clean() {
        let v = CodexValidator;
        let diags = v.validate(Path::new("AGENTS.md"), "# Agents\n\nSome content.");
        assert!(diags.is_empty());
    }

    #[test]
    fn empty_file_is_error() {
        let v = CodexValidator;
        let diags = v.validate(Path::new("AGENTS.md"), "");
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("empty"));
    }

    #[test]
    fn whitespace_only_is_error() {
        let v = CodexValidator;
        let diags = v.validate(Path::new("AGENTS.md"), "   \n\t\n  ");
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("empty"));
    }
}
