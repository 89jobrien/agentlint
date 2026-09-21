//! Infers documentation schemas from existing Markdown frontmatter.

use crate::{DocsSchema, FilenameConvention};
use agentlint_frontmatter::parse;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Derive a [`DocsSchema`] from a corpus of parsed doc files.
///
/// Heuristics:
/// - `required_fields`: fields present in ≥80% of files
/// - `date_fields`: fields where ≥90% of non-empty values match `YYYY-MM-DD`
/// - `doctypes` / `statuses`: all unique observed values for those keys
/// - `conventions`: presence-based cluster (research dirs → research pattern;
///   catch-all always appended)
pub fn infer_schema(corpus: &[(&Path, &str)]) -> DocsSchema {
    let total = corpus.len();
    if total == 0 {
        return DocsSchema {
            file_glob: "docs/**/*.md".into(),
            required_fields: vec![],
            doctypes: vec![],
            statuses: vec![],
            date_fields: vec![],
            conventions: default_conventions(),
        };
    }

    let mut field_counts: HashMap<String, usize> = HashMap::new();
    let mut field_values: HashMap<String, Vec<String>> = HashMap::new();
    let mut field_date_counts: HashMap<String, usize> = HashMap::new();

    for (_path, src) in corpus {
        if let Ok(fields) = parse(src) {
            for field in &fields {
                *field_counts.entry(field.key.clone()).or_default() += 1;
                let val = field.value.trim().to_string();
                if !val.is_empty() {
                    field_values
                        .entry(field.key.clone())
                        .or_default()
                        .push(val.clone());
                    if is_valid_date(&val) {
                        *field_date_counts.entry(field.key.clone()).or_default() += 1;
                    }
                }
            }
        }
    }

    let threshold = ((total as f64) * 0.8).ceil() as usize;
    let mut required_fields: Vec<String> = field_counts
        .iter()
        .filter(|(_, count)| **count >= threshold)
        .map(|(k, _)| k.clone())
        .collect();
    required_fields.sort();

    let mut date_fields: Vec<String> = field_date_counts
        .iter()
        .filter(|(k, date_count)| {
            let total_vals = field_values.get(k.as_str()).map_or(0, |v| v.len());
            total_vals > 0 && (**date_count as f64) / (total_vals as f64) >= 0.9
        })
        .map(|(k, _)| k.clone())
        .collect();
    date_fields.sort();

    let doctypes = unique_sorted(&field_values, "doctype");
    let statuses = unique_sorted(&field_values, "status");
    let conventions = infer_conventions(corpus);

    DocsSchema {
        file_glob: "docs/**/*.md".into(),
        required_fields,
        doctypes,
        statuses,
        date_fields,
        conventions,
    }
}

fn unique_sorted(field_values: &HashMap<String, Vec<String>>, key: &str) -> Vec<String> {
    let set: HashSet<String> = field_values
        .get(key)
        .map(|v| v.iter().cloned().collect())
        .unwrap_or_default();
    let mut vals: Vec<String> = set.into_iter().collect();
    vals.sort();
    vals
}

fn infer_conventions(corpus: &[(&Path, &str)]) -> Vec<FilenameConvention> {
    let research_dirs = ["ideas", "specs", "plans"];
    let has_research = corpus.iter().any(|(path, _)| {
        path.parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|n| research_dirs.contains(&n))
            .unwrap_or(false)
    });
    let mut conventions = Vec::new();
    if has_research {
        conventions.push(FilenameConvention {
            format: "{ref}-{topic}.{doctype}.md".into(),
            dirs: vec!["ideas".into(), "specs".into(), "plans".into()],
        });
        conventions.push(FilenameConvention {
            format: "{ref}-{topic}.md".into(),
            dirs: vec!["ideas".into(), "specs".into(), "plans".into()],
        });
    }
    conventions.push(FilenameConvention {
        format: "{doctype}.{project}.md".into(),
        dirs: vec![],
    });
    conventions
}

fn default_conventions() -> Vec<FilenameConvention> {
    infer_conventions(&[])
}

pub(crate) fn is_valid_date(s: &str) -> bool {
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

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(files: &[(&str, &str)]) -> Vec<(std::path::PathBuf, String)> {
        files
            .iter()
            .map(|(p, s)| (std::path::PathBuf::from(p), s.to_string()))
            .collect()
    }

    fn refs<'a>(files: &'a [(std::path::PathBuf, String)]) -> Vec<(&'a Path, &'a str)> {
        files
            .iter()
            .map(|(p, s)| (p.as_path(), s.as_str()))
            .collect()
    }

    #[test]
    fn infers_required_fields_from_majority() {
        let files = corpus(&[
            (
                "docs/a.md",
                "---\ntitle: A\ndoctype: spec\nstatus: draft\n---\n",
            ),
            (
                "docs/b.md",
                "---\ntitle: B\ndoctype: plan\nstatus: draft\n---\n",
            ),
            // status absent here — only 2/3 = 67%, below 80% threshold
            ("docs/c.md", "---\ntitle: C\ndoctype: guide\n---\n"),
        ]);
        let schema = infer_schema(&refs(&files));
        assert!(schema.required_fields.contains(&"title".to_string()));
        assert!(schema.required_fields.contains(&"doctype".to_string()));
        // status is 67% — below threshold, must NOT be required
        assert!(!schema.required_fields.contains(&"status".to_string()));
    }

    #[test]
    fn infers_doctypes_from_observed_values() {
        let files = corpus(&[
            ("docs/a.md", "---\ndoctype: spec\n---\n"),
            ("docs/b.md", "---\ndoctype: plan\n---\n"),
            ("docs/c.md", "---\ndoctype: spec\n---\n"),
        ]);
        let schema = infer_schema(&refs(&files));
        assert!(schema.doctypes.contains(&"spec".to_string()));
        assert!(schema.doctypes.contains(&"plan".to_string()));
        assert_eq!(schema.doctypes.len(), 2);
    }

    #[test]
    fn infers_date_fields() {
        let files = corpus(&[
            (
                "docs/a.md",
                "---\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n",
            ),
            (
                "docs/b.md",
                "---\ncreated: 2026-01-01\nupdated: 2026-01-02\n---\n",
            ),
        ]);
        let schema = infer_schema(&refs(&files));
        assert!(schema.date_fields.contains(&"created".to_string()));
        assert!(schema.date_fields.contains(&"updated".to_string()));
    }

    #[test]
    fn empty_corpus_returns_empty_schema_fields() {
        let schema = infer_schema(&[]);
        assert!(schema.required_fields.is_empty());
        assert!(schema.doctypes.is_empty());
    }

    #[test]
    fn research_dir_files_produce_research_conventions() {
        let files = corpus(&[(
            "docs/specs/20260516-foo.spec.md",
            "---\ndoctype: spec\n---\n",
        )]);
        let schema = infer_schema(&refs(&files));
        assert!(
            schema
                .conventions
                .iter()
                .any(|c| c.dirs.contains(&"specs".to_string()))
        );
    }
}
