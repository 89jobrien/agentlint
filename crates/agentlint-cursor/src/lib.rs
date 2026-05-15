use agentlint_core::{Diagnostic, Validator};
use agentlint_frontmatter::{ParseError, parse};
use std::path::Path;

pub struct CursorValidator;

impl Validator for CursorValidator {
    fn patterns(&self) -> &[&str] {
        &[
            ".cursor/rules/**/*.mdc",
            ".cursor/rules/**/*.md",
            ".cursorrules",
        ]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        // Frontmatter is optional — only validate when the opening fence is present.
        if !src.starts_with("---\n") && !src.starts_with("---\r\n") {
            return vec![];
        }
        match parse(src) {
            Ok(_) => vec![],
            Err(ParseError::UnclosedFence) => vec![Diagnostic::error(
                path,
                1,
                1,
                "unclosed frontmatter fence: missing closing '---'",
            )],
            Err(ParseError::NoFence) => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn v() -> CursorValidator {
        CursorValidator
    }

    #[test]
    fn no_frontmatter_is_clean() {
        let diags = v().validate(Path::new("rule.md"), "# Hello\nsome content\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn well_formed_frontmatter_is_clean() {
        let src = "---\ntitle: test\ndescription: lint files\n---\n# Body\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(diags.is_empty());
    }

    #[test]
    fn unclosed_fence_is_error() {
        let src = "---\ntitle: test\n# no closing fence\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("unclosed"),
            "unexpected message: {}",
            diags[0].message
        );
    }
}
