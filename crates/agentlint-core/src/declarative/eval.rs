//! Runtime declarative validator: evaluates rules against parsed content.

use crate::{Diagnostic, Difficulty, Severity, Validator};
use std::collections::HashMap;
use std::path::Path;

use super::parse::{
    ParsedContent, get_field_str, has_field, has_field_with_value, parse_frontmatter, parse_json,
    parse_yaml,
};
use super::types::{Check, DifficultyDef, Format, RuleDef, SeverityDef, ValidatorDef, ValuesOrRef};

/// A single declarative validator instantiated from a `ValidatorDef`.
///
/// Holds leaked `&'static str` pattern slices so it can implement `Validator`
/// (which requires `&[&str]`). This is acceptable because plugins are loaded
/// once at startup and live for the process lifetime.
pub struct DeclarativeValidator {
    patterns: Vec<&'static str>,
    def: ValidatorDef,
    /// Resolved constant lists (e.g. `$known_triggers` -> vec of strings).
    constants: HashMap<String, Vec<String>>,
    /// Leaked rule IDs for Diagnostic::with_rule (requires &'static str).
    rule_ids: Vec<&'static str>,
}

impl DeclarativeValidator {
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

    fn resolve_values(&self, v: &ValuesOrRef) -> Vec<String> {
        match v {
            ValuesOrRef::Inline(list) => list.clone(),
            ValuesOrRef::Ref(r) => {
                let key = r.strip_prefix('$').unwrap_or(r);
                self.constants.get(key).cloned().unwrap_or_default()
            }
        }
    }

    fn make_diag(
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

impl DeclarativeValidator {
    fn eval_rule(
        &self,
        path: &Path,
        src: &str,
        idx: usize,
        rule: &RuleDef,
        parsed: &ParsedContent,
    ) -> Option<Diagnostic> {
        match rule.check {
            Check::NonEmpty => {
                if src.trim().is_empty() {
                    return Some(self.make_diag(path, idx, rule, None));
                }
            }
            Check::ValidParse => {
                if matches!(parsed, ParsedContent::Error(_)) {
                    let msg = match parsed {
                        ParsedContent::Error(e) => e.as_str(),
                        _ => unreachable!(),
                    };
                    return Some(self.make_diag(path, idx, rule, Some(msg)));
                }
            }
            Check::HasFrontmatter => {
                if matches!(parsed, ParsedContent::Error(_) | ParsedContent::Markdown(_)) {
                    return Some(self.make_diag(path, idx, rule, None));
                }
                if let ParsedContent::Fields(fields) = parsed
                    && fields.is_empty()
                {
                    return Some(self.make_diag(path, idx, rule, None));
                }
            }
            Check::FrontmatterClosed => {
                let is_unclosed = if let ParsedContent::Error(e) = parsed {
                    e == "unclosed frontmatter"
                } else {
                    false
                };
                if is_unclosed {
                    return Some(self.make_diag(path, idx, rule, None));
                }
            }
            Check::HasHeading => {
                let text = match parsed {
                    ParsedContent::Markdown(t) => t.as_str(),
                    _ => src,
                };
                if !text.lines().any(|l| l.starts_with('#')) {
                    return Some(self.make_diag(path, idx, rule, None));
                }
            }
            Check::HasCommandHeading => {
                let text = match parsed {
                    ParsedContent::Markdown(t) => t.as_str(),
                    _ => src,
                };
                // Only fire if there ARE headings but none are command-related.
                let has_heading = text.lines().any(|l| l.starts_with('#'));
                if !has_heading {
                    return None; // Let has-heading catch this instead.
                }
                const COMMAND_KEYWORDS: &[&str] =
                    &["build", "test", "run", "lint", "commands", "setup"];
                let has_command_heading = text.lines().filter(|l| l.starts_with('#')).any(|l| {
                    let lower = l.to_lowercase();
                    let words: Vec<&str> = lower
                        .split(|c: char| !c.is_alphanumeric())
                        .filter(|w| !w.is_empty())
                        .collect();
                    COMMAND_KEYWORDS.iter().any(|kw| words.contains(kw))
                });
                if !has_command_heading {
                    return Some(self.make_diag(path, idx, rule, None));
                }
            }
            Check::HasShebang => {
                let first_line = src.lines().next().unwrap_or("");
                if !first_line.starts_with("#!") {
                    return Some(self.make_diag(path, idx, rule, None));
                }
            }
            Check::MaxLines => {
                let max = rule.max.unwrap_or(usize::MAX);
                let count = src.lines().count();
                if count > max {
                    return Some(self.make_diag(path, idx, rule, Some(&format!("{count} lines"))));
                }
            }
            Check::MinContent => {
                let min_lines = rule.min_lines.unwrap_or(5);
                let min_chars = rule.min_chars.unwrap_or(100);
                let non_empty_lines = src.lines().filter(|l| !l.trim().is_empty()).count();
                let non_ws_chars = src.chars().filter(|c| !c.is_whitespace()).count();
                if non_empty_lines < min_lines || non_ws_chars < min_chars {
                    return Some(self.make_diag(path, idx, rule, None));
                }
            }
            Check::IsObject => {
                if let ParsedContent::JsonValue(v) = parsed
                    && !v.is_object()
                {
                    return Some(self.make_diag(path, idx, rule, None));
                }
            }
            Check::KnownKeys => {
                let values_ref = rule.values.as_ref()?;
                let known = self.resolve_values(values_ref);
                match parsed {
                    ParsedContent::JsonValue(serde_json::Value::Object(obj)) => {
                        for key in obj.keys() {
                            if !known.iter().any(|k| k == key) {
                                return Some(self.make_diag(
                                    path,
                                    idx,
                                    rule,
                                    Some(&format!("'{key}'")),
                                ));
                            }
                        }
                    }
                    ParsedContent::YamlValue(serde_yaml::Value::Mapping(map)) => {
                        for key in map.keys() {
                            if let Some(k) = key.as_str()
                                && !known.iter().any(|kn| kn == k)
                            {
                                return Some(self.make_diag(
                                    path,
                                    idx,
                                    rule,
                                    Some(&format!("'{k}'")),
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Check::FieldRequired => {
                let field = rule.field.as_deref()?;
                if !has_field_with_value(parsed, field) {
                    return Some(self.make_diag(path, idx, rule, None));
                }
            }
            Check::FieldExists => {
                let field = rule.field.as_deref()?;
                if !has_field(parsed, field) {
                    return Some(self.make_diag(path, idx, rule, None));
                }
            }
            Check::FieldOneOf => {
                let field = rule.field.as_deref()?;
                let values_ref = rule.values.as_ref()?;
                let allowed = self.resolve_values(values_ref);
                if let Some(val) = get_field_str(parsed, field)
                    && !allowed.iter().any(|a| a == &val)
                {
                    return Some(self.make_diag(path, idx, rule, Some(&format!("'{val}'"))));
                }
                // If field doesn't exist, FieldOneOf doesn't fire (use
                // FieldRequired separately for that).
            }
            Check::FieldNotOneOfCi => {
                let field = rule.field.as_deref()?;
                let values_ref = rule.values.as_ref()?;
                let forbidden = self.resolve_values(values_ref);
                if let Some(val) = get_field_str(parsed, field) {
                    let lower = val.to_lowercase();
                    if forbidden.iter().any(|f| f.to_lowercase() == lower) {
                        return Some(self.make_diag(path, idx, rule, Some(&format!("'{val}'"))));
                    }
                }
            }
            Check::FieldMinLength => {
                let field = rule.field.as_deref()?;
                let min = rule.min.unwrap_or(0);
                if let Some(val) = get_field_str(parsed, field)
                    && val.len() < min
                {
                    return Some(self.make_diag(
                        path,
                        idx,
                        rule,
                        Some(&format!("{} chars, minimum {min}", val.len())),
                    ));
                }
            }
            Check::FieldMaxLength => {
                let field = rule.field.as_deref()?;
                let max = rule.max.unwrap_or(usize::MAX);
                if let Some(val) = get_field_str(parsed, field)
                    && val.len() > max
                {
                    return Some(self.make_diag(
                        path,
                        idx,
                        rule,
                        Some(&format!("{} chars, maximum {max}", val.len())),
                    ));
                }
            }
            Check::ArrayNonEmpty => {
                let field = rule.field.as_deref()?;
                match parsed {
                    ParsedContent::YamlValue(v) => {
                        if let Some(seq) = v.get(field)
                            && seq.as_sequence().is_some_and(|s| s.is_empty())
                        {
                            return Some(self.make_diag(path, idx, rule, None));
                        }
                    }
                    ParsedContent::JsonValue(v) => {
                        if let Some(arr) = v.get(field)
                            && arr.as_array().is_some_and(|a| a.is_empty())
                        {
                            return Some(self.make_diag(path, idx, rule, None));
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }
}
