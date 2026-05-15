use agentlint_core::Diagnostic;
use std::path::Path;

pub struct SettingsValidator;

const KNOWN_KEYS: &[&str] = &[
    "permissions",
    "env",
    "hooks",
    "mcpServers",
    "model",
    "apiKeyHelper",
    "includeCoAuthoredBy",
    "enabledMcpjsonServers",
];

impl SettingsValidator {
    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        let value: serde_json::Value = match serde_json::from_str(src) {
            Ok(v) => v,
            Err(e) => {
                return vec![Diagnostic::error(path, 1, 1, format!("invalid JSON: {e}"))];
            }
        };

        let obj = match value.as_object() {
            Some(o) => o,
            None => {
                return vec![Diagnostic::error(
                    path,
                    1,
                    1,
                    "settings must be a JSON object",
                )];
            }
        };

        let mut diags = Vec::new();

        // Unknown top-level keys.
        for key in obj.keys() {
            if !KNOWN_KEYS.contains(&key.as_str()) {
                diags.push(Diagnostic::error(
                    path,
                    1,
                    1,
                    format!("unknown top-level key '{key}'"),
                ));
            }
        }

        // permissions.allow / permissions.deny must be arrays of strings.
        if let Some(perms) = obj.get("permissions").and_then(|v| v.as_object()) {
            for &field in &["allow", "deny"] {
                if let Some(v) = perms.get(field) {
                    if !is_array_of_strings(v) {
                        diags.push(Diagnostic::error(
                            path,
                            1,
                            1,
                            format!("permissions.{field} must be an array of strings"),
                        ));
                    }
                }
            }
        }

        // hooks.<event> must be an array of objects each with a "command" string.
        if let Some(hooks) = obj.get("hooks").and_then(|v| v.as_object()) {
            for (event, entries) in hooks {
                if let Some(arr) = entries.as_array() {
                    for (i, entry) in arr.iter().enumerate() {
                        if entry.get("command").and_then(|v| v.as_str()).is_none() {
                            diags.push(Diagnostic::error(
                                path,
                                1,
                                1,
                                format!("hooks.{event}[{i}] must have a 'command' string field"),
                            ));
                        }
                    }
                }
            }
        }

        diags
    }
}

fn is_array_of_strings(v: &serde_json::Value) -> bool {
    v.as_array()
        .map_or(false, |arr| arr.iter().all(|e| e.is_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentlint_core::testing::{assert_clean, assert_error_contains};
    use std::path::Path;

    const PATH: &str = ".claude/settings.json";

    #[test]
    fn valid_settings_no_diagnostics() {
        let src = r#"{"permissions": {"allow": ["Bash"], "deny": []}}"#;
        assert_clean(&SettingsValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn invalid_json_is_error() {
        let src = "not json";
        assert_error_contains(
            &SettingsValidator::validate(Path::new(PATH), src),
            "invalid JSON",
        );
    }

    #[test]
    fn unknown_top_level_key_is_error() {
        let src = r#"{"theme": "dark"}"#;
        assert_error_contains(
            &SettingsValidator::validate(Path::new(PATH), src),
            "unknown top-level key 'theme'",
        );
    }

    #[test]
    fn permissions_allow_not_array_is_error() {
        let src = r#"{"permissions": {"allow": "Bash"}}"#;
        assert_error_contains(
            &SettingsValidator::validate(Path::new(PATH), src),
            "permissions.allow must be an array of strings",
        );
    }

    #[test]
    fn permissions_deny_not_array_of_strings_is_error() {
        let src = r#"{"permissions": {"deny": [1, 2]}}"#;
        assert_error_contains(
            &SettingsValidator::validate(Path::new(PATH), src),
            "permissions.deny must be an array of strings",
        );
    }

    #[test]
    fn hooks_entry_missing_command_is_error() {
        let src = r#"{"hooks": {"PreToolUse": [{"matcher": "Bash"}]}}"#;
        assert_error_contains(
            &SettingsValidator::validate(Path::new(PATH), src),
            "'command' string field",
        );
    }

    #[test]
    fn hooks_entry_with_command_is_clean() {
        let src =
            r#"{"hooks": {"PreToolUse": [{"matcher": "Bash", "command": "/usr/bin/script"}]}}"#;
        assert_clean(&SettingsValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn all_known_top_level_keys_accepted() {
        let src = r#"{
            "permissions": {},
            "env": {},
            "hooks": {},
            "mcpServers": {},
            "model": "claude-sonnet-4-6",
            "apiKeyHelper": "op://vault/item/field",
            "includeCoAuthoredBy": true,
            "enabledMcpjsonServers": []
        }"#;
        assert_clean(&SettingsValidator::validate(Path::new(PATH), src));
    }
}
