use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

pub struct PiValidator;

impl Validator for PiValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md", "SYSTEM.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if src.trim().is_empty() {
            let filename = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            vec![Diagnostic::error(
                path,
                1,
                1,
                format!("{filename} is empty"),
            )]
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
    fn agents_non_empty_is_clean() {
        let v = PiValidator;
        let diags = v.validate(Path::new("AGENTS.md"), "# Agents\n\nSome content.");
        assert!(diags.is_empty());
    }

    #[test]
    fn system_non_empty_is_clean() {
        let v = PiValidator;
        let diags = v.validate(Path::new("SYSTEM.md"), "# System\n\nSome content.");
        assert!(diags.is_empty());
    }

    #[test]
    fn agents_empty_is_error() {
        let v = PiValidator;
        let diags = v.validate(Path::new("AGENTS.md"), "");
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("empty"),
            "message was: {}",
            diags[0].message
        );
    }

    #[test]
    fn system_empty_is_error() {
        let v = PiValidator;
        let diags = v.validate(Path::new("SYSTEM.md"), "   \n  ");
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("empty"),
            "message was: {}",
            diags[0].message
        );
    }
}
