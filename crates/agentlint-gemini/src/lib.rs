use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

pub struct GeminiValidator;

impl Validator for GeminiValidator {
    fn patterns(&self) -> &[&str] {
        &["GEMINI.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if src.trim().is_empty() {
            vec![Diagnostic::error(path, 1, 1, "GEMINI.md is empty")]
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
        let v = GeminiValidator;
        let diags = v.validate(Path::new("GEMINI.md"), "# Gemini\n\nSome content.\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn empty_file_is_error() {
        let v = GeminiValidator;
        let diags = v.validate(Path::new("GEMINI.md"), "");
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("empty"));
    }

    #[test]
    fn whitespace_only_is_error() {
        let v = GeminiValidator;
        let diags = v.validate(Path::new("GEMINI.md"), "   \n\t\n  ");
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("empty"));
    }
}
