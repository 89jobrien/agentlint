//! Builder API for composing frontmatter validators with field-level constraints.
//!
//! # Example
//!
//! ```rust,ignore
//! let v = FrontmatterValidator::builder()
//!     .required(FieldRule::new("name").max_len(64).format(FieldFormat::KebabCase))
//!     .required(FieldRule::new("description"))
//!     .build();
//! let diags = v.validate(path, src);
//! ```

use std::path::Path;

use agentlint_core::Diagnostic;

use crate::{ParseError, parse};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Constraints on the format of a frontmatter field value.
#[derive(Clone, Debug)]
pub enum FieldFormat {
    /// Lowercase ASCII letters, digits, and hyphens.
    /// No leading/trailing hyphens, no consecutive hyphens.
    KebabCase,
}

/// Rule describing constraints on a single frontmatter field.
#[derive(Clone, Debug)]
pub struct FieldRule {
    pub(crate) name: &'static str,
    pub(crate) max_len: Option<usize>,
    pub(crate) format: Option<FieldFormat>,
    pub(crate) matches_dir_name: bool,
}

impl FieldRule {
    /// Create a new rule for the given field name.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            max_len: None,
            format: None,
            matches_dir_name: false,
        }
    }

    /// Require the value to be at most `n` characters.
    pub fn max_len(mut self, n: usize) -> Self {
        self.max_len = Some(n);
        self
    }

    /// Require the value to conform to `f`.
    pub fn format(mut self, f: FieldFormat) -> Self {
        self.format = Some(f);
        self
    }

    /// Require the value to match the immediate parent directory name of the
    /// validated file.
    pub fn matches_dir_name(mut self) -> Self {
        self.matches_dir_name = true;
        self
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for [`FrontmatterValidator`].
#[derive(Default)]
pub struct ValidatorBuilder {
    only_file: Option<&'static str>,
    required: Vec<FieldRule>,
    optional: Vec<FieldRule>,
}

impl ValidatorBuilder {
    /// Only validate files whose `file_name()` equals `name`. All other files
    /// are silently skipped (returns `vec![]`).
    pub fn only_file(mut self, name: &'static str) -> Self {
        self.only_file = Some(name);
        self
    }

    /// Add a required field rule. Missing or empty field → error diagnostic.
    pub fn required(mut self, rule: FieldRule) -> Self {
        self.required.push(rule);
        self
    }

    /// Add an optional field rule. Constraints are checked only when the field
    /// is present.
    pub fn optional(mut self, rule: FieldRule) -> Self {
        self.optional.push(rule);
        self
    }

    /// Consume the builder and produce a [`FrontmatterValidator`].
    pub fn build(self) -> FrontmatterValidator {
        FrontmatterValidator {
            only_file: self.only_file,
            required: self.required,
            optional: self.optional,
        }
    }
}

// ---------------------------------------------------------------------------
// Validator
// ---------------------------------------------------------------------------

/// A composable frontmatter validator produced by [`ValidatorBuilder`].
pub struct FrontmatterValidator {
    only_file: Option<&'static str>,
    required: Vec<FieldRule>,
    optional: Vec<FieldRule>,
}

impl FrontmatterValidator {
    /// Start building a new validator.
    pub fn builder() -> ValidatorBuilder {
        ValidatorBuilder::default()
    }

    /// Validate `src` from `path`. Returns accumulated diagnostics.
    pub fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        // Skip if only_file filter is set and doesn't match.
        if let Some(only) = self.only_file {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name != only {
                return vec![];
            }
        }

        let fields = match parse(src) {
            Ok(f) => f,
            Err(ParseError::NoFence) => {
                // When `only_file` is set, non-matching files were already
                // filtered above. When it is NOT set, every file reaching this
                // point is expected to carry frontmatter (e.g. agents/, commands/).
                // Files that genuinely have no frontmatter in that context are errors.
                // Supporting files under skills/ use `only_file` and are skipped earlier.
                if self.only_file.is_none() {
                    return vec![Diagnostic::error(
                        path,
                        1,
                        1,
                        "missing frontmatter: file must start with '---'",
                    )];
                }
                return vec![];
            }
            Err(ParseError::UnclosedFence) => {
                return vec![Diagnostic::error(
                    path,
                    1,
                    1,
                    "unclosed frontmatter fence: missing closing '---'",
                )];
            }
        };

        let mut diags = Vec::new();

        for rule in &self.required {
            match fields.iter().find(|f| f.key == rule.name) {
                None => {
                    diags.push(Diagnostic::error(
                        path,
                        1,
                        1,
                        format!("missing required field '{}'", rule.name),
                    ));
                }
                Some(f) if f.value.is_empty() => {
                    diags.push(Diagnostic::error(
                        path,
                        f.line,
                        1,
                        format!("required field '{}' must not be empty", rule.name),
                    ));
                }
                Some(f) => {
                    check_constraints(path, f.line, &f.value, rule, &mut diags);
                }
            }
        }

        for rule in &self.optional {
            if let Some(f) = fields.iter().find(|f| f.key == rule.name)
                && !f.value.is_empty()
            {
                check_constraints(path, f.line, &f.value, rule, &mut diags);
            }
        }

        diags
    }
}

// ---------------------------------------------------------------------------
// Constraint checks
// ---------------------------------------------------------------------------

fn check_constraints(
    diag_path: &Path,
    line: usize,
    value: &str,
    rule: &FieldRule,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(max) = rule.max_len
        && value.len() > max
    {
        diags.push(Diagnostic::error(
            diag_path,
            line,
            1,
            format!(
                "field '{}' value is too long ({} > {} chars)",
                rule.name,
                value.len(),
                max
            ),
        ));
    }

    if let Some(FieldFormat::KebabCase) = &rule.format
        && let Some(msg) = check_kebab_case(rule.name, value)
    {
        diags.push(Diagnostic::error(diag_path, line, 1, msg));
    }

    if rule.matches_dir_name {
        let dir_name = diag_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());
        if dir_name != Some(value) {
            diags.push(Diagnostic::error(
                diag_path,
                line,
                1,
                format!(
                    "field '{}' value '{}' must match parent directory name",
                    rule.name, value
                ),
            ));
        }
    }
}

/// Returns `Some(error_message)` if `value` is not valid kebab-case.
fn check_kebab_case(field: &str, value: &str) -> Option<String> {
    if value.is_empty() {
        return None; // emptiness handled separately
    }

    if value.starts_with('-') || value.ends_with('-') {
        return Some(format!(
            "field '{field}' value '{value}' is not valid kebab-case: leading/trailing hyphen"
        ));
    }

    if value.contains("--") {
        return Some(format!(
            "field '{field}' value '{value}' is not valid kebab-case: consecutive hyphens"
        ));
    }

    for ch in value.chars() {
        if !matches!(ch, 'a'..='z' | '0'..='9' | '-') {
            return Some(format!(
                "field '{field}' value '{value}' is not valid kebab-case: \
                 only lowercase letters, digits, and hyphens allowed"
            ));
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn path(s: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(s)
    }

    #[test]
    fn field_rule_stores_constraints() {
        let rule = FieldRule::new("name")
            .max_len(64)
            .format(FieldFormat::KebabCase)
            .matches_dir_name();
        assert_eq!(rule.name, "name");
        assert_eq!(rule.max_len, Some(64));
        assert!(matches!(rule.format, Some(FieldFormat::KebabCase)));
        assert!(rule.matches_dir_name);
    }

    #[test]
    fn required_field_missing_emits_diagnostic() {
        let v = FrontmatterValidator::builder()
            .required(FieldRule::new("name"))
            .build();
        let src = "---\ndescription: ok\n---\n";
        let diags = v.validate(Path::new("agent.md"), src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("missing required field 'name'"));
    }

    #[test]
    fn max_len_exceeded_emits_diagnostic() {
        let v = FrontmatterValidator::builder()
            .required(FieldRule::new("name").max_len(5))
            .build();
        let src = "---\nname: toolongvalue\n---\n";
        let diags = v.validate(Path::new("agent.md"), src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("too long"));
    }

    #[test]
    fn kebab_case_valid_passes() {
        let v = FrontmatterValidator::builder()
            .required(FieldRule::new("name").format(FieldFormat::KebabCase))
            .build();
        let src = "---\nname: my-agent-42\n---\n";
        let diags = v.validate(Path::new("agent.md"), src);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
    }

    #[test]
    fn kebab_case_leading_hyphen_fails() {
        let v = FrontmatterValidator::builder()
            .required(FieldRule::new("name").format(FieldFormat::KebabCase))
            .build();
        let src = "---\nname: -bad\n---\n";
        let diags = v.validate(Path::new("agent.md"), src);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("kebab-case"));
    }

    #[test]
    fn kebab_case_consecutive_hyphens_fails() {
        let v = FrontmatterValidator::builder()
            .required(FieldRule::new("name").format(FieldFormat::KebabCase))
            .build();
        let src = "---\nname: bad--name\n---\n";
        let diags = v.validate(Path::new("agent.md"), src);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("consecutive hyphens"));
    }

    #[test]
    fn dir_name_mismatch_fails() {
        let v = FrontmatterValidator::builder()
            .required(FieldRule::new("name").matches_dir_name())
            .build();
        let src = "---\nname: wrong\n---\n";
        let p = path(".claude/agents/correct/agent.md");
        let diags = v.validate(&p, src);
        assert!(!diags.is_empty());
        assert!(diags[0].message.contains("parent directory name"));
    }

    #[test]
    fn dir_name_match_passes() {
        let v = FrontmatterValidator::builder()
            .required(FieldRule::new("name").matches_dir_name())
            .build();
        let src = "---\nname: correct\n---\n";
        let p = path(".claude/agents/correct/agent.md");
        let diags = v.validate(&p, src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn only_file_skips_non_matching() {
        let v = FrontmatterValidator::builder()
            .only_file("settings.json")
            .required(FieldRule::new("name"))
            .build();
        // File name doesn't match — even though name is missing, no diagnostics
        let src = "---\ndescription: ok\n---\n";
        let diags = v.validate(Path::new("other.md"), src);
        assert!(diags.is_empty());
    }

    #[test]
    fn only_file_validates_matching() {
        let v = FrontmatterValidator::builder()
            .only_file("agent.md")
            .required(FieldRule::new("name"))
            .build();
        let src = "---\ndescription: ok\n---\n";
        let diags = v.validate(Path::new("agent.md"), src);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn no_fence_without_only_file_is_error() {
        let v = FrontmatterValidator::builder()
            .required(FieldRule::new("name"))
            .build();
        let src = "name: foo\n";
        let diags = v.validate(Path::new("agent.md"), src);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("missing frontmatter"));
    }

    #[test]
    fn no_fence_with_only_file_non_matching_is_empty() {
        // Supporting files (no only_file match) are silently skipped.
        let v = FrontmatterValidator::builder()
            .only_file("SKILL.md")
            .required(FieldRule::new("name"))
            .build();
        let src = "name: foo\n";
        // File name is "guide.md", not "SKILL.md" — already filtered before NoFence check.
        let diags = v.validate(Path::new("skills/my-skill/references/guide.md"), src);
        assert!(diags.is_empty());
    }

    #[test]
    fn optional_rule_unchecked_when_absent() {
        let v = FrontmatterValidator::builder()
            .required(FieldRule::new("name"))
            .optional(FieldRule::new("slug").format(FieldFormat::KebabCase))
            .build();
        let src = "---\nname: foo\n---\n";
        let diags = v.validate(Path::new("agent.md"), src);
        assert!(diags.is_empty());
    }

    #[test]
    fn optional_rule_checked_when_present_and_invalid() {
        let v = FrontmatterValidator::builder()
            .required(FieldRule::new("name"))
            .optional(FieldRule::new("slug").format(FieldFormat::KebabCase))
            .build();
        let src = "---\nname: foo\nslug: BAD_VALUE\n---\n";
        let diags = v.validate(Path::new("agent.md"), src);
        assert!(!diags.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use std::path::Path;

    fn kebab_validator() -> FrontmatterValidator {
        FrontmatterValidator::builder()
            .required(FieldRule::new("name").format(FieldFormat::KebabCase))
            .build()
    }

    proptest! {
        #[test]
        fn valid_kebab_never_errors(s in "[a-z][a-z0-9]*(-[a-z0-9]+)*") {
            let validator = kebab_validator();
            let src = format!("---\nname: {s}\n---\n");
            let diags = validator.validate(Path::new("agent.md"), &src);
            prop_assert!(diags.is_empty(), "unexpected errors for valid kebab '{s}': {diags:?}");
        }

        #[test]
        fn leading_hyphen_always_errors(s in "-[a-z][a-z0-9-]*") {
            let validator = kebab_validator();
            let src = format!("---\nname: {s}\n---\n");
            let diags = validator.validate(Path::new("agent.md"), &src);
            prop_assert!(!diags.is_empty(), "expected error for leading-hyphen '{s}'");
        }
    }
}
