use agentlint_core::{Diagnostic, Difficulty, Validator};
use std::path::Path;

pub struct CodexValidator;

const MIN_NON_EMPTY_LINES: usize = 5;
const MIN_NON_WS_CHARS: usize = 100;

impl Validator for CodexValidator {
    fn patterns(&self) -> &[&str] {
        &["AGENTS.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        if src.trim().is_empty() {
            return vec![
                Diagnostic::error(path, 1, 1, "AGENTS.md is empty")
                    .with_rule("codex/content/empty", Difficulty::Easy),
            ];
        }

        let mut diags = Vec::new();

        // codex/content/no-heading: no line starting with `#`
        let has_heading = src.lines().any(|l| l.starts_with('#'));
        if !has_heading {
            diags.push(
                Diagnostic::warning(path, 1, 1, "AGENTS.md has no markdown headings")
                    .with_rule("codex/content/no-heading", Difficulty::Painful),
            );
        }

        // codex/content/too-sparse: fewer than 5 non-empty lines OR fewer than 100 non-ws chars
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
                .with_rule("codex/content/too-sparse", Difficulty::Painful),
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
        let v = CodexValidator;
        let src = "# Agents\n\nThis is a well-structured agents file.\n\
                   It has multiple lines of content.\n\
                   This line adds more context.\n\
                   And another line for good measure.\n\
                   Final line to ensure sufficient content here.";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn empty_file_is_error() {
        let v = CodexValidator;
        let diags = v.validate(Path::new("AGENTS.md"), "");
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("empty"));
    }

    #[test]
    fn whitespace_only_is_error() {
        let v = CodexValidator;
        let diags = v.validate(Path::new("AGENTS.md"), "   \n\t\n  ");
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("empty"));
    }

    // --- no-heading rule ---

    #[test]
    fn no_heading_fires_when_missing() {
        let v = CodexValidator;
        // Enough content to not trigger too-sparse, but no headings
        let src = "This is a description without any headings.\n\
                   It has plenty of lines to read through.\n\
                   There is no section structure here at all.\n\
                   The content is just a wall of text flowing.\n\
                   This is the fifth line of text content now.";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"codex/content/no-heading"),
            "expected no-heading diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn no_heading_clean_when_heading_present() {
        let v = CodexValidator;
        let src = "# Overview\n\nThis file has a heading and sufficient content.\n\
                   More lines of content here to pass the sparse check.\n\
                   And more content to ensure we have enough characters.\n\
                   Final line with enough text to be over one hundred chars.";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        let heading_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "codex/content/no-heading")
            .collect();
        assert!(heading_diags.is_empty());
    }

    // --- too-sparse rule ---

    #[test]
    fn too_sparse_fires_when_few_lines() {
        let v = CodexValidator;
        // Only 3 non-empty lines, well under 5
        let src = "# Agents\nLine two.\nLine three.";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"codex/content/too-sparse"),
            "expected too-sparse diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn too_sparse_fires_when_few_chars() {
        let v = CodexValidator;
        // 5 non-empty lines but very short — under 100 non-ws chars
        let src = "# A\nb\nc\nd\ne";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        let rules: Vec<_> = diags.iter().map(|d| d.rule).collect();
        assert!(
            rules.contains(&"codex/content/too-sparse"),
            "expected too-sparse diagnostic, got: {rules:?}"
        );
    }

    #[test]
    fn too_sparse_clean_when_sufficient_content() {
        let v = CodexValidator;
        let src = "# Agents\n\nThis file has enough content to pass.\n\
                   It has at least five non-empty lines throughout.\n\
                   This is the fourth line of meaningful content here.\n\
                   Fifth line ensures we meet the line count threshold.\n\
                   And this pushes the character count well past one hundred.";
        let diags = v.validate(Path::new("AGENTS.md"), src);
        let sparse_diags: Vec<_> = diags
            .iter()
            .filter(|d| d.rule == "codex/content/too-sparse")
            .collect();
        assert!(sparse_diags.is_empty());
    }
}
