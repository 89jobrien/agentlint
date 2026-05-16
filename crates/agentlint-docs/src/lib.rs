use agentlint_core::{Diagnostic, Difficulty, Validator};
use agentlint_frontmatter::{ParseError, parse};
use serde::Deserialize;
use std::path::Path;

// ---------------------------------------------------------------------------
// FilenameConvention — configurable naming format
// ---------------------------------------------------------------------------

/// A single filename convention entry.
///
/// `format` is a template string using `{token}` placeholders. Supported
/// tokens:
///
/// | Token      | Meaning                                          |
/// | ---------- | ------------------------------------------------ |
/// | `{doctype}`| maps to the `doctype` frontmatter field          |
/// | `{project}`| maps to the `project` frontmatter field          |
/// | `{topic}`  | free-form topic slug (no frontmatter constraint) |
/// | `{ref}`    | date/ticket prefix — stripped during matching    |
/// | `{status}` | maps to the `status` frontmatter field           |
///
/// `dirs` — if non-empty, this convention only applies when the file's
/// immediate parent directory matches one of these names. An empty `dirs`
/// list means the convention applies to all paths under `file_glob`.
///
/// ```toml
/// # Repo doc convention: {doctype}.{project}.md
/// [[docs.conventions]]
/// format = "{doctype}.{project}.md"
///
/// # Research doc convention scoped to specs/plans/ideas/
/// [[docs.conventions]]
/// dirs   = ["specs", "plans", "ideas"]
/// format = "{ref}-{topic}.{doctype}.md"
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct FilenameConvention {
    /// Format template, e.g. `"{doctype}.{project}.md"`.
    pub format: String,
    /// Optional directory scope. Empty = applies everywhere.
    #[serde(default)]
    pub dirs: Vec<String>,
}

impl FilenameConvention {
    /// Try to match `stem` (filename without `.md` extension) against this
    /// convention's format template.
    ///
    /// Returns a map of token → extracted value on success, or `None` if the
    /// stem does not match the pattern.
    pub fn match_stem(&self, stem: &str) -> Option<std::collections::HashMap<String, String>> {
        // Strip the trailing `.md` from the format if present — we operate on stems.
        let fmt = self
            .format
            .strip_suffix(".md")
            .unwrap_or(self.format.as_str());
        match_template(fmt, stem)
    }
}

/// Match `input` against a `{token}`-based template, returning extracted
/// token values or `None` on mismatch.
///
/// Tokens are matched greedily left-to-right. The `{ref}` token matches a
/// leading date or numeric prefix and its trailing hyphen.
fn match_template(
    template: &str,
    input: &str,
) -> Option<std::collections::HashMap<String, String>> {
    let mut result = std::collections::HashMap::new();
    match_template_inner(template, input, &mut result).then_some(result)
}

fn match_template_inner(
    template: &str,
    input: &str,
    out: &mut std::collections::HashMap<String, String>,
) -> bool {
    if template.is_empty() {
        return input.is_empty();
    }

    if let Some(rest_tmpl) = template.strip_prefix('{') {
        // Find closing '}'.
        let end = match rest_tmpl.find('}') {
            Some(i) => i,
            None => return false,
        };
        let token = &rest_tmpl[..end];
        let after_token = &rest_tmpl[end + 1..];

        if token == "ref" {
            // {ref} matches an optional leading date/ticket prefix (without
            // the trailing '-' separator — that is left for the template literal).
            let ref_val = extract_ref_prefix(input);
            out.insert("ref".into(), ref_val.into());
            let rest_input = &input[ref_val.len()..];
            return match_template_inner(after_token, rest_input, out);
        }

        // Determine what delimiter follows this token in the template.
        if after_token.is_empty() {
            // Last token — consume the rest.
            out.insert(token.to_string(), input.to_string());
            return true;
        }

        let delimiter = if after_token.starts_with('{') {
            // Two adjacent tokens — use '.' as implicit separator.
            "."
        } else {
            // Next char is a literal delimiter.
            let end = after_token.find('{').unwrap_or(after_token.len()).min(1);
            &after_token[..end]
        };

        // Find the delimiter in input and split.
        let split_at = match input.find(delimiter) {
            Some(i) => i,
            None => return false,
        };
        let val = &input[..split_at];
        let rest_input = &input[split_at + delimiter.len()..];
        out.insert(token.to_string(), val.to_string());
        match_template_inner(after_token.trim_start_matches(delimiter), rest_input, out)
    } else {
        // Literal prefix — must match exactly.
        let literal_end = template.find('{').unwrap_or(template.len());
        let literal = &template[..literal_end];
        let rest_tmpl = &template[literal_end..];
        match input.strip_prefix(literal) {
            Some(rest_input) => match_template_inner(rest_tmpl, rest_input, out),
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// DocsSchema — configurable schema for docs frontmatter validation
// ---------------------------------------------------------------------------

/// Schema that drives `DocsValidator`. All fields have sensible defaults and
/// can be overridden via the `[docs]` section of `.agentlint.toml`.
///
/// ```toml
/// [docs]
/// file_glob       = "docs/**/*.md"
/// required_fields = ["title", "doctype", "status", "created", "updated"]
/// doctypes        = ["spec", "plan", "adr", "guide"]
/// statuses        = ["draft", "active", "archived"]
/// date_fields     = ["created", "updated"]
///
/// [[docs.conventions]]
/// format = "{doctype}.{project}.md"
///
/// [[docs.conventions]]
/// dirs   = ["specs", "plans", "ideas"]
/// format = "{ref}-{topic}.{doctype}.md"
/// ```
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DocsSchema {
    /// Glob pattern that selects which files this validator claims.
    pub file_glob: String,
    /// Frontmatter fields that must be present and non-empty.
    pub required_fields: Vec<String>,
    /// Allowed values for the `doctype` field.
    pub doctypes: Vec<String>,
    /// Allowed values for the `status` field.
    pub statuses: Vec<String>,
    /// Fields whose values must be `YYYY-MM-DD` dates.
    pub date_fields: Vec<String>,
    /// Filename conventions. The first convention whose `dirs` scope matches
    /// the file's parent directory is used. An empty `dirs` matches any dir.
    pub conventions: Vec<FilenameConvention>,
}

impl Default for DocsSchema {
    fn default() -> Self {
        Self {
            file_glob: "docs/**/*.md".into(),
            required_fields: vec![
                "title".into(),
                "doctype".into(),
                "project".into(),
                "status".into(),
                "created".into(),
                "updated".into(),
            ],
            doctypes: vec![
                "idea".into(),
                "spec".into(),
                "plan".into(),
                "adr".into(),
                "roadmap".into(),
                "guide".into(),
                "reference".into(),
                "runbook".into(),
                "architecture".into(),
                "capability-matrix".into(),
                "testing".into(),
                "development".into(),
                "readme".into(),
            ],
            statuses: vec![
                "draft".into(),
                "active".into(),
                "archived".into(),
                "superseded".into(),
            ],
            date_fields: vec!["created".into(), "updated".into()],
            conventions: vec![
                // Dir-scoped conventions come first so they take priority over
                // the catch-all repo doc convention for files in those dirs.
                FilenameConvention {
                    format: "{ref}-{topic}.{doctype}.md".into(),
                    dirs: vec!["ideas".into(), "specs".into(), "plans".into()],
                },
                // Fallback for research docs with no explicit doctype suffix.
                FilenameConvention {
                    format: "{ref}-{topic}.md".into(),
                    dirs: vec!["ideas".into(), "specs".into(), "plans".into()],
                },
                // Catch-all repo doc convention.
                FilenameConvention {
                    format: "{doctype}.{project}.md".into(),
                    dirs: vec![],
                },
            ],
        }
    }
}

impl DocsSchema {
    /// Load the `[docs]` section from an `.agentlint.toml` file.
    ///
    /// Returns `DocsSchema::default()` when the file is absent or has no
    /// `[docs]` section. Returns an error string when the file exists but
    /// cannot be parsed.
    pub fn from_config_path(path: &Path) -> Result<Self, String> {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("could not read {}: {e}", path.display())),
        };

        #[derive(Deserialize, Default)]
        struct RawFile {
            #[serde(default)]
            docs: Option<DocsSchema>,
        }

        let raw: RawFile =
            toml::from_str(&src).map_err(|e| format!("invalid config {}: {e}", path.display()))?;

        Ok(raw.docs.unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// DocsValidator
// ---------------------------------------------------------------------------

pub struct DocsValidator {
    schema: DocsSchema,
    /// Leaked copy of `schema.file_glob` so we can return `&[&str]` from
    /// `patterns()` without lifetime complications.
    glob: &'static str,
}

impl DocsValidator {
    pub fn new(schema: DocsSchema) -> Self {
        let glob: &'static str = Box::leak(schema.file_glob.clone().into_boxed_str());
        Self { schema, glob }
    }
}

impl Default for DocsValidator {
    fn default() -> Self {
        Self::new(DocsSchema::default())
    }
}

// ---------------------------------------------------------------------------
// Filename parsing
// ---------------------------------------------------------------------------

/// Tokens extracted from a filename by matching against a convention.
type TokenMap = std::collections::HashMap<String, String>;

/// Try each convention in order, returning the first match along with the
/// matched convention's format string, or `None` if no convention matches.
fn match_convention<'a>(
    path: &Path,
    conventions: &'a [FilenameConvention],
) -> Option<(&'a FilenameConvention, TokenMap)> {
    let stem = path.file_stem().and_then(|s| s.to_str())?;
    let parent_name = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");

    for conv in conventions {
        // Check dir scope.
        if !conv.dirs.is_empty() && !conv.dirs.iter().any(|d| d == parent_name) {
            continue;
        }
        if let Some(tokens) = conv.match_stem(stem) {
            return Some((conv, tokens));
        }
    }
    None
}

/// Extract just the leading date or ticket-number prefix from `s`, without
/// the trailing `-` separator. Returns an empty string if there is no prefix.
///
/// Examples:
/// - `"20260516-agentlint-docs"` → `"20260516"`
/// - `"2026-05-15-agentlint"`   → `"2026-05-15"`
/// - `"agentlint-docs"`         → `""`
fn extract_ref_prefix(s: &str) -> &str {
    // YYYY-MM-DD prefix (10 chars).
    if s.len() > 11 {
        let candidate = &s[..10];
        let parts: Vec<&str> = candidate.split('-').collect();
        if parts.len() == 3
            && parts[0].len() == 4
            && parts[1].len() == 2
            && parts[2].len() == 2
            && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
            && s.as_bytes().get(10) == Some(&b'-')
        {
            return &s[..10];
        }
    }
    // All-digit prefix before the first hyphen (YYYYMMDD or ticket number).
    if let Some(pos) = s.find('-') {
        let prefix = &s[..pos];
        if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
            return &s[..pos];
        }
    }
    ""
}

// ---------------------------------------------------------------------------
// Date validation
// ---------------------------------------------------------------------------

/// Returns true if `s` matches `YYYY-MM-DD` with plausible calendar ranges.
fn is_valid_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    let (y, m, d) = (parts[0], parts[1], parts[2]);
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (y.parse::<u32>(), m.parse::<u32>(), d.parse::<u32>())
    else {
        return false;
    };
    year >= 2000 && (1..=12).contains(&month) && (1..=31).contains(&day)
}

// ---------------------------------------------------------------------------
// Rule ID helpers — map field name to a stable rule string
// ---------------------------------------------------------------------------

fn missing_field_rule(key: &str) -> String {
    format!("docs/frontmatter/missing-{key}")
}

// ---------------------------------------------------------------------------
// Validator impl
// ---------------------------------------------------------------------------

impl Validator for DocsValidator {
    fn patterns(&self) -> &[&str] {
        std::slice::from_ref(&self.glob)
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        let schema = &self.schema;

        // Skip files with no frontmatter fence.
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
                    .with_rule("docs/frontmatter/unclosed-fence", Difficulty::Easy),
                ];
            }
            Err(ParseError::NoFence) => return vec![],
        };

        let mut diags = Vec::new();

        // Helper: find a non-empty field value.
        let get = |key: &str| -> Option<(&str, usize)> {
            fields
                .iter()
                .find(|f| f.key == key && !f.value.trim().is_empty())
                .map(|f| (f.value.trim(), f.line))
        };

        // Match filename against conventions.
        let convention_match = match_convention(path, &schema.conventions);

        if convention_match.is_none() && !schema.conventions.is_empty() {
            let formats: Vec<&str> = schema
                .conventions
                .iter()
                .map(|c| c.format.as_str())
                .collect();
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    format!(
                        "filename does not match any configured convention; \
                         expected one of: {}",
                        formats.join(", ")
                    ),
                )
                .with_rule("docs/frontmatter/invalid-filename", Difficulty::Easy),
            );
        }

        // Required field presence.
        for key in &schema.required_fields {
            let rule_id: &'static str = Box::leak(missing_field_rule(key).into_boxed_str());
            let present = fields.iter().any(|f| f.key == key.as_str());
            let non_empty = fields
                .iter()
                .any(|f| f.key == key.as_str() && !f.value.trim().is_empty());
            if !present {
                diags.push(
                    Diagnostic::error(path, 1, 1, format!("missing required field '{key}'"))
                        .with_rule(rule_id, Difficulty::Easy),
                );
            } else if !non_empty {
                let line = fields
                    .iter()
                    .find(|f| f.key == key.as_str())
                    .map_or(1, |f| f.line);
                diags.push(
                    Diagnostic::error(
                        path,
                        line,
                        1,
                        format!("required field '{key}' must not be empty"),
                    )
                    .with_rule(rule_id, Difficulty::Easy),
                );
            }
        }

        // Enum: status.
        if let Some((status_val, line)) = get("status") {
            if !schema.statuses.iter().any(|s| s == status_val) {
                diags.push(
                    Diagnostic::error(
                        path,
                        line,
                        1,
                        format!(
                            "unknown status '{status_val}'; expected one of: {}",
                            schema.statuses.join(", ")
                        ),
                    )
                    .with_rule("docs/frontmatter/unknown-status", Difficulty::Easy),
                );
            }
        }

        // Enum: doctype.
        if let Some((doctype_val, line)) = get("doctype") {
            if !schema.doctypes.iter().any(|d| d == doctype_val) {
                diags.push(
                    Diagnostic::error(
                        path,
                        line,
                        1,
                        format!(
                            "unknown doctype '{doctype_val}'; expected one of: {}",
                            schema.doctypes.join(", ")
                        ),
                    )
                    .with_rule("docs/frontmatter/unknown-doctype", Difficulty::Easy),
                );
            }
        }

        // Date fields.
        for date_key in &schema.date_fields {
            if let Some((date_val, line)) = get(date_key) {
                if !is_valid_date(date_val) {
                    diags.push(
                        Diagnostic::error(
                            path,
                            line,
                            1,
                            format!(
                                "field '{date_key}' value '{date_val}' is not a valid \
                                 YYYY-MM-DD date"
                            ),
                        )
                        .with_rule("docs/frontmatter/invalid-date", Difficulty::Easy),
                    );
                }
            }
        }

        // Cross-field validation: tokens that correspond to frontmatter fields
        // must match the frontmatter values.
        //
        // Known field-mapped tokens: doctype, project, status.
        // title is derived as "{topic}-{doctype}" or "{project}-{doctype}".
        if let Some((_conv, tokens)) = &convention_match {
            // Tokens that map 1:1 to frontmatter fields.
            for field_token in &["doctype", "project", "status"] {
                if let Some(token_val) = tokens.get(*field_token) {
                    if let Some((field_val, line)) = get(field_token) {
                        if field_val != token_val {
                            let rule_id: &'static str = Box::leak(
                                format!("docs/frontmatter/{field_token}-mismatch").into_boxed_str(),
                            );
                            diags.push(
                                Diagnostic::error(
                                    path,
                                    line,
                                    1,
                                    format!(
                                        "{field_token} field '{field_val}' does not match \
                                         filename-derived value '{token_val}'"
                                    ),
                                )
                                .with_rule(rule_id, Difficulty::Easy),
                            );
                        }
                    }
                }
            }

            // title: expected as "{topic}-{doctype}" when both tokens present,
            // or "{project}-{doctype}" for the repo-doc convention.
            let title_base = tokens
                .get("topic")
                .or_else(|| tokens.get("project"))
                .cloned();
            let title_dt = tokens.get("doctype").cloned();
            if let (Some(base), Some(dt)) = (title_base, title_dt) {
                let expected_title = format!("{base}-{dt}");
                if let Some((title_val, line)) = get("title") {
                    if title_val != expected_title {
                        diags.push(
                            Diagnostic::error(
                                path,
                                line,
                                1,
                                format!(
                                    "title '{title_val}' does not match filename-derived id \
                                     '{expected_title}'"
                                ),
                            )
                            .with_rule("docs/frontmatter/title-mismatch", Difficulty::Easy),
                        );
                    }
                }
            }

            // plan docs must have a meta block.
            if tokens.get("doctype").map(|s| s.as_str()) == Some("plan")
                && !fields.iter().any(|f| f.key == "meta")
            {
                diags.push(
                    Diagnostic::warning(
                        path,
                        1,
                        1,
                        "plan doc has no 'meta' block; add `meta:\\n  spec: \
                         <path>` to link this plan to its upstream spec",
                    )
                    .with_rule("docs/frontmatter/plan-missing-spec-ref", Difficulty::Easy),
                );
            }
        }

        // meta: if present, must be a non-empty YAML mapping.
        if let Some(meta_field) = fields.iter().find(|f| f.key == "meta") {
            let v = meta_field.value.trim();
            if v.is_empty() {
                diags.push(
                    Diagnostic::error(
                        path,
                        meta_field.line,
                        1,
                        "'meta' field is present but empty; \
                         provide a JSON object e.g. {\"spec\": \"path/to/spec.md\"}",
                    )
                    .with_rule("docs/frontmatter/empty-meta", Difficulty::Easy),
                );
            } else {
                match serde_yaml::from_str::<serde_yaml::Value>(v) {
                    Ok(serde_yaml::Value::Mapping(_)) => {}
                    Ok(_) => {
                        diags.push(
                            Diagnostic::error(
                                path,
                                meta_field.line,
                                1,
                                "'meta' value must be a mapping (got a non-object value); \
                                 use inline JSON {\"key\": \"val\"} or indented YAML keys",
                            )
                            .with_rule("docs/frontmatter/meta-not-object", Difficulty::Easy),
                        );
                    }
                    Err(_) => {
                        diags.push(
                            Diagnostic::error(
                                path,
                                meta_field.line,
                                1,
                                "'meta' value could not be parsed; \
                                 use inline JSON {\"key\": \"val\"} or indented YAML keys",
                            )
                            .with_rule("docs/frontmatter/meta-invalid-json", Difficulty::Easy),
                        );
                    }
                }
            }
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
    use std::path::Path;

    fn v() -> DocsValidator {
        DocsValidator::default()
    }

    // --- fence skip ---

    #[test]
    fn no_fence_is_skipped() {
        let src = "# My Doc\nno frontmatter here\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    // --- repo doc: valid ---

    #[test]
    fn repo_doc_valid() {
        let src = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n# body\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn repo_doc_with_meta_valid() {
        let src = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n\
                   meta: {\"author\": \"Joe\"}\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    // --- repo doc: missing required fields ---

    #[test]
    fn missing_title_is_error() {
        let src = "---\ndoctype: roadmap\nproject: agentlint\nstatus: active\n\
                   created: 2026-05-16\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("missing-title")),
            "expected missing-title: {diags:?}"
        );
    }

    #[test]
    fn missing_doctype_is_error() {
        let src = "---\ntitle: agentlint-roadmap\nproject: agentlint\nstatus: active\n\
                   created: 2026-05-16\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("missing-doctype")),
            "expected missing-doctype: {diags:?}"
        );
    }

    // --- repo doc: cross-field validation ---

    #[test]
    fn title_mismatch_is_error() {
        let src = "---\ntitle: wrong-title\ndoctype: roadmap\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/title-mismatch"),
            "expected title-mismatch: {diags:?}"
        );
    }

    #[test]
    fn doctype_mismatch_is_error() {
        let src = "---\ntitle: agentlint-guide\ndoctype: guide\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/doctype-mismatch"),
            "expected doctype-mismatch: {diags:?}"
        );
    }

    #[test]
    fn project_mismatch_is_error() {
        let src = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: other\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/project-mismatch"),
            "expected project-mismatch: {diags:?}"
        );
    }

    // --- enum validation ---

    #[test]
    fn unknown_status_is_error() {
        let src = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: agentlint\n\
                   status: wip\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/unknown-status"),
            "expected unknown-status: {diags:?}"
        );
    }

    #[test]
    fn unknown_doctype_is_error() {
        let src = "---\ntitle: agentlint-wiki\ndoctype: wiki\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/unknown-doctype"),
            "expected unknown-doctype: {diags:?}"
        );
    }

    // --- date validation ---

    #[test]
    fn invalid_date_format_is_error() {
        let src = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: agentlint\n\
                   status: active\ncreated: 16-05-2026\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/invalid-date"),
            "expected invalid-date: {diags:?}"
        );
    }

    #[test]
    fn invalid_month_is_error() {
        let src = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: agentlint\n\
                   status: active\ncreated: 2026-13-01\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/invalid-date"),
            "expected invalid-date for month 13: {diags:?}"
        );
    }

    // --- research doc: valid ---

    #[test]
    fn research_spec_valid() {
        let src = "---\ntitle: agentlint-docs-spec\ndoctype: spec\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/specs/20260516-agentlint-docs.spec.md"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn research_plan_with_meta_valid() {
        let src = "---\ntitle: agentlint-docs-plan\ndoctype: plan\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n\
                   meta: {\"spec\": \"docs/specs/20260516-agentlint-docs.spec.md\"}\n---\n";
        let diags = v().validate(Path::new("docs/plans/20260516-agentlint-docs.plan.md"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    // --- plan without meta warns ---

    #[test]
    fn plan_without_meta_warns() {
        let src = "---\ntitle: agentlint-docs-plan\ndoctype: plan\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/plans/20260516-agentlint-docs.plan.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/plan-missing-spec-ref"),
            "expected plan-missing-spec-ref: {diags:?}"
        );
    }

    // --- path-inferred doctype ---

    #[test]
    fn path_inferred_plan_doctype() {
        // Legacy file with no explicit doctype suffix — inferred from docs/plans/
        let src = "---\ntitle: agentlint-plan\ndoctype: plan\nproject: agentlint\n\
                   status: draft\ncreated: 2026-05-15\nupdated: 2026-05-15\n\
                   meta: {\"spec\": \"docs/specs/20260515.spec.md\"}\n---\n";
        let diags = v().validate(Path::new("docs/plans/2026-05-15-agentlint.md"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    // --- invalid filename ---

    #[test]
    fn invalid_filename_warns() {
        let src = "---\ntitle: something\ndoctype: guide\nproject: foo\nstatus: active\n\
                   created: 2026-05-16\nupdated: 2026-05-16\n---\n";
        let diags = v().validate(Path::new("docs/some-random-name.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/invalid-filename"),
            "expected invalid-filename: {diags:?}"
        );
    }

    // --- meta checks ---

    #[test]
    fn empty_meta_is_error() {
        let src = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\nmeta:\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags.iter().any(|d| d.rule == "docs/frontmatter/empty-meta"
                && d.severity == agentlint_core::Severity::Error),
            "expected empty-meta error: {diags:?}"
        );
    }

    #[test]
    fn meta_scalar_string_is_error() {
        let src = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n\
                   meta: not-a-mapping\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/meta-not-object"),
            "expected meta-not-object error: {diags:?}"
        );
    }

    #[test]
    fn meta_json_array_is_error() {
        let src = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n\
                   meta: [\"a\", \"b\"]\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/meta-not-object"),
            "expected meta-not-object error: {diags:?}"
        );
    }

    #[test]
    fn meta_yaml_dict_indented_is_valid() {
        let src = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\nmeta:\n  \
                   author: Joe\n  tags: [foo, bar]\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            !diags
                .iter()
                .any(|d| d.rule.starts_with("docs/frontmatter/meta")),
            "valid YAML dict meta should produce no meta errors: {diags:?}"
        );
    }

    // --- custom schema via DocsSchema ---

    #[test]
    fn custom_schema_accepts_custom_doctype() {
        let schema = DocsSchema {
            doctypes: vec!["wiki".into(), "note".into()],
            statuses: vec!["open".into(), "closed".into()],
            required_fields: vec!["title".into(), "doctype".into(), "status".into()],
            date_fields: vec![],
            ..DocsSchema::default()
        };
        let v = DocsValidator::new(schema);
        let src = "---\ntitle: agentlint-wiki\ndoctype: wiki\nstatus: open\n---\n";
        let diags = v.validate(Path::new("docs/wiki.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .all(|d| d.severity == agentlint_core::Severity::Warning
                    || !d.rule.contains("unknown-doctype")),
            "custom doctype 'wiki' should be accepted: {diags:?}"
        );
    }

    #[test]
    fn custom_schema_rejects_default_doctypes() {
        let schema = DocsSchema {
            doctypes: vec!["wiki".into()],
            statuses: vec!["open".into()],
            required_fields: vec!["title".into(), "doctype".into(), "status".into()],
            date_fields: vec![],
            ..DocsSchema::default()
        };
        let v = DocsValidator::new(schema);
        let src = "---\ntitle: agentlint-spec\ndoctype: spec\nstatus: open\n---\n";
        let diags = v.validate(Path::new("docs/spec.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/unknown-doctype"),
            "default doctype 'spec' should be rejected by custom schema: {diags:?}"
        );
    }

    // --- all default doctypes accepted ---

    #[test]
    fn all_default_doctypes_accepted() {
        let schema = DocsSchema::default();
        for dt in &schema.doctypes.clone() {
            let src = format!(
                "---\ntitle: agentlint-{dt}\ndoctype: {dt}\nproject: agentlint\n\
                 status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n"
            );
            let path = format!("docs/{dt}.agentlint.md");
            let v = DocsValidator::new(schema.clone());
            let diags = v.validate(Path::new(&path), &src);
            let errors: Vec<_> = diags
                .iter()
                .filter(|d| {
                    matches!(d.severity, agentlint_core::Severity::Error)
                        && d.rule != "docs/frontmatter/plan-missing-spec-ref"
                })
                .collect();
            assert!(
                errors.is_empty(),
                "unexpected errors for doctype '{dt}': {diags:?}"
            );
        }
    }
}
