use agentlint_core::{Diagnostic, Difficulty};
use std::path::Path;

pub struct MetaValidator;

impl MetaValidator {
    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        // #43: CLAUDE.md with no markdown headings.
        let has_heading = src.lines().any(|l| l.starts_with('#'));
        if !has_heading {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    "CLAUDE.md has no markdown headings; consider adding section headings",
                )
                .with_rule("claude/meta/claude-md-no-heading", Difficulty::Painful),
            );
        }

        // #44: CLAUDE.md exceeding 500 lines.
        let line_count = src.lines().count();
        if line_count > 500 {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    format!(
                        "CLAUDE.md is {line_count} lines; consider splitting into smaller files \
                         (limit: 500)"
                    ),
                )
                .with_rule("claude/meta/claude-md-too-long", Difficulty::Painful),
            );
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
    use agentlint_core::{Difficulty, Severity};
    use std::path::Path;

    const PATH: &str = "CLAUDE.md";

    fn make_src(lines: usize, with_heading: bool) -> String {
        let mut out = String::new();
        if with_heading {
            out.push_str("# Overview\n");
            for _ in 1..lines {
                out.push_str("line\n");
            }
        } else {
            for _ in 0..lines {
                out.push_str("line\n");
            }
        }
        out
    }

    #[test]
    fn valid_claude_md_no_diagnostics() {
        let src = "# Overview\n\nSome content here.\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        assert!(
            diags.is_empty(),
            "valid CLAUDE.md should produce no diagnostics"
        );
    }

    // ---- #43: claude-md-no-heading ----

    #[test]
    fn no_heading_emits_warning() {
        let src = "Some content without any heading.\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        let hit = diags
            .iter()
            .find(|d| d.rule == "claude/meta/claude-md-no-heading");
        assert!(hit.is_some(), "expected claude-md-no-heading warning");
        let d = hit.unwrap();
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.difficulty, Difficulty::Painful);
    }

    #[test]
    fn heading_in_middle_clears_warning() {
        let src = "intro\n\n## Section\n\ncontent\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .all(|d| d.rule != "claude/meta/claude-md-no-heading"),
            "heading in middle should suppress no-heading warning"
        );
    }

    // ---- #44: claude-md-too-long ----

    #[test]
    fn over_500_lines_emits_warning() {
        let src = make_src(501, true);
        let diags = MetaValidator::validate(Path::new(PATH), &src);
        let hit = diags
            .iter()
            .find(|d| d.rule == "claude/meta/claude-md-too-long");
        assert!(hit.is_some(), "expected claude-md-too-long warning");
        let d = hit.unwrap();
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.difficulty, Difficulty::Painful);
        assert!(d.message.contains("501"));
    }

    #[test]
    fn exactly_500_lines_is_clean() {
        let src = make_src(500, true);
        let diags = MetaValidator::validate(Path::new(PATH), &src);
        assert!(
            diags
                .iter()
                .all(|d| d.rule != "claude/meta/claude-md-too-long"),
            "500-line file should not trigger too-long warning"
        );
    }

    #[test]
    fn both_rules_can_fire_together() {
        // 501 lines, no heading
        let src = make_src(501, false);
        let diags = MetaValidator::validate(Path::new(PATH), &src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "claude/meta/claude-md-no-heading"),
            "no-heading should fire"
        );
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "claude/meta/claude-md-too-long"),
            "too-long should fire"
        );
    }
}
