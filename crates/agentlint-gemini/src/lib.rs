use agentlint_core::{Diagnostic, Difficulty, Validator};
use std::path::Path;

pub struct GeminiValidator;

const MIN_NON_EMPTY_LINES: usize = 5;
const MIN_NON_WS_CHARS: usize = 100;

impl Validator for GeminiValidator {
    fn patterns(&self) -> &[&str] {
        &["GEMINI.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if src.trim().is_empty() {
            return vec![
                Diagnostic::error(path, 1, 1, "GEMINI.md is empty")
                    .with_rule("gemini/content/empty", Difficulty::Easy),
            ];
        }

        let mut diags = Vec::new();

        // gemini/content/no-heading: no line starting with `#`
        let has_heading = src.lines().any(|l| l.starts_with('#'));
        if !has_heading {
            diags.push(
                Diagnostic::warning(path, 1, 1, "GEMINI.md has no markdown headings")
                    .with_rule("gemini/content/no-heading", Difficulty::Painful),
            );
        }

        // gemini/content/no-commands-section: has headings but none mention build/test/run/lint/commands/setup
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
                        "GEMINI.md has no build/test commands section; add a heading like \
                         `## Commands` or `## Build` so Gemini knows how to run the project",
                    )
                    .with_rule("gemini/content/no-commands-section", Difficulty::Painful),
                );
            }
        }

        // gemini/content/too-sparse: fewer than 5 non-empty lines OR fewer than 100 non-ws chars
        let non_empty_lines = src.lines().filter(|l| !l.trim().is_empty()).count();
        let non_ws_chars = src.chars().filter(|c| !c.is_whitespace()).count();
        if non_empty_lines < MIN_NON_EMPTY_LINES || non_ws_chars < MIN_NON_WS_CHARS {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    "GEMINI.md is too sparse to provide meaningful guidance",
                )
                .with_rule("gemini/content/too-sparse", Difficulty::Painful),
            );
        }

        diags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn non_empty_with_heading_is_clean() {
        let v = GeminiValidator;
        let src = "# Gemini\n\n## Commands\n\nRun `cargo test`.\n\
                   It has multiple lines of content.\n\
                   This line adds more context.\n\
                   And another line for good measure.\n\
                   Final line to ensure sufficient content here.";
        let diags = v.validate(Path::new("GEMINI.md"), src);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn non_empty_is_clean() {
        let v = GeminiValidator;
        let diags = v.validate(
            Path::new("GEMINI.md"),
            "# Gemini\n\n## Commands\n\nSome content.\nMore lines here.\n\
             Enough text to exceed one hundred non-whitespace characters total for sure.",
        );
        assert!(diags.is_empty());
    }

    #[test]
    fn empty_file_is_error() {
        let v = GeminiValidator;
        let diags = v.validate(Path::new("GEMINI.md"), "");
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("empty"));
    }

    #[test]
    fn whitespace_only_is_error() {
        let v = GeminiValidator;
        let diags = v.validate(Path::new("GEMINI.md"), "   \n\t\n  ");
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("empty"));
    }

    // --- no-heading rule ---

    #[test]
    fn no_heading_fires_when_missing() {
        let v = GeminiValidator;
        let src = "This is a description without any headings.\n\
                   It has plenty of lines to read through.\n\
                   There is no section structure here at all.\n\
                   The content is just a wall of text flowing.\n\
                   This is the fifth line of text content now.";
        let diags = v.validate(Path::new("GEMINI.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"gemini/content/no-heading"),
            "expected no-heading diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn no_heading_clean_when_heading_present() {
        let v = GeminiValidator;
        let src = "# Overview\n\n## Commands\n\nThis file has a heading and sufficient content.\n\
                   More lines of content here to pass the sparse check.\n\
                   And more content to ensure we have enough characters.\n\
                   Final line with enough text to be over one hundred chars.";
        let diags = v.validate(Path::new("GEMINI.md"), src);
        let heading_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "gemini/content/no-heading")
            .collect();
        assert!(heading_diags.is_empty());
    }

    // --- no-commands-section rule ---

    #[test]
    fn no_commands_section_fires_when_no_command_heading() {
        let v = GeminiValidator;
        let src = "# Overview\n\n## Background\n\nThis project does something interesting.\n\
                   It has multiple sections but none about running commands.\n\
                   There is no build section here.\n\
                   There is no test section here either.\n\
                   And no setup or lint section anywhere in this file.";
        let diags = v.validate(Path::new("GEMINI.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"gemini/content/no-commands-section"),
            "expected no-commands-section diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn no_commands_section_clean_with_commands_heading() {
        let v = GeminiValidator;
        let src = "# Overview\n\n## Commands\n\nRun `cargo test`.\n\
                   More content here to pass the sparse check.\n\
                   And more lines to ensure sufficient content.\n\
                   Final line with enough text to be over one hundred chars total.";
        let diags = v.validate(Path::new("GEMINI.md"), src);
        let cmd_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "gemini/content/no-commands-section")
            .collect();
        assert!(cmd_diags.is_empty(), "unexpected: {cmd_diags:?}");
    }

    #[test]
    fn no_commands_section_clean_with_build_heading() {
        let v = GeminiValidator;
        let src = "# Overview\n\n## Build\n\nRun `make build`.\n\
                   More content here to pass the sparse check.\n\
                   And more lines to ensure sufficient content.\n\
                   Final line with enough text to be over one hundred chars total.";
        let diags = v.validate(Path::new("GEMINI.md"), src);
        let cmd_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "gemini/content/no-commands-section")
            .collect();
        assert!(cmd_diags.is_empty(), "unexpected: {cmd_diags:?}");
    }

    #[test]
    fn no_commands_section_not_fired_when_no_heading() {
        let v = GeminiValidator;
        let src = "This file has no headings whatsoever.\n\
                   It is just a wall of text with no structure.\n\
                   There are enough lines here to avoid sparse check.\n\
                   Fourth line of content to pass line count threshold.\n\
                   Fifth line with enough chars to exceed one hundred total non-ws chars here.";
        let diags = v.validate(Path::new("GEMINI.md"), src);
        let cmd_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "gemini/content/no-commands-section")
            .collect();
        assert!(
            cmd_diags.is_empty(),
            "should not fire without headings: {cmd_diags:?}"
        );
    }

    #[test]
    fn no_commands_section_and_too_sparse_both_fire() {
        let v = GeminiValidator;
        let src = "# Overview\n\nSome text.\nMore text.";
        let diags = v.validate(Path::new("GEMINI.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"gemini/content/no-commands-section"),
            "expected no-commands-section, got: {rules:?}"
        );
        assert!(
            rules.contains(&"gemini/content/too-sparse"),
            "expected too-sparse, got: {rules:?}"
        );
    }

    // --- too-sparse rule ---

    #[test]
    fn too_sparse_fires_when_few_lines() {
        let v = GeminiValidator;
        let src = "# Gemini\nLine two.\nLine three.";
        let diags = v.validate(Path::new("GEMINI.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"gemini/content/too-sparse"),
            "expected too-sparse diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn too_sparse_fires_when_few_chars() {
        let v = GeminiValidator;
        let src = "# A\nb\nc\nd\ne";
        let diags = v.validate(Path::new("GEMINI.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"gemini/content/too-sparse"),
            "expected too-sparse diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn too_sparse_clean_when_sufficient_content() {
        let v = GeminiValidator;
        let src = "# Gemini\n\n## Commands\n\nThis file has enough content to pass.\n\
                   It has at least five non-empty lines throughout.\n\
                   This is the fourth line of meaningful content here.\n\
                   Fifth line ensures we meet the line count threshold.\n\
                   And this pushes the character count well past one hundred.";
        let diags = v.validate(Path::new("GEMINI.md"), src);
        let sparse_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "gemini/content/too-sparse")
            .collect();
        assert!(sparse_diags.is_empty());
    }
}
