use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

pub struct OpenCodeValidator;

impl Validator for OpenCodeValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md", "opencode.json"]
    }

    fn validate(&self, _path: &Path, _src: &str) -> Vec<Diagnostic> {
        // TODO: AGENTS.md non-empty check; opencode.json valid-JSON check
        vec![]
    }
}
