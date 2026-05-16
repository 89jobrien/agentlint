use std::path::{Path, PathBuf};

#[cfg(feature = "test-utils")]
pub mod testing;

// ---------------------------------------------------------------------------
// Difficulty
// ---------------------------------------------------------------------------

/// Controls which rules fire. Rules at or below the configured difficulty are
/// reported; rules above are silently suppressed.
///
/// Ordered: `Easy` < `Hard` < `Painful`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Difficulty {
    /// Definite breakage only: invalid JSON, missing shebang, empty files,
    /// credential exposure.
    Easy,
    /// Breakage + operational problems: hook leaks, dangerous settings,
    /// missing required fields.
    #[default]
    Hard,
    /// Everything: best-practice style, stale allows, broad permissions,
    /// naive patterns.
    Painful,
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Difficulty::Easy => write!(f, "easy"),
            Difficulty::Hard => write!(f, "hard"),
            Difficulty::Painful => write!(f, "painful"),
        }
    }
}

impl std::str::FromStr for Difficulty {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "easy" => Ok(Difficulty::Easy),
            "hard" => Ok(Difficulty::Hard),
            "painful" => Ok(Difficulty::Painful),
            other => Err(format!(
                "unknown difficulty '{other}'; expected easy, hard, or painful"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// RunConfig
// ---------------------------------------------------------------------------

/// Configuration passed to the runner; controls filtering and output behaviour.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Only report diagnostics whose difficulty is ≤ this level.
    /// Default: `Hard`.
    pub difficulty: Difficulty,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            difficulty: Difficulty::Hard,
        }
    }
}

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
    /// Rule identifier in `<validator>/<category>/<slug>` form. Empty string
    /// means the rule is unclassified (always shown regardless of difficulty).
    pub rule: &'static str,
    /// Difficulty tier that gates this diagnostic.
    pub difficulty: Difficulty,
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
            rule: "",
            difficulty: Difficulty::Easy,
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
            rule: "",
            difficulty: Difficulty::Easy,
        }
    }

    /// Set the rule ID and difficulty tier for this diagnostic.
    pub fn with_rule(mut self, rule: &'static str, difficulty: Difficulty) -> Self {
        self.rule = rule;
        self.difficulty = difficulty;
        self
    }

    pub fn gnu_format(&self) -> String {
        if self.rule.is_empty() {
            format!(
                "{}:{}:{}: {}: {}",
                self.path.display(),
                self.line,
                self.col,
                self.severity,
                self.message,
            )
        } else {
            format!(
                "{}:{}:{}: {}[{}]: {}",
                self.path.display(),
                self.line,
                self.col,
                self.severity,
                self.rule,
                self.message,
            )
        }
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

/// Pure domain runner: dispatch `files` (already-loaded path+content pairs) to
/// matching validators and collect diagnostics.
///
/// This is the hexagonal core — it has no filesystem dependency. Infrastructure
/// callers (see [`run`]) are responsible for discovery and I/O.
pub fn run_on(
    files: impl IntoIterator<Item = (PathBuf, String)>,
    validators: &[Box<dyn Validator>],
    config: &RunConfig,
) -> RunResult {
    let mut diagnostics = Vec::new();
    let mut files_checked = 0;

    for (path, src) in files {
        let matched = find_validators(&path, validators);
        if matched.is_empty() {
            continue;
        }
        files_checked += 1;
        for validator in matched {
            diagnostics.extend(validator.validate(&path, &src));
        }
    }

    // Filter by difficulty: unclassified (rule="") diagnostics always show.
    diagnostics.retain(|d| d.rule.is_empty() || d.difficulty <= config.difficulty);

    RunResult {
        diagnostics,
        files_checked,
    }
}

/// Infrastructure convenience: walk `roots`, read each file, then delegate to
/// [`run_on`]. Only files claimed by at least one validator are read; binary
/// and unrecognised files are silently skipped. Read errors on claimed files
/// are surfaced as [`Diagnostic::error`] entries rather than panicking.
pub fn run(roots: &[PathBuf], validators: &[Box<dyn Validator>], config: &RunConfig) -> RunResult {
    let mut read_errors: Vec<Diagnostic> = Vec::new();

    let files: Vec<(PathBuf, String)> = collect_paths(roots)
        .into_iter()
        .filter(|path| !find_validators(path, validators).is_empty())
        .filter_map(|path| match std::fs::read_to_string(&path) {
            Ok(src) => Some((path, src)),
            Err(e) => {
                read_errors.push(Diagnostic::error(
                    &path,
                    1,
                    1,
                    format!("could not read file: {e}"),
                ));
                None
            }
        })
        .collect();

    let mut result = run_on(files, validators, config);
    // Prepend read errors so they appear before validation diagnostics.
    read_errors.extend(result.diagnostics);
    result.diagnostics = read_errors;
    result
}

/// Directory names that are never walked (build artifacts, VCS, package caches).
const SKIP_DIRS: &[&str] = &["target", ".git", "node_modules", "plugins"];

fn collect_paths(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        if root.is_file() {
            out.push(root.clone());
        } else if root.is_dir() {
            for entry in walkdir::WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    if e.file_type().is_dir() {
                        let name = e.file_name().to_string_lossy();
                        !SKIP_DIRS.iter().any(|skip| *skip == name.as_ref())
                    } else {
                        true
                    }
                })
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
    // Build candidate strings: every component-suffix of the path, so that
    // patterns like `.claude/agents/**/*.md` match both relative paths
    // (`.claude/agents/foo.md`) and absolute paths (`/repo/.claude/agents/foo.md`).
    let comps: Vec<_> = path.components().collect();
    let suffixes: Vec<String> = (0..comps.len())
        .map(|i| {
            comps[i..]
                .iter()
                .collect::<PathBuf>()
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    validators
        .iter()
        .filter(|v| {
            v.patterns()
                .iter()
                .any(|p| suffixes.iter().any(|s| glob_match(p, s)))
        })
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
                "rule": d.rule,
                "difficulty": d.difficulty.to_string(),
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
