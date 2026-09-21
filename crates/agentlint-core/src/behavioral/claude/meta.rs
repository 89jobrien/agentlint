use crate::{Diagnostic, Difficulty};
use std::path::Path;

pub struct MetaValidator;

/// Maximum lines before a CLAUDE.md triggers a too-long warning.
const MAX_CLAUDE_MD_LINES: usize = 500;

impl MetaValidator {
    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        // Empty or whitespace-only file.
        if src.trim().is_empty() {
            diags.push(
                Diagnostic::error(
                    path,
                    1,
                    1,
                    "CLAUDE.md is empty; add project instructions or remove the file",
                )
                .with_rule("claude/meta/empty-file", Difficulty::Easy),
            );
            return diags;
        }

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
        if line_count > MAX_CLAUDE_MD_LINES {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    format!(
                        "CLAUDE.md is {line_count} lines; consider splitting into smaller files \
                         (limit: {MAX_CLAUDE_MD_LINES})"
                    ),
                )
                .with_rule("claude/meta/claude-md-too-long", Difficulty::Painful),
            );
        }

        // Duplicate headings at the same level.
        check_duplicate_headings(path, src, &mut diags);

        // Missing commands/build section.
        check_no_commands_section(path, src, &mut diags);

        diags
    }
}

/// Warn when two headings at the same level have identical text.
fn check_duplicate_headings(path: &Path, src: &str, diags: &mut Vec<Diagnostic>) {
    let mut seen: std::collections::HashMap<(usize, String), usize> =
        std::collections::HashMap::new();
    for (lineno, line) in src.lines().enumerate() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            continue;
        }
        let level = trimmed.chars().take_while(|&c| c == '#').count();
        let text = trimmed[level..].trim().to_lowercase();
        if text.is_empty() {
            continue;
        }
        let key = (level, text);
        if let Some(&first_line) = seen.get(&key) {
            diags.push(
                Diagnostic::warning(
                    path,
                    lineno + 1,
                    1,
                    format!(
                        "duplicate heading (first at line {}); duplicate sections confuse \
                         the agent — merge or rename",
                        first_line
                    ),
                )
                .with_rule("claude/meta/duplicate-heading", Difficulty::Hard),
            );
        } else {
            seen.insert(key, lineno + 1);
        }
    }
}

/// Warn when no build/commands section heading is present.
fn check_no_commands_section(path: &Path, src: &str, diags: &mut Vec<Diagnostic>) {
    const COMMAND_HEADINGS: &[&str] = &[
        "commands",
        "build",
        "build & test",
        "build and test",
        "development",
        "getting started",
        "usage",
        "quick start",
        "setup",
    ];
    let has_commands_section = src.lines().any(|line| {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            return false;
        }
        let level = trimmed.chars().take_while(|&c| c == '#').count();
        if level > 3 {
            return false;
        }
        let text = trimmed[level..].trim().to_lowercase();
        COMMAND_HEADINGS.iter().any(|h| text == *h)
    });
    if !has_commands_section {
        diags.push(
            Diagnostic::warning(
                path,
                1,
                1,
                "CLAUDE.md has no build/commands section (e.g. `## Commands`); \
                 agents perform better with explicit build and test instructions",
            )
            .with_rule("claude/meta/no-commands-section", Difficulty::Hard),
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Difficulty, Severity};
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
        let src = "# Overview\n\n## Commands\n\n```bash\ncargo test\n```\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        assert!(
            diags.is_empty(),
            "valid CLAUDE.md should produce no diagnostics, got: {diags:?}"
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

    // ---- empty-file ----

    #[test]
    fn empty_file_is_error() {
        let diags = MetaValidator::validate(Path::new(PATH), "");
        let hit = diags.iter().find(|d| d.rule == "claude/meta/empty-file");
        assert!(hit.is_some(), "expected empty-file error");
        assert_eq!(hit.unwrap().severity, Severity::Error);
    }

    #[test]
    fn whitespace_only_file_is_error() {
        let diags = MetaValidator::validate(Path::new(PATH), "   \n  \n  ");
        assert!(
            diags.iter().any(|d| d.rule == "claude/meta/empty-file"),
            "whitespace-only should trigger empty-file"
        );
    }

    #[test]
    fn empty_file_returns_early_no_other_diags() {
        let diags = MetaValidator::validate(Path::new(PATH), "");
        assert_eq!(diags.len(), 1, "empty file should only produce one diag");
    }

    // ---- duplicate-heading ----

    #[test]
    fn duplicate_heading_same_level_warns() {
        let src = "# Overview\n\ntext\n\n## Commands\n\nmore\n\n## Commands\n\ndup\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "claude/meta/duplicate-heading"),
            "expected duplicate-heading warning, got: {diags:?}"
        );
    }

    #[test]
    fn duplicate_heading_different_level_is_clean() {
        let src = "# Commands\n\n## Commands\n\ntext\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        assert!(
            !diags
                .iter()
                .any(|d| d.rule == "claude/meta/duplicate-heading"),
            "different heading levels should not trigger duplicate"
        );
    }

    #[test]
    fn duplicate_heading_case_insensitive() {
        let src = "# Overview\n\n## commands\n\ntext\n\n## Commands\n\ndup\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "claude/meta/duplicate-heading"),
            "case-insensitive duplicate should trigger"
        );
    }

    // ---- no-commands-section ----

    #[test]
    fn no_commands_section_warns() {
        let src = "# Overview\n\nJust some text.\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "claude/meta/no-commands-section"),
            "expected no-commands-section, got: {diags:?}"
        );
    }

    #[test]
    fn commands_heading_suppresses_warning() {
        let src = "# Overview\n\n## Commands\n\n```bash\ncargo test\n```\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        assert!(
            !diags
                .iter()
                .any(|d| d.rule == "claude/meta/no-commands-section"),
            "## Commands present — should not warn"
        );
    }

    #[test]
    fn build_heading_suppresses_warning() {
        let src = "# Overview\n\n## Build\n\n```bash\ncargo build\n```\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        assert!(
            !diags
                .iter()
                .any(|d| d.rule == "claude/meta/no-commands-section"),
            "## Build present — should not warn"
        );
    }

    #[test]
    fn development_heading_suppresses_warning() {
        let src = "# My Project\n\n## Development\n\nRun tests with pytest.\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        assert!(
            !diags
                .iter()
                .any(|d| d.rule == "claude/meta/no-commands-section"),
            "## Development present — should not warn"
        );
    }

    #[test]
    fn deep_heading_level_4_does_not_count() {
        let src = "# Overview\n\n#### Commands\n\ntext\n";
        let diags = MetaValidator::validate(Path::new(PATH), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "claude/meta/no-commands-section"),
            "h4 Commands should not suppress the warning"
        );
    }
}
