use agentlint_core::{Diagnostic, Difficulty, Validator};
use std::path::Path;

/// Validates that OpenCode's `AGENTS.md` is non-empty.
pub struct AgentsMarkdownValidator;

impl Validator for AgentsMarkdownValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if src.trim().is_empty() {
            return vec![
                Diagnostic::error(path, 1, 1, "AGENTS.md is empty")
                    .with_rule("opencode/content/empty", Difficulty::Easy),
            ];
        }
        vec![]
    }
}

/// Known top-level keys for `opencode.json`.
const KNOWN_OPENCODE_KEYS: &[&str] = &[
    "model",
    "provider",
    "providers",
    "mcpServers",
    "theme",
    "keybinds",
    "disabled_providers",
    "autoshare",
];

/// Validates that `opencode.json` is well-formed JSON and has no unknown top-level keys.
pub struct OpenCodeJsonValidator;

impl Validator for OpenCodeJsonValidator {
    fn patterns(&self) -> &[&str] {
        &["opencode.json"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        let value: serde_json::Value = match serde_json::from_str(src) {
            Ok(v) => v,
            Err(e) => {
                return vec![
                    Diagnostic::error(path, e.line(), e.column(), format!("invalid JSON: {e}"))
                        .with_rule("opencode/config/invalid-json", Difficulty::Easy),
                ];
            }
        };

        let mut diags = Vec::new();

        if let Some(obj) = value.as_object() {
            for key in obj.keys() {
                if !KNOWN_OPENCODE_KEYS.contains(&key.as_str()) {
                    diags.push(
                        Diagnostic::warning(
                            path,
                            1,
                            1,
                            format!(
                                "unknown key '{key}' in opencode.json; \
                                 it will be silently ignored"
                            ),
                        )
                        .with_rule("opencode/config/unknown-key", Difficulty::Hard),
                    );
                }
            }
        }

        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn agents_non_empty_is_clean() {
        let diags =
            AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), "# OpenCode Instructions\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn agents_empty_is_error() {
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), "");
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("empty"),
            "message: {}",
            diags[0].message
        );
    }

    #[test]
    fn opencode_json_valid_is_clean() {
        let diags = OpenCodeJsonValidator.validate(
            Path::new("opencode.json"),
            r#"{"model": "claude-sonnet-4-6"}"#,
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn opencode_json_empty_object_is_clean() {
        let diags = OpenCodeJsonValidator.validate(Path::new("opencode.json"), "{}");
        assert!(diags.is_empty());
    }

    #[test]
    fn opencode_json_invalid_is_error() {
        let diags = OpenCodeJsonValidator.validate(Path::new("opencode.json"), "{bad json");
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("invalid JSON"),
            "message: {}",
            diags[0].message
        );
    }

    // #41 — unknown-key

    #[test]
    fn unknown_key_emits_warning() {
        let diags = OpenCodeJsonValidator.validate(
            Path::new("opencode.json"),
            r#"{"model": "gpt-4", "unknownOption": true}"#,
        );
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].rule.contains("unknown-key"),
            "rule: {}",
            diags[0].rule
        );
        assert!(
            diags[0].message.contains("unknownOption"),
            "message: {}",
            diags[0].message
        );
    }

    #[test]
    fn all_known_keys_are_clean() {
        let src = r#"{
            "model": "gpt-4",
            "provider": "openai",
            "providers": {},
            "mcpServers": {},
            "theme": "dark",
            "keybinds": {},
            "disabled_providers": [],
            "autoshare": false
        }"#;
        let diags = OpenCodeJsonValidator.validate(Path::new("opencode.json"), src);
        assert!(diags.is_empty(), "unexpected diags: {diags:?}");
    }

    #[test]
    fn multiple_unknown_keys_each_emit_warning() {
        let src = r#"{"foo": 1, "bar": 2, "model": "x"}"#;
        let diags = OpenCodeJsonValidator.validate(Path::new("opencode.json"), src);
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.rule.contains("unknown-key")));
    }
}
