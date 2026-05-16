use agentlint_core::{Diagnostic, Difficulty};
use std::path::Path;

pub struct McpValidator;

/// Known keys inside a server entry object.
const KNOWN_SERVER_KEYS: &[&str] = &["command", "args", "env", "type", "url", "headers"];

/// Validate a single MCP server entry object.
///
/// `name` is the server name (used in diagnostic messages).
/// `server` is the parsed JSON object for that entry.
/// Diagnostics are appended to `diags`.
pub fn validate_server_entry(
    path: &Path,
    name: &str,
    server: &serde_json::Map<String, serde_json::Value>,
    diags: &mut Vec<Diagnostic>,
) {
    let has_command = server.get("command").is_some();
    let has_url = server.get("url").is_some();
    if !has_command && !has_url {
        diags.push(
            Diagnostic::error(
                path,
                1,
                1,
                format!(
                    "mcpServers.{name}: server entry must have 'command' (stdio) \
                     or 'url' (HTTP/SSE) transport"
                ),
            )
            .with_rule("claude/mcp/missing-transport", Difficulty::Easy),
        );
    }

    if let Some(cmd) = server.get("command").and_then(|v| v.as_str())
        && cmd.trim().is_empty()
    {
        diags.push(
            Diagnostic::error(
                path,
                1,
                1,
                format!("mcpServers.{name}: 'command' must not be empty"),
            )
            .with_rule("claude/mcp/empty-command", Difficulty::Easy),
        );
    }

    if let Some(cmd) = server.get("command").and_then(|v| v.as_str())
        && (cmd.starts_with("./") || cmd.starts_with("../"))
    {
        diags.push(
            Diagnostic::warning(
                path,
                1,
                1,
                format!(
                    "mcpServers.{name}: 'command' is a relative path ({cmd:?}); \
                     relative paths break when Claude Code is launched from a \
                     different working directory — use an absolute path instead"
                ),
            )
            .with_rule("claude/mcp/relative-command", Difficulty::Hard),
        );
    }

    // Detect unconstrained HTTP fetch tools.
    {
        let name_lc = name.to_lowercase();
        let cmd_lc = server
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        let args_lc = server
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_lowercase()
            })
            .unwrap_or_default();

        let name_matches = name_lc.contains("fetch") || name_lc.contains("http");
        let cmd_matches = cmd_lc.contains("mcp-server-fetch")
            || cmd_lc.contains("mcp-fetch")
            || args_lc.contains("mcp-server-fetch")
            || args_lc.contains("mcp-fetch");

        if name_matches || cmd_matches {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    format!(
                        "mcpServers.{name}: appears to be an unconstrained HTTP fetch tool; \
                         fetch MCP servers can be used to exfiltrate data or make arbitrary \
                         network requests — ensure this is intentional and scoped"
                    ),
                )
                .with_rule("claude/mcp/fetch-server", Difficulty::Hard),
            );
        }
    }

    if let Some(env) = server.get("env").and_then(|v| v.as_object()) {
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
                            "mcpServers.{name}.env.{key}: op:// URI will not \
                             resolve in Claude's shell context; use \
                             'apiKeyHelper' in settings.json or pre-resolve \
                             the secret before launch"
                        ),
                    )
                    .with_rule("claude/mcp/op-uri-in-env", Difficulty::Hard),
                );
            }
        }
    }

    for key in server.keys() {
        if !KNOWN_SERVER_KEYS.contains(&key.as_str()) {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    format!("mcpServers.{name}: unknown field '{key}'"),
                )
                .with_rule("claude/mcp/unknown-server-field", Difficulty::Painful),
            );
        }
    }
}

impl McpValidator {
    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        let value: serde_json::Value = match serde_json::from_str(src) {
            Ok(v) => v,
            Err(e) => {
                return vec![
                    Diagnostic::error(path, 1, 1, format!("invalid JSON: {e}"))
                        .with_rule("claude/mcp/invalid-json", Difficulty::Easy),
                ];
            }
        };

        let obj = match value.as_object() {
            Some(o) => o,
            None => {
                return vec![
                    Diagnostic::error(path, 1, 1, ".mcp.json must be a JSON object")
                        .with_rule("claude/mcp/invalid-json", Difficulty::Easy),
                ];
            }
        };

        let mut diags = Vec::new();

        let servers = match obj.get("mcpServers").and_then(|v| v.as_object()) {
            Some(s) => s,
            None => {
                diags.push(
                    Diagnostic::error(
                        path,
                        1,
                        1,
                        ".mcp.json must have a top-level 'mcpServers' object",
                    )
                    .with_rule("claude/mcp/missing-mcpservers", Difficulty::Easy),
                );
                return diags;
            }
        };

        for (name, entry) in servers {
            let Some(server) = entry.as_object() else {
                diags.push(
                    Diagnostic::error(
                        path,
                        1,
                        1,
                        format!("mcpServers.{name}: server entry must be a JSON object"),
                    )
                    .with_rule("claude/mcp/invalid-server-entry", Difficulty::Easy),
                );
                continue;
            };
            validate_server_entry(path, name, server, &mut diags);
        }

        diags
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentlint_core::testing::{assert_clean, assert_error_contains};
    use std::path::Path;

    const PATH: &str = ".mcp.json";

    #[test]
    fn valid_stdio_server_is_clean() {
        let src = r#"{
            "mcpServers": {
                "my-server": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
                }
            }
        }"#;
        assert_clean(&McpValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn valid_url_server_is_clean() {
        let src = r#"{"mcpServers": {"remote": {"url": "http://localhost:3000/sse"}}}"#;
        assert_clean(&McpValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn invalid_json_is_error() {
        let diags = McpValidator::validate(Path::new(PATH), "not json");
        assert_error_contains(&diags, "invalid JSON");
    }

    #[test]
    fn missing_mcpservers_key_is_error() {
        let diags = McpValidator::validate(Path::new(PATH), r#"{"foo": {}}"#);
        assert_error_contains(&diags, "mcpServers");
    }

    #[test]
    fn missing_transport_is_error() {
        let src = r#"{"mcpServers": {"bad": {"args": ["foo"]}}}"#;
        let diags = McpValidator::validate(Path::new(PATH), src);
        assert_error_contains(&diags, "transport");
    }

    #[test]
    fn empty_command_is_error() {
        let src = r#"{"mcpServers": {"bad": {"command": "  "}}}"#;
        let diags = McpValidator::validate(Path::new(PATH), src);
        assert_error_contains(&diags, "must not be empty");
    }

    #[test]
    fn op_uri_in_env_is_warning() {
        let src = r#"{
            "mcpServers": {
                "s": {
                    "command": "node",
                    "env": {"API_KEY": "op://Personal/item/field"}
                }
            }
        }"#;
        let diags = McpValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("op://")
                    && d.severity == agentlint_core::Severity::Warning),
            "expected op:// warning, got: {diags:?}"
        );
    }

    #[test]
    fn relative_command_dot_slash_is_warning() {
        let src = r#"{"mcpServers": {"s": {"command": "./scripts/mcp-server"}}}"#;
        let diags = McpValidator::validate(Path::new(PATH), src);
        assert!(
            diags.iter().any(|d| d.rule == "claude/mcp/relative-command"
                && d.severity == agentlint_core::Severity::Warning),
            "expected relative-command warning, got: {diags:?}"
        );
    }

    #[test]
    fn relative_command_dot_dot_slash_is_warning() {
        let src = r#"{"mcpServers": {"s": {"command": "../bin/server"}}}"#;
        let diags = McpValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "claude/mcp/relative-command"),
            "expected relative-command warning, got: {diags:?}"
        );
    }

    #[test]
    fn absolute_command_is_clean() {
        let src = r#"{"mcpServers": {"s": {"command": "/usr/local/bin/server"}}}"#;
        assert_clean(&McpValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn fetch_server_name_is_warning() {
        let src = r#"{"mcpServers": {"mcp-fetch": {"command": "node", "args": ["server.js"]}}}"#;
        let diags = McpValidator::validate(Path::new(PATH), src);
        assert!(
            diags.iter().any(|d| d.rule == "claude/mcp/fetch-server"
                && d.severity == agentlint_core::Severity::Warning),
            "expected fetch-server warning, got: {diags:?}"
        );
    }

    #[test]
    fn fetch_server_command_is_warning() {
        let src =
            r#"{"mcpServers": {"my-server": {"command": "npx", "args": ["mcp-server-fetch"]}}}"#;
        let diags = McpValidator::validate(Path::new(PATH), src);
        assert!(
            diags.iter().any(|d| d.rule == "claude/mcp/fetch-server"),
            "expected fetch-server warning, got: {diags:?}"
        );
    }

    #[test]
    fn http_in_server_name_is_warning() {
        let src = r#"{"mcpServers": {"http-proxy": {"command": "node", "args": ["proxy.js"]}}}"#;
        let diags = McpValidator::validate(Path::new(PATH), src);
        assert!(
            diags.iter().any(|d| d.rule == "claude/mcp/fetch-server"),
            "expected fetch-server warning, got: {diags:?}"
        );
    }

    #[test]
    fn env_with_regular_value_is_clean() {
        let src = r#"{
            "mcpServers": {
                "s": {
                    "command": "node",
                    "env": {"LOG_LEVEL": "debug"}
                }
            }
        }"#;
        assert_clean(&McpValidator::validate(Path::new(PATH), src));
    }

    #[test]
    fn unknown_server_field_is_warning() {
        let src = r#"{"mcpServers": {"s": {"command": "node", "timeout": 30}}}"#;
        let diags = McpValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("unknown field 'timeout'")),
            "expected unknown-field warning, got: {diags:?}"
        );
    }
}
