use agentlint_core::{Diagnostic, Validator};
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
        if !src.starts_with("---\n") {
            return vec![];
        }
        // Content after the opening fence
        let after_open = &src[4..];
        if after_open.contains("\n---") {
            vec![]
        } else {
            vec![Diagnostic::error(path, 1, 1, "unclosed frontmatter fence")]
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
        let src = "---\ntitle: test\n---\n# Body\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(diags.is_empty());
    }

    #[test]
    fn unclosed_fence_is_error() {
        let src = "---\ntitle: test\n# Body\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("unclosed frontmatter"),
            "unexpected message: {}",
            diags[0].message
        );
    }
}
