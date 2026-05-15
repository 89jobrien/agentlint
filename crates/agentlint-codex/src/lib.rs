use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

pub struct CodexValidator;

impl Validator for CodexValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md"]
    }

    fn validate(&self, _path: &Path, _src: &str) -> Vec<Diagnostic> {
        // TODO: non-empty content check
        vec![]
    }
}
