//! Minimal YAML frontmatter parser for behavioral validators.
//!
//! This is a subset of `agentlint-frontmatter` inlined into core to avoid a
//! cyclic dependency (`core` -> `frontmatter` -> `core`). Only the parse
//! primitives are duplicated; the builder/validator API stays in the
//! `agentlint-frontmatter` crate.

use nom::{
    IResult,
    bytes::complete::take_while1,
    character::complete::{char, space0},
    sequence::{preceded, terminated},
};

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

    let mut fields: Vec<Field> = Vec::new();
    let mut closed = false;

    for (i, line) in lines {
        let line_num = i + 1;

        if line.trim() == "---" {
            closed = true;
            break;
        }

        // Indented line — append as continuation of the previous field's value.
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(last) = fields.last_mut() {
                if !last.value.is_empty() {
                    last.value.push('\n');
                }
                last.value.push_str(line.trim());
            }
            continue;
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
        preceded(space0, terminated(char(':'), space0))(input)
    }

    let input = line.trim();
    let (rest, k) = key(input).ok()?;
    let (value_str, _) = colon_sep(rest).ok()?;

    let value = value_str.trim();
    let value = if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        &value[1..value.len() - 1]
    } else {
        value
    };

    Some(Field {
        key: k.to_string(),
        value: value.to_string(),
        line: line_num,
    })
}
