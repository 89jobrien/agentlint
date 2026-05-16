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
    // Additional Claude Code settings keys
    "cleanupPeriodDays",
    "effortLevel",
    "enabledPlugins",
    "extraKnownMarketplaces",
    "fastMode",
    "skipDangerousModePermissionPrompt",
    "statusLine",
    "verbose",
];

/// Warn when a single matcher has more hooks than this. Each hook spawns a
/// subprocess; too many causes process accumulation in long sessions.
const MAX_HOOKS_PER_MATCHER: usize = 7;

/// Hook command substrings that produce a warning diagnostic.
/// Matched against the full command string regardless of absolute prefix.
const WARN_PATTERNS: &[(&str, &str)] = &[
    (
        "cargo",
        "invokes the Rust compiler on every hook call; consolidate or use a git hook instead",
    ),
    (
        "clippy",
        "runs clippy on every hook call; redundant with cargo-check and expensive",
    ),
    (
        "python3",
        "spawns a Python interpreter on every hook call; prefer a compiled binary",
    ),
    (
        "python",
        "spawns a Python interpreter on every hook call; prefer a compiled binary",
    ),
    (
        "node",
        "spawns a Node.js runtime on every hook call; prefer a compiled binary",
    ),
    (
        "ruby",
        "spawns a Ruby interpreter on every hook call; prefer a compiled binary",
    ),
];

/// Hook command substrings that produce an error diagnostic.
/// These patterns are always wrong in an agent hook context.
const ERROR_PATTERNS: &[(&str, &str)] = &[(
    "sleep",
    "sleep in a hook blocks the agent; remove the sleep or run the hook async",
)];

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
                if let Some(v) = perms.get(field)
                    && !is_array_of_strings(v)
                {
                    diags.push(Diagnostic::error(
                        path,
                        1,
                        1,
                        format!("permissions.{field} must be an array of strings"),
                    ));
                }
            }
        }

        // hooks.<event> is an array of matcher groups: [{matcher, hooks: [{type, command}]}]
        if let Some(hooks) = obj.get("hooks").and_then(|v| v.as_object()) {
            for (event, entries) in hooks {
                let Some(groups) = entries.as_array() else {
                    continue;
                };
                for (gi, group) in groups.iter().enumerate() {
                    let matcher = group
                        .get("matcher")
                        .and_then(|v| v.as_str())
                        .unwrap_or("<no matcher>");

                    let inner = match group.get("hooks").and_then(|v| v.as_array()) {
                        Some(h) => h,
                        None => {
                            diags.push(Diagnostic::error(
                                path,
                                1,
                                1,
                                format!(
                                    "hooks.{event}[{gi}] (matcher: {matcher:?}) \
                                     must have a 'hooks' array"
                                ),
                            ));
                            continue;
                        }
                    };

                    // Structural check: each inner hook needs a command.
                    for (hi, hook) in inner.iter().enumerate() {
                        if hook.get("command").and_then(|v| v.as_str()).is_none() {
                            diags.push(Diagnostic::error(
                                path,
                                1,
                                1,
                                format!(
                                    "hooks.{event}[{gi}].hooks[{hi}] \
                                     must have a 'command' string field"
                                ),
                            ));
                        }
                    }

                    // Warn when a matcher has too many hooks (process spawn pressure).
                    if inner.len() > MAX_HOOKS_PER_MATCHER {
                        diags.push(Diagnostic::warning(
                            path,
                            1,
                            1,
                            format!(
                                "hooks.{event} matcher {matcher:?} has {} hooks \
                                 (>{MAX_HOOKS_PER_MATCHER}); each spawns a subprocess — \
                                 consider consolidating into a single binary",
                                inner.len()
                            ),
                        ));
                    }

                    // Check hook commands against warn/error pattern tables.
                    for hook in inner {
                        let cmd = hook.get("command").and_then(|v| v.as_str()).unwrap_or("");
                        for (pattern, reason) in WARN_PATTERNS {
                            if cmd.contains(pattern) {
                                diags.push(Diagnostic::warning(
                                    path,
                                    1,
                                    1,
                                    format!("hooks.{event} command contains '{pattern}': {reason}"),
                                ));
                            }
                        }
                        for (pattern, reason) in ERROR_PATTERNS {
                            if cmd.contains(pattern) {
                                diags.push(Diagnostic::error(
                                    path,
                                    1,
                                    1,
                                    format!("hooks.{event} command contains '{pattern}': {reason}"),
                                ));
                            }
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
        .is_some_and(|arr| arr.iter().all(|e| e.is_string()))
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
        let src =
            r#"{"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command"}]}]}}"#;
        assert_error_contains(
            &SettingsValidator::validate(Path::new(PATH), src),
            "'command' string field",
        );
    }

    #[test]
    fn hooks_entry_with_command_is_clean() {
        let src = r#"{"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "/usr/bin/script"}]}]}}"#;
        assert_clean(&SettingsValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn hooks_too_many_per_matcher_is_warning() {
        // 8 hooks on one matcher should warn
        let hook = r#"{"type":"command","command":"/usr/bin/x"}"#;
        let hooks_arr = std::iter::repeat(hook)
            .take(8)
            .collect::<Vec<_>>()
            .join(",");
        let src =
            format!(r#"{{"hooks":{{"PreToolUse":[{{"matcher":"Bash","hooks":[{hooks_arr}]}}]}}}}"#);
        let diags = SettingsValidator::validate(Path::new(PATH), &src);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("spawns a subprocess")),
            "expected process-spawn warning, got: {diags:?}"
        );
    }

    #[test]
    fn hook_with_python3_is_warning() {
        let src = r#"{"hooks":{"PostToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/usr/bin/python3 /home/user/.claude/hooks/redact.py"}]}]}}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags.iter().any(|d| d.message.contains("python3")
                && d.severity == agentlint_core::Severity::Warning),
            "expected python3 warning, got: {diags:?}"
        );
    }

    #[test]
    fn hook_with_sleep_is_error() {
        let src = r#"{"hooks":{"PostToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/bin/sleep 10"}]}]}}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("sleep")
                    && d.severity == agentlint_core::Severity::Error),
            "expected sleep error, got: {diags:?}"
        );
    }

    #[test]
    fn hook_with_cargo_is_warning() {
        let src = r#"{"hooks":{"PreToolUse":[{"matcher":"Edit","hooks":[{"type":"command","command":"/home/user/.cargo/bin/cargo clippy"}]}]}}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("cargo")
                    && d.severity == agentlint_core::Severity::Warning),
            "expected cargo warning, got: {diags:?}"
        );
    }

    #[test]
    fn hooks_missing_hooks_array_is_error() {
        let src = r#"{"hooks": {"PreToolUse": [{"matcher": "Bash"}]}}"#;
        assert_error_contains(
            &SettingsValidator::validate(Path::new(PATH), src),
            "must have a 'hooks' array",
        );
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
