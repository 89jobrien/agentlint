use agentlint_core::{Diagnostic, Difficulty, Validator};
use std::path::Path;

pub struct PiValidator;

const MIN_NON_EMPTY_LINES: usize = 5;
const MIN_NON_WS_CHARS: usize = 100;

impl Validator for PiValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md", "SYSTEM.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());

        if src.trim().is_empty() {
            return vec![
                Diagnostic::error(path, 1, 1, format!("{filename} is empty"))
                    .with_rule("pi/content/empty", Difficulty::Easy),
            ];
        }

        let mut diags = Vec::new();

        // pi/content/no-heading: no line starting with `#`
        let has_heading = src.lines().any(|l| l.starts_with('#'));
        if !has_heading {
            diags.push(
                Diagnostic::warning(path, 1, 1, format!("{filename} has no markdown headings"))
                    .with_rule("pi/content/no-heading", Difficulty::Painful),
            );
        }

        // pi/content/too-sparse: fewer than 5 non-empty lines OR fewer than 100 non-ws chars
        let non_empty_lines = src.lines().filter(|l| !l.trim().is_empty()).count();
        let non_ws_chars = src.chars().filter(|c| !c.is_whitespace()).count();
        if non_empty_lines < MIN_NON_EMPTY_LINES || non_ws_chars < MIN_NON_WS_CHARS {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    format!("{filename} is too sparse to provide meaningful guidance"),
                )
                .with_rule("pi/content/too-sparse", Difficulty::Painful),
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
    fn agents_non_empty_is_clean() {
        let v = PiValidator;
        let src = "# Agents\n\n## Overview\n\nSome content here.\n\
                   More content to pass sparse check.\n\
                   Third line of meaningful content.\n\
                   Fourth line ensures we meet the threshold.\n\
                   Fifth line pushes character count well past one hundred.";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn system_non_empty_is_clean() {
        let v = PiValidator;
        let src = "# System\n\n## Overview\n\nSome content here.\n\
                   More content to pass sparse check.\n\
                   Third line of meaningful content.\n\
                   Fourth line ensures we meet the threshold.\n\
                   Fifth line pushes character count well past one hundred.";
        let diags = v.validate(Path::new("SYSTEM.md"), src);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
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

    // --- no-heading rule ---

    #[test]
    fn no_heading_fires_for_agents_when_missing() {
        let v = PiValidator;
        let src = "This is a description without any headings.\n\
                   It has plenty of lines to read through.\n\
                   There is no section structure here at all.\n\
                   The content is just a wall of text flowing.\n\
                   This is the fifth line of text content now.";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"pi/content/no-heading"),
            "expected no-heading diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn no_heading_fires_for_system_when_missing() {
        let v = PiValidator;
        let src = "This is a description without any headings.\n\
                   It has plenty of lines to read through.\n\
                   There is no section structure here at all.\n\
                   The content is just a wall of text flowing.\n\
                   This is the fifth line of text content now.";
        let diags = v.validate(Path::new("SYSTEM.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"pi/content/no-heading"),
            "expected no-heading diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn no_heading_clean_when_heading_present() {
        let v = PiValidator;
        let src = "# Overview\n\nThis file has a heading and sufficient content.\n\
                   More lines of content here to pass the sparse check.\n\
                   And more content to ensure we have enough characters.\n\
                   Final line with enough text to be over one hundred chars.";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        let heading_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "pi/content/no-heading")
            .collect();
        assert!(heading_diags.is_empty(), "unexpected: {heading_diags:?}");
    }

    // --- too-sparse rule ---

    #[test]
    fn too_sparse_fires_for_agents_when_few_lines() {
        let v = PiValidator;
        let src = "# Agents\nLine two.\nLine three.";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"pi/content/too-sparse"),
            "expected too-sparse diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn too_sparse_fires_for_system_when_few_lines() {
        let v = PiValidator;
        let src = "# System\nLine two.\nLine three.";
        let diags = v.validate(Path::new("SYSTEM.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"pi/content/too-sparse"),
            "expected too-sparse diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn too_sparse_fires_when_few_chars() {
        let v = PiValidator;
        // 5 non-empty lines but very short — under 100 non-ws chars
        let src = "# A\nb\nc\nd\ne";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"pi/content/too-sparse"),
            "expected too-sparse diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn too_sparse_clean_when_sufficient_content() {
        let v = PiValidator;
        let src = "# Agents\n\nThis file has enough content to pass.\n\
                   It has at least five non-empty lines throughout.\n\
                   This is the fourth line of meaningful content here.\n\
                   Fifth line ensures we meet the line count threshold.\n\
                   And this pushes the character count well past one hundred.";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        let sparse_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "pi/content/too-sparse")
            .collect();
        assert!(sparse_diags.is_empty(), "unexpected: {sparse_diags:?}");
    }

    #[test]
    fn no_heading_and_too_sparse_both_fire() {
        let v = PiValidator;
        let src = "Some text.\nMore text.";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"pi/content/no-heading"),
            "expected no-heading, got: {rules:?}"
        );
        assert!(
            rules.contains(&"pi/content/too-sparse"),
            "expected too-sparse, got: {rules:?}"
        );
    }
}
