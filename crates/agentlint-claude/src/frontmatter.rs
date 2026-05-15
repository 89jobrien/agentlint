//! YAML frontmatter parser and required-field validator.
//!
//! Handles files of the form:
//!
//! ```text
//! ---
//! key: value
//! other: value
//! ---
//! body...
//! ```
//!
//! Produces `Vec<Field>` with 1-indexed line numbers for accurate diagnostics.
//! Parsing is line-based; nom is used for field extraction within each line.

use agentlint_core::Diagnostic;
use nom::{
    IResult,
    bytes::complete::take_while1,
    character::complete::{char, space0},
    sequence::terminated,
};
use std::path::Path;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub key: String,
    pub value: String,
    /// 1-indexed line number within the source file.
    pub line: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    /// File does not begin with `---`.
    NoFence,
    /// Opening `---` found but no closing `---`.
    UnclosedFence,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse YAML frontmatter from `src`. Returns all fields found between the
/// opening and closing `---` fences, with accurate 1-indexed line numbers.
pub fn parse(src: &str) -> Result<Vec<Field>, ParseError> {
    let mut lines = src.lines().enumerate();

    // Line 1 must be "---"
    match lines.next() {
        Some((_, l)) if l.trim() == "---" => {}
        _ => return Err(ParseError::NoFence),
    }

    let mut fields = Vec::new();
    let mut closed = false;

    for (i, line) in lines {
        let line_num = i + 1; // enumerate is 0-indexed; line 0 was the opening fence

        if line.trim() == "---" {
            closed = true;
            break;
        }

        if let Some(field) = parse_field(line, line_num) {
            fields.push(field);
        }
    }

    if !closed {
        return Err(ParseError::UnclosedFence);
    }

    Ok(fields)
}

/// Use nom to extract `key: value` from a single line.
fn parse_field(line: &str, line_num: usize) -> Option<Field> {
    fn key(input: &str) -> IResult<&str, &str> {
        take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_')(input)
    }

    fn colon_sep(input: &str) -> IResult<&str, char> {
        terminated(char(':'), space0)(input)
    }

    let input = line.trim();
    let (rest, k) = key(input).ok()?;
    let (value_str, _) = colon_sep(rest).ok()?;

    Some(Field {
        key: k.to_string(),
        value: value_str.trim().to_string(),
        line: line_num,
    })
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate that all `required` fields are present and non-empty in `src`.
///
/// Returns diagnostics with accurate line numbers where possible.
pub fn check_required(path: &Path, src: &str, required: &[&str]) -> Vec<Diagnostic> {
    let fields = match parse(src) {
        Ok(f) => f,
        Err(ParseError::NoFence) => {
            return vec![Diagnostic::error(
                path,
                1,
                1,
                "missing frontmatter: file must start with '---'",
            )];
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

    let mut diagnostics = Vec::new();

    for &field in required {
        match fields.iter().find(|f| f.key == field) {
            None => {
                diagnostics.push(Diagnostic::error(
                    path,
                    1,
                    1,
                    format!("missing required field '{field}'"),
                ));
            }
            Some(f) if f.value.is_empty() => {
                diagnostics.push(Diagnostic::error(
                    path,
                    f.line,
                    1,
                    format!("required field '{field}' must not be empty"),
                ));
            }
            Some(_) => {}
        }
    }

    diagnostics
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // --- parse() ---

    #[test]
    fn parse_well_formed_returns_fields() {
        let src = "---\nname: my-agent\ndescription: does things\n---\nbody";
        let fields = parse(src).unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].key, "name");
        assert_eq!(fields[0].value, "my-agent");
        assert_eq!(fields[0].line, 2);
        assert_eq!(fields[1].key, "description");
        assert_eq!(fields[1].value, "does things");
        assert_eq!(fields[1].line, 3);
    }

    #[test]
    fn parse_no_fence_returns_error() {
        let src = "name: foo\n---\n";
        assert_eq!(parse(src), Err(ParseError::NoFence));
    }

    #[test]
    fn parse_unclosed_fence_returns_error() {
        let src = "---\nname: foo\n";
        assert_eq!(parse(src), Err(ParseError::UnclosedFence));
    }

    #[test]
    fn parse_empty_body_is_ok() {
        let src = "---\nname: x\ndescription: y\n---\n";
        assert!(parse(src).is_ok());
    }

    #[test]
    fn parse_field_line_numbers_are_accurate() {
        let src = "---\ntools: []\nname: x\ndescription: y\n---\n";
        let fields = parse(src).unwrap();
        let name = fields.iter().find(|f| f.key == "name").unwrap();
        assert_eq!(name.line, 3);
        let desc = fields.iter().find(|f| f.key == "description").unwrap();
        assert_eq!(desc.line, 4);
    }

    #[test]
    fn parse_skips_unparseable_lines() {
        // Lines without a colon are ignored rather than erroring.
        let src = "---\nname: foo\nnot a field\ndescription: bar\n---\n";
        let fields = parse(src).unwrap();
        assert!(fields.iter().any(|f| f.key == "name"));
        assert!(fields.iter().any(|f| f.key == "description"));
    }

    // --- check_required() ---

    #[test]
    fn check_required_clean_file_no_diagnostics() {
        let src = "---\nname: my-agent\ndescription: does things\n---\n";
        let diags = check_required(Path::new("agent.md"), src, &["name", "description"]);
        assert!(diags.is_empty());
    }

    #[test]
    fn check_required_missing_field_is_error() {
        let src = "---\nname: my-agent\n---\n";
        let diags = check_required(Path::new("agent.md"), src, &["name", "description"]);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("description"));
    }

    #[test]
    fn check_required_empty_field_is_error_at_correct_line() {
        let src = "---\nname: \ndescription: ok\n---\n";
        let diags = check_required(Path::new("agent.md"), src, &["name", "description"]);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 2);
        assert!(diags[0].message.contains("'name'"));
    }

    #[test]
    fn check_required_no_fence_is_error() {
        let src = "name: foo\n";
        let diags = check_required(Path::new("agent.md"), src, &["name"]);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("frontmatter"));
    }

    #[test]
    fn check_required_unclosed_fence_is_error() {
        let src = "---\nname: foo\n";
        let diags = check_required(Path::new("agent.md"), src, &["name"]);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("unclosed"));
    }

    #[test]
    fn check_required_accumulates_all_missing_fields() {
        let src = "---\n---\n";
        let diags = check_required(Path::new("agent.md"), src, &["name", "description"]);
        assert_eq!(diags.len(), 2);
    }
}
