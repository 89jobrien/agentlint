use std::path::{Path, PathBuf};

#[cfg(feature = "test-utils")]
pub mod testing;

// ---------------------------------------------------------------------------
// Diagnostic
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub severity: Severity,
    pub message: String,
}

impl Diagnostic {
    pub fn error(
        path: impl Into<PathBuf>,
        line: usize,
        col: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            line,
            col,
            severity: Severity::Error,
            message: message.into(),
        }
    }

    pub fn warning(
        path: impl Into<PathBuf>,
        line: usize,
        col: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            line,
            col,
            severity: Severity::Warning,
            message: message.into(),
        }
    }

    pub fn gnu_format(&self) -> String {
        format!(
            "{}:{}:{}: {}: {}",
            self.path.display(),
            self.line,
            self.col,
            self.severity,
            self.message,
        )
    }
}

// ---------------------------------------------------------------------------
// Validator trait
// ---------------------------------------------------------------------------

pub trait Validator: Send + Sync {
    /// File glob patterns this validator claims (e.g. `.claude/agents/**/*.md`).
    fn patterns(&self) -> &[&str];

    /// Validate `src` (the file contents) for `path`. Returns all diagnostics.
    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic>;
}

// ---------------------------------------------------------------------------
// Output format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Gnu,
    Json,
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

pub struct RunResult {
    pub diagnostics: Vec<Diagnostic>,
    pub files_checked: usize,
}

/// Walk `roots` (files or directories), dispatch each file to the first
/// matching validator, and collect all diagnostics.
pub fn run(roots: &[PathBuf], validators: &[Box<dyn Validator>]) -> RunResult {
    let mut diagnostics = Vec::new();
    let mut files_checked = 0;

    let paths = collect_paths(roots);

    for path in paths {
        let matched = find_validators(&path, validators);
        if matched.is_empty() {
            continue;
        }

        files_checked += 1;

        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                diagnostics.push(Diagnostic::error(
                    &path,
                    1,
                    1,
                    format!("could not read file: {e}"),
                ));
                continue;
            }
        };

        for validator in matched {
            diagnostics.extend(validator.validate(&path, &src));
        }
    }

    RunResult {
        diagnostics,
        files_checked,
    }
}

fn collect_paths(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        if root.is_file() {
            out.push(root.clone());
        } else if root.is_dir() {
            for entry in walkdir::WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                out.push(entry.into_path());
            }
        }
    }
    out
}

fn find_validators<'a>(
    path: &Path,
    validators: &'a [Box<dyn Validator>],
) -> Vec<&'a dyn Validator> {
    let path_str = path.to_string_lossy();
    validators
        .iter()
        .filter(|v| v.patterns().iter().any(|p| glob_match(p, &path_str)))
        .map(|v| v.as_ref())
        .collect()
}

/// Minimal glob matching: supports `**`, `*`, and literal segments.
fn glob_match(pattern: &str, path: &str) -> bool {
    glob_match_inner(pattern.as_bytes(), path.as_bytes())
}

fn glob_match_inner(pat: &[u8], s: &[u8]) -> bool {
    match (pat.first(), s.first()) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(b'*'), _) => {
            // Check for `**`
            if pat.get(1) == Some(&b'*') {
                let rest_pat = pat.get(2..).unwrap_or(b"");
                // Skip leading `/` after `**`
                let rest_pat = rest_pat.strip_prefix(b"/").unwrap_or(rest_pat);
                // Try matching rest_pat against every suffix of s
                for i in 0..=s.len() {
                    if glob_match_inner(rest_pat, &s[i..]) {
                        return true;
                    }
                }
                false
            } else {
                let rest_pat = &pat[1..];
                // `*` matches anything except `/`
                for i in 0..=s.len() {
                    if s[..i].contains(&b'/') {
                        break;
                    }
                    if glob_match_inner(rest_pat, &s[i..]) {
                        return true;
                    }
                }
                false
            }
        }
        (Some(&pc), Some(&sc)) => {
            if pc == sc {
                glob_match_inner(&pat[1..], &s[1..])
            } else {
                false
            }
        }
        (Some(_), None) => false,
    }
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

pub fn format_gnu(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|d| d.gnu_format())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn format_json(diagnostics: &[Diagnostic]) -> String {
    let entries: Vec<serde_json::Value> = diagnostics
        .iter()
        .map(|d| {
            serde_json::json!({
                "path": d.path.display().to_string(),
                "line": d.line,
                "col": d.col,
                "severity": d.severity.to_string(),
                "message": d.message,
            })
        })
        .collect();
    serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_literal() {
        assert!(glob_match("AGENTS.md", "AGENTS.md"));
        assert!(!glob_match("AGENTS.md", "agents.md"));
    }

    #[test]
    fn glob_star() {
        assert!(glob_match("*.md", "README.md"));
        assert!(!glob_match("*.md", "src/README.md"));
    }

    #[test]
    fn glob_double_star() {
        assert!(glob_match(
            ".claude/agents/**/*.md",
            ".claude/agents/foo/bar.md"
        ));
        assert!(glob_match(
            ".claude/agents/**/*.md",
            ".claude/agents/bar.md"
        ));
    }
}
