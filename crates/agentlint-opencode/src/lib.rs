use agentlint_core::{Diagnostic, Difficulty, Validator};
use std::path::Path;

const MIN_NON_EMPTY_LINES: usize = 5;
const MIN_NON_WS_CHARS: usize = 100;

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

        let mut diags = Vec::new();

        // opencode/content/no-heading: no line starting with `#`
        let has_heading = src.lines().any(|l| l.starts_with('#'));
        if !has_heading {
            diags.push(
                Diagnostic::warning(path, 1, 1, "AGENTS.md has no markdown headings")
                    .with_rule("opencode/content/no-heading", Difficulty::Painful),
            );
        }

        // opencode/content/no-commands-section: has headings but none mention commands keywords
        if has_heading {
            const COMMAND_KEYWORDS: &[&str] =
                &["build", "test", "run", "lint", "commands", "setup"];
            let has_command_heading = src.lines().filter(|l| l.starts_with('#')).any(|l| {
                let lower = l.to_lowercase();
                let words: Vec<&str> = lower
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| !w.is_empty())
                    .collect();
                COMMAND_KEYWORDS.iter().any(|kw| words.contains(kw))
            });
            if !has_command_heading {
                diags.push(
                    Diagnostic::warning(
                        path,
                        1,
                        1,
                        "AGENTS.md has no build/test commands section; add a heading like \
                         `## Commands` or `## Build` so OpenCode knows how to run the project",
                    )
                    .with_rule("opencode/content/no-commands-section", Difficulty::Painful),
                );
            }
        }

        // opencode/content/too-sparse: fewer than 5 non-empty lines OR fewer than 100 non-ws chars
        let non_empty_lines = src.lines().filter(|l| !l.trim().is_empty()).count();
        let non_ws_chars = src.chars().filter(|c| !c.is_whitespace()).count();
        if non_empty_lines < MIN_NON_EMPTY_LINES || non_ws_chars < MIN_NON_WS_CHARS {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    "AGENTS.md is too sparse to provide meaningful guidance",
                )
                .with_rule("opencode/content/too-sparse", Difficulty::Painful),
            );
        }

        diags
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
    fn agents_non_empty_with_heading_and_commands_is_clean() {
        let src = "# Overview\n\n## Commands\n\nRun `cargo test`.\n\
                   It has multiple lines of content here.\n\
                   This line adds more context to the file.\n\
                   And another line for good measure here.\n\
                   Final line to ensure sufficient content.";
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), src);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
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

    // --- no-heading rule ---

    #[test]
    fn no_heading_fires_when_missing() {
        let src = "This is a description without any headings.\n\
                   It has plenty of lines to read through.\n\
                   There is no section structure here at all.\n\
                   The content is just a wall of text flowing.\n\
                   This is the fifth line of text content now.";
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"opencode/content/no-heading"),
            "expected no-heading diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn no_heading_clean_when_heading_present() {
        let src = "# Overview\n\n## Commands\n\nThis file has a heading and sufficient content.\n\
                   More lines of content here to pass the sparse check.\n\
                   And more content to ensure we have enough characters.\n\
                   Final line with enough text to be over one hundred chars.";
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), src);
        let heading_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "opencode/content/no-heading")
            .collect();
        assert!(heading_diags.is_empty(), "unexpected: {heading_diags:?}");
    }

    // --- too-sparse rule ---

    #[test]
    fn too_sparse_fires_when_few_lines() {
        let src = "# Agents\nLine two.\nLine three.";
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"opencode/content/too-sparse"),
            "expected too-sparse diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn too_sparse_fires_when_few_chars() {
        let src = "# A\nb\nc\nd\ne";
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"opencode/content/too-sparse"),
            "expected too-sparse diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn too_sparse_clean_when_sufficient_content() {
        let src = "# Agents\n\n## Commands\n\nThis file has enough content to pass.\n\
                   It has at least five non-empty lines throughout.\n\
                   This is the fourth line of meaningful content.\n\
                   Fifth line ensures we meet the line count threshold.\n\
                   And this pushes the character count well past one hundred.";
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), src);
        let sparse_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "opencode/content/too-sparse")
            .collect();
        assert!(sparse_diags.is_empty(), "unexpected: {sparse_diags:?}");
    }

    // --- no-commands-section rule ---

    #[test]
    fn no_commands_section_fires_when_no_command_heading() {
        let src = "# Overview\n\n## Background\n\nThis project does something interesting.\n\
                   It has multiple sections but none about running commands.\n\
                   There is no build section here at all anywhere.\n\
                   There is no test section here either in this file.\n\
                   And no setup or lint section anywhere in this file.";
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"opencode/content/no-commands-section"),
            "expected no-commands-section diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn no_commands_section_clean_with_commands_heading() {
        let src = "# Overview\n\n## Commands\n\nRun `cargo test`.\n\
                   More content here to pass the sparse check.\n\
                   And more lines to ensure sufficient content here.\n\
                   Final line with enough text to be over one hundred chars total.";
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), src);
        let cmd_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "opencode/content/no-commands-section")
            .collect();
        assert!(cmd_diags.is_empty(), "unexpected: {cmd_diags:?}");
    }

    #[test]
    fn no_commands_section_clean_with_build_heading() {
        let src = "# Overview\n\n## Build\n\nRun `make build`.\n\
                   More content here to pass the sparse check.\n\
                   And more lines to ensure sufficient content here.\n\
                   Final line with enough text to be over one hundred chars total.";
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), src);
        let cmd_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "opencode/content/no-commands-section")
            .collect();
        assert!(cmd_diags.is_empty(), "unexpected: {cmd_diags:?}");
    }

    #[test]
    fn no_commands_section_not_fired_when_no_heading() {
        let src = "This file has no headings whatsoever at all.\n\
                   It is just a wall of text with no structure.\n\
                   There are enough lines here to avoid sparse check.\n\
                   Fourth line of content to pass line count threshold.\n\
                   Fifth line with enough chars to exceed one hundred total non-ws chars here.";
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), src);
        let cmd_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "opencode/content/no-commands-section")
            .collect();
        assert!(
            cmd_diags.is_empty(),
            "should not fire without headings: {cmd_diags:?}"
        );
    }

    #[test]
    fn no_commands_section_and_too_sparse_both_fire() {
        let src = "# Overview\n\nSome text.\nMore text.";
        let diags = AgentsMarkdownValidator.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"opencode/content/no-commands-section"),
            "expected no-commands-section, got: {rules:?}"
        );
        assert!(
            rules.contains(&"opencode/content/too-sparse"),
            "expected too-sparse, got: {rules:?}"
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
