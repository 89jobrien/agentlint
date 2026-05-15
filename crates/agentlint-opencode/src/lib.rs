use agentlint_core::{Diagnostic, Validator};
use std::path::Path;

pub struct OpenCodeValidator;

impl Validator for OpenCodeValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md", "opencode.json"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        match path.file_name().and_then(|n| n.to_str()) {
            Some("AGENTS.md") => {
                if src.trim().is_empty() {
                    vec![Diagnostic::error(path, 1, 1, "AGENTS.md is empty")]
                } else {
                    vec![]
                }
            }
            Some("opencode.json") => match serde_json::from_str::<serde_json::Value>(src) {
                Ok(_) => vec![],
                Err(e) => vec![Diagnostic::error(
                    path,
                    e.line(),
                    e.column(),
                    format!("invalid JSON: {e}"),
                )],
            },
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn v() -> OpenCodeValidator {
        OpenCodeValidator
    }

    #[test]
    fn agents_non_empty_is_clean() {
        let diags = v().validate(
            Path::new("AGENTS.md"),
            "# Agent instructions\n\nDo stuff.\n",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn agents_empty_is_error() {
        let diags = v().validate(Path::new("AGENTS.md"), "   \n\t\n  ");
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("empty"),
            "unexpected message: {}",
            diags[0].message
        );
    }

    #[test]
    fn opencode_json_valid_is_clean() {
        let src = r#"{"model": "claude-opus-4", "temperature": 0.7}"#;
        let diags = v().validate(Path::new("opencode.json"), src);
        assert!(diags.is_empty());
    }

    #[test]
    fn opencode_json_empty_object_is_clean() {
        let diags = v().validate(Path::new("opencode.json"), "{}");
        assert!(diags.is_empty());
    }

    #[test]
    fn opencode_json_invalid_is_error() {
        let src = r#"{"model": "claude-opus-4", bad}"#;
        let diags = v().validate(Path::new("opencode.json"), src);
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("invalid JSON"),
            "unexpected message: {}",
            diags[0].message
        );
    }
}
