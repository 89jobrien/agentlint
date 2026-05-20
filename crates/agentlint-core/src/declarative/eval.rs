//! Rule evaluation logic for DeclarativeValidator.

use crate::Diagnostic;
use std::path::Path;

use super::parse::{ParsedContent, get_field_str, has_field, has_field_with_value};
use super::types::{Check, RuleDef};
use super::validator::DeclarativeValidator;

impl DeclarativeValidator {
    pub(super) fn eval_rule(
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
