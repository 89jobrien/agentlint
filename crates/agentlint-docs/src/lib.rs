use agentlint_core::{Diagnostic, Difficulty, Validator};
use agentlint_frontmatter::{ParseError, parse};
use std::path::Path;

pub struct DocsValidator;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

const KNOWN_DOCTYPES: &[&str] = &[
    "idea",
    "spec",
    "plan",
    "adr",
    "roadmap",
    "guide",
    "reference",
    "runbook",
    "architecture",
    "capability-matrix",
    "testing",
    "development",
    "readme",
];

const KNOWN_STATUSES: &[&str] = &["draft", "active", "archived", "superseded"];

/// (field name, rule id) pairs for required-field checks.
const REQUIRED_FIELDS: &[(&str, &str)] = &[
    ("title", "docs/frontmatter/missing-title"),
    ("doctype", "docs/frontmatter/missing-doctype"),
    ("project", "docs/frontmatter/missing-project"),
    ("status", "docs/frontmatter/missing-status"),
    ("created", "docs/frontmatter/missing-created"),
    ("updated", "docs/frontmatter/missing-updated"),
];

// ---------------------------------------------------------------------------
// Filename parsing
// ---------------------------------------------------------------------------

/// Information extracted from a doc filename.
enum DocKind {
    /// `docs/{doctype}.{stub}.md` — project-bound repo doc.
    Repo { doctype: String, stub: String },
    /// `docs/{dir}/{ref}-{topic}.{doctype}.md` — research doc.
    Research { doctype: String, topic: String },
    /// Filename does not match either convention.
    Unknown,
}

fn parse_filename(path: &Path) -> DocKind {
    let stem = match path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return DocKind::Unknown,
    };
    let parent_name = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");

    match parent_name {
        "ideas" | "specs" | "plans" => parse_research(stem, parent_name),
        _ => parse_repo(stem),
    }
}

/// Repo doc: `{doctype}.{stub}` — exactly one dot, both parts non-empty.
fn parse_repo(stem: &str) -> DocKind {
    let mut parts = stem.splitn(2, '.');
    match (parts.next(), parts.next()) {
        (Some(dt), Some(stub)) if !dt.is_empty() && !stub.is_empty() => DocKind::Repo {
            doctype: dt.to_string(),
            stub: stub.to_string(),
        },
        _ => DocKind::Unknown,
    }
}

/// Research doc: `{ref}-{topic}.{doctype}` with path-based doctype inference as fallback.
fn parse_research(stem: &str, parent_dir: &str) -> DocKind {
    if let Some(dot) = stem.rfind('.') {
        let prefix = &stem[..dot];
        let doctype_str = &stem[dot + 1..];
        if !doctype_str.is_empty() && !prefix.is_empty() {
            return DocKind::Research {
                doctype: doctype_str.to_string(),
                topic: strip_ref(prefix).to_string(),
            };
        }
    }
    // No explicit doctype suffix — infer from directory.
    let inferred = match parent_dir {
        "ideas" => "idea",
        "specs" => "spec",
        "plans" => "plan",
        _ => return DocKind::Unknown,
    };
    DocKind::Research {
        doctype: inferred.to_string(),
        topic: strip_ref(stem).to_string(),
    }
}

/// Strip a leading date or ticket-number ref before the topic.
///
/// Handles:
/// - `YYYY-MM-DD-{topic}` — 10-char ISO date prefix
/// - `YYYYMMDD-{topic}` — 8-digit compact date prefix
/// - `{digits}-{topic}` — ticket number prefix
fn strip_ref(s: &str) -> &str {
    // Check for YYYY-MM-DD- prefix (11 chars: 10-char date + hyphen).
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
            return &s[11..];
        }
    }
    // Fall back: all-digit prefix before the first hyphen (YYYYMMDD or ticket number).
    if let Some(pos) = s.find('-') {
        let prefix = &s[..pos];
        if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
            return &s[pos + 1..];
        }
    }
    s
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
// Validator
// ---------------------------------------------------------------------------

impl Validator for DocsValidator {
    fn patterns(&self) -> &[&str] {
        &["docs/**/*.md"]
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
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

        // Parse filename.
        let kind = parse_filename(path);

        if matches!(kind, DocKind::Unknown) {
            diags.push(
                Diagnostic::warning(
                    path,
                    1,
                    1,
                    "filename does not match a known doc convention; \
                     expected `{doctype}.{project}.md` (repo doc) or \
                     `{ref}-{topic}.{doctype}.md` in docs/ideas|specs|plans/",
                )
                .with_rule("docs/frontmatter/invalid-filename", Difficulty::Easy),
            );
        }

        // Helper: find a non-empty field value.
        let get = |key: &str| -> Option<(&str, usize)> {
            fields
                .iter()
                .find(|f| f.key == key && !f.value.trim().is_empty())
                .map(|f| (f.value.trim(), f.line))
        };

        // Required field presence.
        for &(key, rule) in REQUIRED_FIELDS {
            let present = fields.iter().any(|f| f.key == key);
            let non_empty = fields
                .iter()
                .any(|f| f.key == key && !f.value.trim().is_empty());
            if !present {
                diags.push(
                    Diagnostic::error(path, 1, 1, format!("missing required field '{key}'"))
                        .with_rule(rule, Difficulty::Easy),
                );
            } else if !non_empty {
                let line = fields.iter().find(|f| f.key == key).map_or(1, |f| f.line);
                diags.push(
                    Diagnostic::error(
                        path,
                        line,
                        1,
                        format!("required field '{key}' must not be empty"),
                    )
                    .with_rule(rule, Difficulty::Easy),
                );
            }
        }

        // Enum: status.
        if let Some((status_val, line)) = get("status") {
            if !KNOWN_STATUSES.contains(&status_val) {
                diags.push(
                    Diagnostic::error(
                        path,
                        line,
                        1,
                        format!(
                            "unknown status '{status_val}'; expected one of: {}",
                            KNOWN_STATUSES.join(", ")
                        ),
                    )
                    .with_rule("docs/frontmatter/unknown-status", Difficulty::Easy),
                );
            }
        }

        // Enum: doctype.
        if let Some((doctype_val, line)) = get("doctype") {
            if !KNOWN_DOCTYPES.contains(&doctype_val) {
                diags.push(
                    Diagnostic::error(
                        path,
                        line,
                        1,
                        format!(
                            "unknown doctype '{doctype_val}'; expected one of: {}",
                            KNOWN_DOCTYPES.join(", ")
                        ),
                    )
                    .with_rule("docs/frontmatter/unknown-doctype", Difficulty::Easy),
                );
            }
        }

        // Date fields.
        for date_key in ["created", "updated"] {
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

        // Cross-field validation.
        match &kind {
            DocKind::Repo { doctype, stub } => {
                if let Some((title_val, line)) = get("title") {
                    let expected = format!("{stub}-{doctype}");
                    if title_val != expected {
                        diags.push(
                            Diagnostic::error(
                                path,
                                line,
                                1,
                                format!(
                                    "title '{title_val}' does not match filename-derived id \
                                     '{expected}'"
                                ),
                            )
                            .with_rule("docs/frontmatter/title-mismatch", Difficulty::Easy),
                        );
                    }
                }
                if let Some((doctype_val, line)) = get("doctype") {
                    if doctype_val != doctype.as_str() {
                        diags.push(
                            Diagnostic::error(
                                path,
                                line,
                                1,
                                format!(
                                    "doctype field '{doctype_val}' does not match filename \
                                     doctype '{doctype}'"
                                ),
                            )
                            .with_rule("docs/frontmatter/doctype-mismatch", Difficulty::Easy),
                        );
                    }
                }
                if let Some((project_val, line)) = get("project") {
                    if project_val != stub.as_str() {
                        diags.push(
                            Diagnostic::error(
                                path,
                                line,
                                1,
                                format!(
                                    "project field '{project_val}' does not match filename \
                                     stub '{stub}'"
                                ),
                            )
                            .with_rule("docs/frontmatter/project-mismatch", Difficulty::Easy),
                        );
                    }
                }
            }
            DocKind::Research { doctype, topic } => {
                if let Some((title_val, line)) = get("title") {
                    let expected = format!("{topic}-{doctype}");
                    if title_val != expected {
                        diags.push(
                            Diagnostic::error(
                                path,
                                line,
                                1,
                                format!(
                                    "title '{title_val}' does not match filename-derived id \
                                     '{expected}'"
                                ),
                            )
                            .with_rule("docs/frontmatter/title-mismatch", Difficulty::Easy),
                        );
                    }
                }
                if let Some((doctype_val, line)) = get("doctype") {
                    if doctype_val != doctype.as_str() {
                        diags.push(
                            Diagnostic::error(
                                path,
                                line,
                                1,
                                format!(
                                    "doctype field '{doctype_val}' does not match filename \
                                     doctype '{doctype}'"
                                ),
                            )
                            .with_rule("docs/frontmatter/doctype-mismatch", Difficulty::Easy),
                        );
                    }
                }
                // plan docs must have a meta block (to carry the spec reference).
                if doctype == "plan" && !fields.iter().any(|f| f.key == "meta") {
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
                // spec docs: project field not cross-validated against filename.
            }
            DocKind::Unknown => {}
        }

        // meta: if present, must not be empty.
        if let Some(meta_field) = fields.iter().find(|f| f.key == "meta") {
            if meta_field.value.trim().is_empty() {
                diags.push(
                    Diagnostic::warning(
                        path,
                        meta_field.line,
                        1,
                        "'meta' field is present but empty; add content or remove the key",
                    )
                    .with_rule("docs/frontmatter/empty-meta", Difficulty::Easy),
                );
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
        DocsValidator
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
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\nmeta: |\n  \
                   author: Joe\n---\n";
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
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\nmeta: |\n  \
                   spec: docs/specs/20260516-agentlint-docs.spec.md\n---\n";
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
                   status: draft\ncreated: 2026-05-15\nupdated: 2026-05-15\nmeta: |\n  \
                   spec: docs/specs/20260515.spec.md\n---\n";
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
    fn empty_meta_warns() {
        let src = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\nmeta:\n---\n";
        let diags = v().validate(Path::new("docs/roadmap.agentlint.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule == "docs/frontmatter/empty-meta"),
            "expected empty-meta: {diags:?}"
        );
    }

    // --- all known doctypes accepted ---

    #[test]
    fn all_known_doctypes_accepted() {
        for dt in super::KNOWN_DOCTYPES {
            let src = format!(
                "---\ntitle: agentlint-{dt}\ndoctype: {dt}\nproject: agentlint\n\
                 status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n"
            );
            let path = format!("docs/{dt}.agentlint.md");
            let diags = v().validate(Path::new(&path), &src);
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
