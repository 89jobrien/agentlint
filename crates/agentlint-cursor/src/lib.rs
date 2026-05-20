use agentlint_core::{Diagnostic, Difficulty, Validator};
use agentlint_frontmatter::{ParseError, parse};
use std::path::Path;

pub struct CursorValidator;

impl Validator for CursorValidator {
    fn patterns(&self) -> &[&str] {
        &[
            ".cursor/rules/**/*.mdc",
            ".cursor/rules/**/*.md",
            ".cursorrules",
        ]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        // Frontmatter is optional — only validate when the opening fence is present.
        if !src.starts_with("---\n") && !src.starts_with("---\r\n") {
            return vec![];
        }
        let fields = match parse(src) {
            Ok(f) => f,
            Err(ParseError::UnclosedFence) => {
                return vec![
                    Diagnostic::error(
                        path,
                        1,
                        1,
                        "unclosed frontmatter fence: missing closing '---'",
                    )
                    .with_rule("cursor/frontmatter/unclosed-fence", Difficulty::Easy),
                ];
            }
            Err(ParseError::NoFence) => return vec![],
        };

        let mut diags = Vec::new();

        // #39 — missing description
        if !fields.iter().any(|f| f.key == "description") {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    "missing 'description' field: Cursor cannot surface or auto-apply this rule \
                     without a description",
                )
                .with_rule("cursor/frontmatter/missing-description", Difficulty::Hard),
            );
        }

        // #49 — unknown frontmatter keys
        const KNOWN_KEYS: &[&str] = &["description", "globs", "alwaysApply"];
        for field in &fields {
            if !KNOWN_KEYS.contains(&field.key.as_str()) {
                diags.push(
                    Diagnostic::warning(
                        path,
                        field.line,
                        1,
                        format!(
                            "unknown frontmatter key '{}'; Cursor only recognises `description`, \
                             `globs`, and `alwaysApply`",
                            field.key
                        ),
                    )
                    .with_rule("cursor/frontmatter/unknown-key", Difficulty::Easy),
                );
            }
        }

        // #48 — never-fires: frontmatter present but rule can never auto-apply.
        // Only applies to .mdc/.md rules — .cursorrules does not support globs/alwaysApply.
        let is_mdc_rule = matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("mdc") | Some("md")
        );
        if is_mdc_rule {
            let always_apply = fields
                .iter()
                .find(|f| f.key == "alwaysApply")
                .map(|f| f.value.trim().eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let has_globs = fields
                .iter()
                .find(|f| f.key == "globs")
                .map(|f| !f.value.trim().is_empty())
                .unwrap_or(false);
            if !always_apply && !has_globs {
                diags.push(
                    Diagnostic::warning(
                        path,
                        1,
                        1,
                        "rule has no `globs` and `alwaysApply` is not set; Cursor will never \
                         auto-apply this rule — add a `globs` pattern or set `alwaysApply: true`",
                    )
                    .with_rule("cursor/frontmatter/never-fires", Difficulty::Easy),
                );
            }
        }

        // #40 — invalid globs
        if let Some(globs_field) = fields.iter().find(|f| f.key == "globs") {
            for segment in globs_field.value.split(',') {
                let seg = segment.trim();
                if seg.is_empty() {
                    diags.push(
                        Diagnostic::warning(
                            path,
                            globs_field.line,
                            1,
                            "invalid glob: empty segment in 'globs' field",
                        )
                        .with_rule("cursor/frontmatter/invalid-globs", Difficulty::Hard),
                    );
                    continue;
                }
                let open_brackets = seg.chars().filter(|&c| c == '[').count();
                let close_brackets = seg.chars().filter(|&c| c == ']').count();
                if open_brackets > close_brackets {
                    diags.push(
                        Diagnostic::warning(
                            path,
                            globs_field.line,
                            1,
                            format!("invalid glob '{seg}': unmatched '[' in 'globs' field"),
                        )
                        .with_rule("cursor/frontmatter/invalid-globs", Difficulty::Hard),
                    );
                }
                // Check for invalid escape sequences: backslash not followed by a valid char
                let chars: Vec<char> = seg.chars().collect();
                let mut i = 0;
                while i < chars.len() {
                    if chars[i] == '\\' {
                        let next = chars.get(i + 1).copied();
                        match next {
                            None | Some(' ') => {
                                diags.push(
                                    Diagnostic::warning(
                                        path,
                                        globs_field.line,
                                        1,
                                        format!(
                                            "invalid glob '{seg}': invalid escape sequence in \
                                             'globs' field"
                                        ),
                                    )
                                    .with_rule(
                                        "cursor/frontmatter/invalid-globs",
                                        Difficulty::Hard,
                                    ),
                                );
                                break;
                            }
                            _ => {
                                i += 1; // skip escaped char
                            }
                        }
                    }
                    i += 1;
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

    fn v() -> CursorValidator {
        CursorValidator
    }

    #[test]
    fn no_frontmatter_is_clean() {
        let diags = v().validate(Path::new("rule.md"), "# Hello\nsome content\n");
        assert!(diags.is_empty());
    }

    #[test]
    fn well_formed_frontmatter_is_clean() {
        let src = "---\ndescription: lint files\nglobs: **/*.rs\nalwaysApply: false\n---\n# Body\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        // globs is non-empty so never-fires does not trigger; all keys are known
        assert!(diags.is_empty(), "unexpected diags: {diags:?}");
    }

    #[test]
    fn unclosed_fence_is_error() {
        let src = "---\ntitle: test\n# no closing fence\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0].message.contains("unclosed"),
            "unexpected message: {}",
            diags[0].message
        );
    }

    // #39 — missing-description

    #[test]
    fn missing_description_emits_warning() {
        let src = "---\nglobs: **/*.rs\n---\n# Body\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("missing-description")),
            "expected missing-description, got: {diags:?}"
        );
    }

    #[test]
    fn description_present_no_missing_description_warning() {
        let src = "---\ndescription: does stuff\nglobs: **/*.rs\n---\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            !diags.iter().any(|d| d.rule.contains("missing-description")),
            "unexpected missing-description diagnostic"
        );
    }

    // #40 — invalid-globs

    #[test]
    fn valid_globs_are_clean() {
        let src = "---\ndescription: ok\nglobs: **/*.rs,**/*.toml\n---\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(diags.is_empty(), "unexpected diags: {diags:?}");
    }

    #[test]
    fn unmatched_bracket_in_globs_emits_warning() {
        let src = "---\ndescription: ok\nglobs: **/*.rs,[invalid\n---\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("invalid-globs")),
            "no invalid-globs diagnostic: {diags:?}"
        );
    }

    #[test]
    fn empty_segment_in_globs_emits_warning() {
        let src = "---\ndescription: ok\nglobs: **/*.rs,,**/*.toml\n---\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("invalid-globs")),
            "no invalid-globs diagnostic: {diags:?}"
        );
    }

    #[test]
    fn trailing_comma_in_globs_emits_warning() {
        let src = "---\ndescription: ok\nglobs: **/*.rs,\n---\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("invalid-globs")),
            "no invalid-globs diagnostic: {diags:?}"
        );
    }

    // #48 — never-fires

    #[test]
    fn never_fires_when_no_globs_and_no_always_apply() {
        let src = "---\ndescription: my rule\n---\n# Body\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("never-fires")),
            "expected never-fires diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn never_fires_when_globs_empty_and_no_always_apply() {
        let src = "---\ndescription: my rule\nglobs: \n---\n# Body\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("never-fires")),
            "expected never-fires diagnostic, got: {diags:?}"
        );
    }

    #[test]
    fn always_apply_true_suppresses_never_fires() {
        let src = "---\ndescription: my rule\nalwaysApply: true\n---\n# Body\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            !diags.iter().any(|d| d.rule.contains("never-fires")),
            "unexpected never-fires: {diags:?}"
        );
    }

    #[test]
    fn globs_present_suppresses_never_fires() {
        let src = "---\ndescription: my rule\nglobs: **/*.rs\n---\n# Body\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            !diags.iter().any(|d| d.rule.contains("never-fires")),
            "unexpected never-fires: {diags:?}"
        );
    }

    #[test]
    fn no_frontmatter_no_never_fires() {
        let src = "# Just a body\nno frontmatter here\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            !diags.iter().any(|d| d.rule.contains("never-fires")),
            "unexpected never-fires: {diags:?}"
        );
    }

    #[test]
    fn cursorrules_with_frontmatter_no_never_fires() {
        // .cursorrules does not support globs/alwaysApply — never-fires must not fire here.
        let src = "---\ndescription: my rule\n---\n# Body\n";
        let diags = v().validate(Path::new(".cursorrules"), src);
        assert!(
            !diags.iter().any(|d| d.rule.contains("never-fires")),
            "never-fires should not apply to .cursorrules: {diags:?}"
        );
    }

    // #49 — unknown-key

    #[test]
    fn unknown_key_emits_warning() {
        let src = "---\ndescription: ok\nglobs: **/*.rs\nfoo: bar\n---\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("unknown-key")),
            "expected unknown-key diagnostic, got: {diags:?}"
        );
        assert!(
            diags.iter().any(|d| d.message.contains("foo")),
            "expected key name in message, got: {diags:?}"
        );
    }

    #[test]
    fn known_keys_no_unknown_key_warning() {
        let src = "---\ndescription: ok\nglobs: **/*.rs\nalwaysApply: true\n---\n";
        let diags = v().validate(Path::new("rule.mdc"), src);
        assert!(
            !diags.iter().any(|d| d.rule.contains("unknown-key")),
            "unexpected unknown-key: {diags:?}"
        );
    }
}
