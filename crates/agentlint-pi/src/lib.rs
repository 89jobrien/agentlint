use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

pub struct PiValidator;

impl Validator for PiValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md", "SYSTEM.md"]
    }

    fn validate(&self, _path: &Path, _src: &str) -> Vec<Diagnostic> {
        // TODO: non-empty content check
        vec![]
    }
}
