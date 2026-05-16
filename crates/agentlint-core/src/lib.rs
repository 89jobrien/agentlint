use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[cfg(feature = "test-utils")]
pub mod testing;

#[cfg(feature = "config")]
pub mod config;
pub use config_types::{IgnoreEntry, RuleOverride};

mod config_types {
    /// Per-rule severity override from config.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[cfg_attr(feature = "config", derive(serde::Deserialize))]
    pub enum RuleOverride {
        #[cfg_attr(feature = "config", serde(rename = "error"))]
        Error,
        #[cfg_attr(feature = "config", serde(rename = "warning"))]
        Warning,
        #[cfg_attr(feature = "config", serde(rename = "off"))]
        Off,
    }

    /// Suppress specific rules for paths whose string representation ends with
    /// `path`. An empty `rules` vec suppresses all rules for matching paths.
    #[derive(Debug, Clone)]
    pub struct IgnoreEntry {
        pub path: String,
        pub rules: Vec<String>,
    }
}

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
    /// Per-rule severity overrides. Keys are rule IDs; values control whether
    /// the rule is suppressed (`Off`) or its severity is rewritten.
    pub rule_overrides: HashMap<String, RuleOverride>,
    /// Path-scoped ignore entries. Diagnostics whose path suffix matches and
    /// whose rule appears in `rules` (or all rules when `rules` is empty) are
    /// suppressed.
    pub ignores: Vec<IgnoreEntry>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            difficulty: Difficulty::Hard,
            rule_overrides: HashMap::new(),
            ignores: Vec::new(),
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
    Pretty,
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

    // 1. Difficulty filter — unclassified diagnostics (rule="") always show.
    diagnostics.retain(|d| d.rule.is_empty() || d.difficulty <= config.difficulty);

    // 2. Ignore filter — path suffix + rule match.
    diagnostics.retain(|d| {
        if d.rule.is_empty() {
            return true; // unclassified always passes
        }
        let path_str = d.path.to_string_lossy();
        for entry in &config.ignores {
            let matches_path = path_str.ends_with(&entry.path)
                || path_str.ends_with(&entry.path.replace('/', std::path::MAIN_SEPARATOR_STR));
            if matches_path && (entry.rules.is_empty() || entry.rules.iter().any(|r| r == d.rule)) {
                return false;
            }
        }
        true
    });

    // 3. Override filter — rewrite or suppress severity.
    let mut kept = Vec::with_capacity(diagnostics.len());
    for mut d in diagnostics {
        if !d.rule.is_empty() {
            match config.rule_overrides.get(d.rule) {
                Some(RuleOverride::Off) => continue,
                Some(RuleOverride::Error) => d.severity = Severity::Error,
                Some(RuleOverride::Warning) => d.severity = Severity::Warning,
                None => {}
            }
        }
        kept.push(d);
    }

    RunResult {
        diagnostics: kept,
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
        .filter_map(|path| {
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(e) => {
                    read_errors.push(Diagnostic::error(
                        &path,
                        1,
                        1,
                        format!("could not read file: {e}"),
                    ));
                    return None;
                }
            };
            // Silently skip binary files — only text files are lintable.
            String::from_utf8(bytes).ok().map(|src| (path, src))
        })
        .collect();

    let mut result = run_on(files, validators, config);
    // Prepend read errors so they appear before validation diagnostics.
    read_errors.extend(result.diagnostics);
    result.diagnostics = read_errors;
    result
}

/// Directory names that are never walked (build artifacts, VCS, package caches).
const SKIP_DIRS: &[&str] = &["target", ".git", "node_modules", "plugins", ".maestro"];

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

/// Pretty-print diagnostics grouped by file with ANSI colour.
///
/// Pass `color = false` when stdout is not a TTY.
pub fn format_pretty(diagnostics: &[Diagnostic], color: bool) -> String {
    use std::collections::BTreeMap;

    // ANSI helpers — empty strings when color is off.
    let bold = if color { "\x1b[1m" } else { "" };
    let dim = if color { "\x1b[2m" } else { "" };
    let red = if color { "\x1b[31m" } else { "" };
    let yellow = if color { "\x1b[33m" } else { "" };
    let cyan = if color { "\x1b[36m" } else { "" };
    let reset = if color { "\x1b[0m" } else { "" };

    // Group by path, preserving insertion order via BTreeMap (sorts paths).
    let mut by_file: BTreeMap<String, Vec<&Diagnostic>> = BTreeMap::new();
    for d in diagnostics {
        by_file
            .entry(d.path.display().to_string())
            .or_default()
            .push(d);
    }

    // Try to strip cwd prefix for shorter paths.
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.display().to_string() + "/");

    let shorten = |p: &str| -> String {
        if let Some(ref prefix) = cwd
            && let Some(rel) = p.strip_prefix(prefix.as_str())
        {
            return rel.to_string();
        }
        p.to_string()
    };

    let mut out = String::new();
    let mut total_errors: usize = 0;
    let mut total_warnings: usize = 0;

    for (path, diags) in &by_file {
        // File header.
        out.push_str(&format!("{bold}{cyan}{}{reset}\n", shorten(path)));

        for d in diags {
            let (sev_color, sev_label) = match d.severity {
                Severity::Error => (red, "error"),
                Severity::Warning => (yellow, "warning"),
            };

            let rule_hint = if d.rule.is_empty() {
                String::new()
            } else {
                format!("  {dim}[{}]{reset}", d.rule)
            };

            out.push_str(&format!(
                "  {sev_color}{bold}{sev_label}{reset}  {}{rule_hint}\n",
                d.message,
            ));

            match d.severity {
                Severity::Error => total_errors += 1,
                Severity::Warning => total_warnings += 1,
            }
        }
        out.push('\n');
    }

    // Summary line.
    match (total_errors, total_warnings) {
        (0, 0) => {}
        (e, 0) => out.push_str(&format!(
            "{red}{bold}✖ {e} error{}{reset}\n",
            if e == 1 { "" } else { "s" }
        )),
        (0, w) => out.push_str(&format!(
            "{yellow}{bold}⚠ {w} warning{}{reset}\n",
            if w == 1 { "" } else { "s" }
        )),
        (e, w) => out.push_str(&format!(
            "{red}{bold}✖ {e} error{}{reset}  {yellow}{bold}⚠ {w} warning{}{reset}\n",
            if e == 1 { "" } else { "s" },
            if w == 1 { "" } else { "s" },
        )),
    }

    out
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
    use std::path::PathBuf;

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

    // ---------------------------------------------------------------------------
    // Filtering logic tests
    // ---------------------------------------------------------------------------

    fn easy_error(path: &str, rule: &'static str) -> Diagnostic {
        Diagnostic::error(PathBuf::from(path), 1, 1, "msg").with_rule(rule, Difficulty::Easy)
    }

    fn painful_warning(path: &str, rule: &'static str) -> Diagnostic {
        Diagnostic::warning(PathBuf::from(path), 1, 1, "msg").with_rule(rule, Difficulty::Painful)
    }

    fn unclassified(path: &str) -> Diagnostic {
        Diagnostic::error(PathBuf::from(path), 1, 1, "unclassified")
    }

    fn run_filters(diagnostics: Vec<Diagnostic>, config: RunConfig) -> Vec<Diagnostic> {
        // Use run_on with a fixed file set — simulate by calling filtering inline.
        // We pass no validators since we supply pre-built diagnostics; instead we
        // replicate the filter logic by running run_on on an empty file set and
        // verifying the filter path directly via a minimal Validator shim.
        struct Shim(Vec<Diagnostic>);
        impl Validator for Shim {
            fn patterns(&self) -> &[&str] {
                &["__shim__"]
            }
            fn validate(&self, _: &Path, _: &str) -> Vec<Diagnostic> {
                self.0.clone()
            }
        }
        let files = vec![(PathBuf::from("__shim__"), String::new())];
        let validators: Vec<Box<dyn Validator>> = vec![Box::new(Shim(diagnostics))];
        run_on(files, &validators, &config).diagnostics
    }

    #[test]
    fn difficulty_filter_drops_painful_at_hard() {
        let diags = vec![painful_warning(
            ".claude/settings.json",
            "claude/settings/broad-read",
        )];
        let result = run_filters(diags, RunConfig::default()); // default = Hard
        assert!(result.is_empty());
    }

    #[test]
    fn difficulty_filter_passes_painful_at_painful() {
        let diags = vec![painful_warning(
            ".claude/settings.json",
            "claude/settings/broad-read",
        )];
        let result = run_filters(
            diags,
            RunConfig {
                difficulty: Difficulty::Painful,
                ..RunConfig::default()
            },
        );
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn ignore_filter_suppresses_matching_rule_for_matching_path() {
        let diags = vec![easy_error(
            ".claude/settings.local.json",
            "claude/settings/broad-read",
        )];
        let config = RunConfig {
            ignores: vec![IgnoreEntry {
                path: ".claude/settings.local.json".into(),
                rules: vec!["claude/settings/broad-read".into()],
            }],
            ..RunConfig::default()
        };
        assert!(run_filters(diags, config).is_empty());
    }

    #[test]
    fn ignore_filter_empty_rules_suppresses_all_for_path() {
        let diags = vec![
            easy_error(".claude/settings.local.json", "claude/settings/broad-read"),
            easy_error(
                ".claude/settings.local.json",
                "claude/settings/sshpass-credential",
            ),
        ];
        let config = RunConfig {
            ignores: vec![IgnoreEntry {
                path: ".claude/settings.local.json".into(),
                rules: vec![],
            }],
            ..RunConfig::default()
        };
        assert!(run_filters(diags, config).is_empty());
    }

    #[test]
    fn ignore_filter_does_not_suppress_different_path() {
        let diags = vec![easy_error(
            ".claude/settings.json",
            "claude/settings/broad-read",
        )];
        let config = RunConfig {
            ignores: vec![IgnoreEntry {
                path: ".claude/settings.local.json".into(),
                rules: vec!["claude/settings/broad-read".into()],
            }],
            ..RunConfig::default()
        };
        assert_eq!(run_filters(diags, config).len(), 1);
    }

    #[test]
    fn override_off_drops_diagnostic() {
        let diags = vec![easy_error(
            ".claude/settings.json",
            "claude/settings/unknown-key",
        )];
        let config = RunConfig {
            rule_overrides: [("claude/settings/unknown-key".into(), RuleOverride::Off)]
                .into_iter()
                .collect(),
            ..RunConfig::default()
        };
        assert!(run_filters(diags, config).is_empty());
    }

    #[test]
    fn override_warning_demotes_error() {
        let diags = vec![easy_error(
            ".claude/settings.json",
            "claude/settings/unknown-key",
        )];
        let config = RunConfig {
            rule_overrides: [("claude/settings/unknown-key".into(), RuleOverride::Warning)]
                .into_iter()
                .collect(),
            ..RunConfig::default()
        };
        let result = run_filters(diags, config);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].severity, Severity::Warning);
    }

    #[test]
    fn override_error_promotes_warning() {
        let diags = vec![
            Diagnostic::warning(PathBuf::from(".claude/settings.json"), 1, 1, "msg")
                .with_rule("claude/settings/skip-dangerous-mode", Difficulty::Hard),
        ];
        let config = RunConfig {
            rule_overrides: [(
                "claude/settings/skip-dangerous-mode".into(),
                RuleOverride::Error,
            )]
            .into_iter()
            .collect(),
            ..RunConfig::default()
        };
        let result = run_filters(diags, config);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].severity, Severity::Error);
    }

    #[test]
    fn unclassified_passes_all_filters() {
        let diags = vec![unclassified("some/path")];
        let config = RunConfig {
            difficulty: Difficulty::Easy,
            rule_overrides: [("".into(), RuleOverride::Off)].into_iter().collect(),
            ignores: vec![IgnoreEntry {
                path: "some/path".into(),
                rules: vec![],
            }],
        };
        // Unclassified should survive even with an empty-rule ignore for its path
        // because we skip ignore+override for rule="" diagnostics.
        let result = run_filters(diags, config);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_order_difficulty_before_ignore() {
        // painful diagnostic — no ignore needed, dropped by difficulty alone
        let diags = vec![painful_warning(
            ".claude/settings.json",
            "claude/settings/broad-read",
        )];
        let config = RunConfig {
            difficulty: Difficulty::Hard,
            // No ignore entries — if difficulty filter works, this is never reached
            ..RunConfig::default()
        };
        assert!(run_filters(diags, config).is_empty());
    }
}
