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

    fn validate(&self, _path: &Path, _src: &str) -> Vec<Diagnostic> {
        // TODO: optional frontmatter validation
        vec![]
    }
}
