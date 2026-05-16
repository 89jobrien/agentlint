use crate::mcp::validate_server_entry;
use agentlint_core::{Diagnostic, Difficulty};
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
    "attribution",
];

/// Warn when a single matcher has more hooks than this. Each hook spawns a
/// subprocess; too many causes process accumulation in long sessions.
const MAX_HOOKS_PER_MATCHER: usize = 7;

/// Hook command substrings that produce a warning diagnostic.
/// Matched against the full command string regardless of absolute prefix.
const WARN_PATTERNS: &[(&str, &str)] = &[
    (
        "cargo ",
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
                return vec![
                    Diagnostic::error(path, 1, 1, format!("invalid JSON: {e}"))
                        .with_rule("claude/settings/invalid-json", Difficulty::Easy),
                ];
            }
        };

        let obj = match value.as_object() {
            Some(o) => o,
            None => {
                return vec![
                    Diagnostic::error(path, 1, 1, "settings must be a JSON object")
                        .with_rule("claude/settings/invalid-json", Difficulty::Easy),
                ];
            }
        };

        let mut diags = Vec::new();

        // Unknown top-level keys.
        for key in obj.keys() {
            if !KNOWN_KEYS.contains(&key.as_str()) {
                diags.push(
                    Diagnostic::error(path, 1, 1, format!("unknown top-level key '{key}'"))
                        .with_rule("claude/settings/unknown-key", Difficulty::Hard),
                );
            }
        }

        // skipDangerousModePermissionPrompt: true disables all permission prompts globally.
        if obj
            .get("skipDangerousModePermissionPrompt")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    "skipDangerousModePermissionPrompt is true: all permission prompts are \
                     disabled globally; consider scoping with permissions.allow instead",
                )
                .with_rule("claude/settings/skip-dangerous-mode", Difficulty::Hard),
            );
        }

        // model key — validate against known Claude model IDs.
        const KNOWN_MODELS: &[&str] = &[
            "claude-opus-4-6",
            "claude-sonnet-4-6",
            "claude-haiku-4-5-20251001",
            "claude-3-5-sonnet-20241022",
            "claude-3-5-haiku-20241022",
            "claude-3-opus-20240229",
            "claude-3-sonnet-20240229",
            "claude-3-haiku-20240307",
        ];
        if let Some(model) = obj.get("model").and_then(|v| v.as_str())
            && !KNOWN_MODELS.contains(&model)
        {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    format!(
                        "model '{model}' is not a known Claude model ID; \
                         check for typos or update agentlint's known-models list"
                    ),
                )
                .with_rule("claude/settings/unknown-model", Difficulty::Hard),
            );
        }

        // Top-level env block — warn on any op:// URIs.
        if let Some(env) = obj.get("env").and_then(|v| v.as_object()) {
            for (key, val) in env {
                if let Some(s) = val.as_str()
                    && s.starts_with("op://")
                {
                    diags.push(
                        Diagnostic::warning(
                            path,
                            1,
                            1,
                            format!(
                                "env.{key}: op:// URI will not resolve in Claude's shell \
                                 context; use 'apiKeyHelper' or pre-resolve the secret \
                                 before launch"
                            ),
                        )
                        .with_rule("claude/settings/env-unresolved-op-ref", Difficulty::Hard),
                    );
                }
            }
        }

        // permissions.allow / permissions.deny must be arrays of strings.
        if let Some(perms) = obj.get("permissions").and_then(|v| v.as_object()) {
            for &field in &["allow", "deny"] {
                if let Some(v) = perms.get(field)
                    && !is_array_of_strings(v)
                {
                    diags.push(
                        Diagnostic::error(
                            path,
                            1,
                            1,
                            format!("permissions.{field} must be an array of strings"),
                        )
                        .with_rule("claude/settings/invalid-permissions", Difficulty::Easy),
                    );
                }
            }

            // Inspect individual allow entries for dangerous patterns.
            if let Some(allow) = perms.get("allow").and_then(|v| v.as_array()) {
                for entry in allow {
                    if let Some(s) = entry.as_str() {
                        check_allow_entry(path, s, &mut diags);
                    }
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
                            diags.push(
                                Diagnostic::error(
                                    path,
                                    1,
                                    1,
                                    format!(
                                        "hooks.{event}[{gi}] (matcher: {matcher:?}) \
                                         must have a 'hooks' array"
                                    ),
                                )
                                .with_rule(
                                    "claude/settings/hook-missing-hooks-array",
                                    Difficulty::Easy,
                                ),
                            );
                            continue;
                        }
                    };

                    // Structural check: each inner hook needs a command.
                    for (hi, hook) in inner.iter().enumerate() {
                        if hook.get("command").and_then(|v| v.as_str()).is_none() {
                            diags.push(
                                Diagnostic::error(
                                    path,
                                    1,
                                    1,
                                    format!(
                                        "hooks.{event}[{gi}].hooks[{hi}] \
                                         must have a 'command' string field"
                                    ),
                                )
                                .with_rule(
                                    "claude/settings/hook-missing-command",
                                    Difficulty::Easy,
                                ),
                            );
                        }
                    }

                    // Warn when a matcher has too many hooks (process spawn pressure).
                    if inner.len() > MAX_HOOKS_PER_MATCHER {
                        diags.push(
                            Diagnostic::warning(
                                path,
                                1,
                                1,
                                format!(
                                    "hooks.{event} matcher {matcher:?} has {} hooks \
                                     (>{MAX_HOOKS_PER_MATCHER}); each spawns a subprocess — \
                                     consider consolidating into a single binary",
                                    inner.len()
                                ),
                            )
                            .with_rule("claude/settings/too-many-hooks", Difficulty::Hard),
                        );
                    }

                    // Check hook commands against warn/error pattern tables.
                    for hook in inner {
                        let cmd = hook.get("command").and_then(|v| v.as_str()).unwrap_or("");
                        for (pattern, reason) in WARN_PATTERNS {
                            if cmd.contains(pattern) {
                                diags.push(
                                    Diagnostic::warning(
                                        path,
                                        1,
                                        1,
                                        format!(
                                            "hooks.{event} command contains '{pattern}': {reason}"
                                        ),
                                    )
                                    .with_rule(
                                        "claude/settings/expensive-hook-command",
                                        Difficulty::Hard,
                                    ),
                                );
                            }
                        }
                        for (pattern, reason) in ERROR_PATTERNS {
                            if cmd.contains(pattern) {
                                diags.push(
                                    Diagnostic::error(
                                        path,
                                        1,
                                        1,
                                        format!(
                                            "hooks.{event} command contains '{pattern}': {reason}"
                                        ),
                                    )
                                    .with_rule("claude/settings/sleep-in-hook", Difficulty::Easy),
                                );
                            }
                        }
                    }
                }
            }
        }

        // mcpServers entries — validate each server using the shared MCP helper.
        if let Some(servers) = obj.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, entry) in servers {
                match entry.as_object() {
                    Some(server) => validate_server_entry(path, name, server, &mut diags),
                    None => diags.push(
                        Diagnostic::error(
                            path,
                            1,
                            1,
                            format!("mcpServers.{name}: server entry must be a JSON object"),
                        )
                        .with_rule("claude/mcp/invalid-server-entry", Difficulty::Easy),
                    ),
                }
            }
        }

        // Deduplicate: for rules that fire per-entry (allow list, hook commands),
        // only keep the first occurrence of each rule ID per file.
        let mut seen = std::collections::HashSet::new();
        diags.retain(|d| {
            if d.rule.is_empty() {
                return true;
            }
            seen.insert(d.rule)
        });

        diags
    }
}

/// Inspect a single `permissions.allow` entry string for dangerous patterns.
///
/// Entry format: `TOOL(SPEC)` e.g. `Bash(git add:*)`, `Read(//Users/joe/**)`.
fn check_allow_entry(path: &Path, entry: &str, diags: &mut Vec<Diagnostic>) {
    if entry.starts_with("Bash(") {
        // Hardcoded credentials via sshpass.
        if entry.contains("sshpass -p") {
            diags.push(
                Diagnostic::error(
                    path,
                    1,
                    1,
                    "permissions.allow contains 'sshpass -p': hardcoded credential in allow \
                     list; use SSH key authentication instead",
                )
                .with_rule("claude/settings/sshpass-credential", Difficulty::Easy),
            );
        }
        // Sleep blocks the agent between tool calls.
        if entry.contains("sleep ") {
            diags.push(
                Diagnostic::error(
                    path,
                    1,
                    1,
                    "permissions.allow Bash entry contains 'sleep': sleeping in an allow rule \
                     stalls the agent; remove or move to an async process",
                )
                .with_rule("claude/settings/sleep-in-allow", Difficulty::Easy),
            );
        }
        // Baked-in CI workflow modifications are stale one-off allowances.
        if entry.contains(".github/workflows") {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    "permissions.allow Bash entry references .github/workflows: CI workflow \
                     modifications should not be permanently baked into the allow list",
                )
                .with_rule("claude/settings/ci-workflow-in-allow", Difficulty::Hard),
            );
        }
    }

    if entry.starts_with("Read(") && entry.contains("**") {
        // Extract the path spec from Read(...).
        let inner = entry
            .strip_prefix("Read(")
            .unwrap_or("")
            .trim_end_matches(')');
        if is_broad_read_path(inner) {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    format!(
                        "permissions.allow Read({inner}) grants broad filesystem read access; \
                         scope to a specific project directory"
                    ),
                )
                .with_rule("claude/settings/broad-read", Difficulty::Painful),
            );
        }
    }
}

/// Returns true when a Read permission path covers a broad area of the filesystem.
///
/// Paths with ≤3 concrete (non-wildcard) components after stripping leading slashes
/// and the trailing `/**` are considered broad — e.g. `//Users/joe/**` (home dir) or
/// `//Users/joe/dev/**` (entire dev tree). Deeper paths like `//Users/joe/dev/myproject/**`
/// are acceptable.
fn is_broad_read_path(spec: &str) -> bool {
    let stripped = spec.trim_start_matches('/');
    let without_glob = stripped.trim_end_matches("/**");
    let concrete_parts = without_glob
        .split('/')
        .filter(|p| !p.is_empty() && !p.contains('*'))
        .count();
    concrete_parts <= 3
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
        let src = r#"{"hooks":{"PreToolUse":[{"matcher":"Edit","hooks":[{"type":"command","command":"cargo clippy --workspace"}]}]}}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("cargo")
                    && d.severity == agentlint_core::Severity::Warning),
            "expected cargo warning, got: {diags:?}"
        );
    }

    // --- permissions.allow content checks ---

    #[test]
    fn skip_dangerous_mode_true_is_warning() {
        let src = r#"{"skipDangerousModePermissionPrompt": true}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("skipDangerousModePermissionPrompt")
                    && d.severity == agentlint_core::Severity::Warning),
            "expected warning, got: {diags:?}"
        );
    }

    #[test]
    fn skip_dangerous_mode_false_is_clean() {
        let src = r#"{"skipDangerousModePermissionPrompt": false}"#;
        assert_clean(&SettingsValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn allow_sshpass_credential_is_error() {
        let src = r#"{"permissions": {"allow": ["Bash(sshpass -p 'secret' ssh user@host)"]}}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags.iter().any(|d| d.message.contains("sshpass -p")
                && d.severity == agentlint_core::Severity::Error),
            "expected credential error, got: {diags:?}"
        );
    }

    #[test]
    fn allow_sleep_bash_is_error() {
        let src = r#"{"permissions": {"allow": ["Bash(sleep 30 && curl http://localhost)"]}}"#;
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
    fn allow_ci_workflow_modification_is_warning() {
        let src =
            r#"{"permissions": {"allow": ["Bash(tee /repo/.github/workflows/ci.yml << 'EOF')"]}}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags.iter().any(|d| d.message.contains(".github/workflows")
                && d.severity == agentlint_core::Severity::Warning),
            "expected CI workflow warning, got: {diags:?}"
        );
    }

    #[test]
    fn allow_broad_home_read_is_warning() {
        let src = r#"{"permissions": {"allow": ["Read(//Users/joe/**)"]}}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("broad filesystem read")
                    && d.severity == agentlint_core::Severity::Warning),
            "expected broad read warning, got: {diags:?}"
        );
    }

    #[test]
    fn allow_broad_dev_tree_read_is_warning() {
        let src = r#"{"permissions": {"allow": ["Read(//Users/joe/dev/**)"]}}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("broad filesystem read")),
            "expected broad read warning, got: {diags:?}"
        );
    }

    #[test]
    fn allow_specific_project_read_is_clean() {
        let src = r#"{"permissions": {"allow": ["Read(//Users/joe/dev/myproject/**)"]}}"#;
        assert_clean(&SettingsValidator::validate(Path::new(PATH), src));
    }

    // --- hooks structural checks ---

    #[test]
    fn hooks_missing_hooks_array_is_error() {
        let src = r#"{"hooks": {"PreToolUse": [{"matcher": "Bash"}]}}"#;
        assert_error_contains(
            &SettingsValidator::validate(Path::new(PATH), src),
            "must have a 'hooks' array",
        );
    }

    #[test]
    fn mcp_servers_in_settings_op_uri_warns() {
        let src = r#"{
            "mcpServers": {
                "my-server": {
                    "command": "npx",
                    "env": {"KEY": "op://Personal/item/field"}
                }
            }
        }"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags.iter().any(|d| d.message.contains("op://")),
            "expected op:// warning in settings mcpServers, got: {diags:?}"
        );
    }

    #[test]
    fn unknown_model_is_warning() {
        let src = r#"{"model": "gpt-4o"}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags.iter().any(|d| d.message.contains("gpt-4o")
                && d.rule == "claude/settings/unknown-model"
                && d.severity == agentlint_core::Severity::Warning),
            "expected unknown-model warning, got: {diags:?}"
        );
    }

    #[test]
    fn known_model_is_clean() {
        let src = r#"{"model": "claude-sonnet-4-6"}"#;
        assert_clean(&SettingsValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn top_level_env_op_uri_is_warning() {
        let src = r#"{"env": {"API_KEY": "op://Personal/item/field"}}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "claude/settings/env-unresolved-op-ref"
                    && d.severity == agentlint_core::Severity::Warning),
            "expected env-unresolved-op-ref warning, got: {diags:?}"
        );
    }

    #[test]
    fn top_level_env_regular_value_is_clean() {
        let src = r#"{"env": {"LOG_LEVEL": "debug"}}"#;
        assert_clean(&SettingsValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn mcp_servers_in_settings_missing_transport_is_error() {
        let src = r#"{"mcpServers": {"s": {"args": ["foo"]}}}"#;
        let diags = SettingsValidator::validate(Path::new(PATH), src);
        assert_error_contains(&diags, "transport");
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
