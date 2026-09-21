//! DeclarativeValidator struct, construction, and Validator trait impl.

use crate::{Diagnostic, Difficulty, Severity, Validator};
use std::collections::HashMap;
use std::path::Path;

use super::parse::{ParsedContent, parse_frontmatter, parse_json, parse_yaml};
use super::types::{Check, DifficultyDef, Format, RuleDef, SeverityDef, ValidatorDef, ValuesOrRef};

/// A single declarative validator instantiated from a `ValidatorDef`.
///
/// Holds leaked `&'static str` pattern slices so it can implement `Validator`
/// (which requires `&[&str]`). This is acceptable because plugins are loaded
/// once at startup and live for the process lifetime.
pub struct DeclarativeValidator {
    pub(super) patterns: Vec<&'static str>,
    pub(super) def: ValidatorDef,
    /// Resolved constant lists (e.g. `$known_triggers` -> vec of strings).
    pub(super) constants: HashMap<String, Vec<String>>,
    /// Leaked rule IDs for Diagnostic::with_rule (requires &'static str).
    pub(super) rule_ids: Vec<&'static str>,
}

impl DeclarativeValidator {
    /// Builds a validator from a parsed definition and resolves its constant references.
    pub fn new(def: ValidatorDef, constants: &HashMap<String, toml::Value>) -> Self {
        let patterns: Vec<&'static str> = def
            .patterns
            .iter()
            .map(|p| &*Box::leak(p.clone().into_boxed_str()))
            .collect();

        let rule_ids: Vec<&'static str> = def
            .rules
            .iter()
            .map(|r| &*Box::leak(r.id.clone().into_boxed_str()))
            .collect();

        // Resolve constants from toml::Value -> Vec<String>.
        let mut resolved = HashMap::new();
        for (k, v) in constants {
            if let Some(arr) = v.as_array() {
                let strings: Vec<String> = arr
                    .iter()
                    .filter_map(|item| item.as_str().map(String::from))
                    .collect();
                resolved.insert(k.clone(), strings);
            }
        }

        Self {
            patterns,
            def,
            constants: resolved,
            rule_ids,
        }
    }

    pub(super) fn resolve_values(&self, v: &ValuesOrRef) -> Vec<String> {
        match v {
            ValuesOrRef::Inline(list) => list.clone(),
            ValuesOrRef::Ref(r) => {
                let key = r.strip_prefix('$').unwrap_or(r);
                self.constants.get(key).cloned().unwrap_or_default()
            }
        }
    }

    pub(super) fn make_diag(
        &self,
        path: &Path,
        rule_idx: usize,
        rule: &RuleDef,
        extra: Option<&str>,
    ) -> Diagnostic {
        let msg = if let Some(extra) = extra {
            format!("{}: {extra}", rule.message)
        } else {
            rule.message.clone()
        };

        let severity = match rule.severity {
            SeverityDef::Error => Severity::Error,
            SeverityDef::Warning => Severity::Warning,
        };

        let difficulty = match rule.difficulty {
            DifficultyDef::Easy => Difficulty::Easy,
            DifficultyDef::Normal => Difficulty::Normal,
            DifficultyDef::Hard => Difficulty::Hard,
            DifficultyDef::Painful => Difficulty::Painful,
        };

        Diagnostic {
            path: path.into(),
            line: 1,
            col: 1,
            severity,
            message: msg,
            rule: self.rule_ids[rule_idx],
            difficulty,
        }
    }
}

impl Validator for DeclarativeValidator {
    fn patterns(&self) -> &[&str] {
        &self.patterns
    }

    fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
        // Check skip_filenames.
        if let Some(fname) = path.file_name().and_then(|n| n.to_str())
            && self
                .def
                .skip_filenames
                .iter()
                .any(|s| s.eq_ignore_ascii_case(fname))
        {
            return vec![];
        }

        let mut diags = Vec::new();

        // Parse the content based on format.
        let parsed = match self.def.format {
            Format::Yaml => parse_yaml(src),
            Format::Json => parse_json(src),
            Format::Frontmatter => {
                let p = parse_frontmatter(src);
                // When frontmatter is optional and the file has no fence, skip.
                if self.def.frontmatter_optional
                    && let ParsedContent::Error(ref e) = p
                    && e == "no frontmatter"
                {
                    return vec![];
                }
                p
            }
            Format::Markdown => ParsedContent::Markdown(src.to_string()),
        };

        for (idx, rule) in self.def.rules.iter().enumerate() {
            if let Some(d) = self.eval_rule(path, src, idx, rule, &parsed) {
                diags.push(d);
                // If the file is empty or unparseable, stop after the first
                // diagnostic — further field checks are meaningless.
                if matches!(
                    rule.check,
                    Check::NonEmpty
                        | Check::ValidParse
                        | Check::HasFrontmatter
                        | Check::FrontmatterClosed
                ) {
                    break;
                }
            }
        }

        diags
    }
}
