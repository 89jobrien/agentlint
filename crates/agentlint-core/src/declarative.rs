//! Declarative plugin engine: load validator definitions from TOML files.

use crate::{Diagnostic, Difficulty, Severity, Validator};
use std::collections::HashMap;
use std::path::Path;

// -------------------------------------------------------------------------
// TOML schema types
// -------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
pub struct PluginFile {
    pub plugin: PluginMeta,
    #[serde(default)]
    pub validator: Vec<ValidatorDef>,
}

#[derive(Debug, serde::Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub prefix: String,
    #[serde(default)]
    pub constants: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ValidatorDef {
    pub id: String,
    pub patterns: Vec<String>,
    pub format: Format,
    #[serde(default)]
    pub skip_filenames: Vec<String>,
    /// When true, frontmatter is optional — files without `---` are silently
    /// skipped rather than producing parse errors.
    #[serde(default)]
    pub frontmatter_optional: bool,
    #[serde(default)]
    pub rules: Vec<RuleDef>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Yaml,
    Json,
    Frontmatter,
    Markdown,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RuleDef {
    pub id: String,
    pub check: Check,
    #[serde(default = "default_severity")]
    pub severity: SeverityDef,
    #[serde(default = "default_difficulty")]
    pub difficulty: DifficultyDef,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub values: Option<ValuesOrRef>,
    #[serde(default)]
    pub min: Option<usize>,
    #[serde(default)]
    pub max: Option<usize>,
    /// Minimum non-empty lines for `min-content` check.
    #[serde(default)]
    pub min_lines: Option<usize>,
    /// Minimum non-whitespace characters for `min-content` check.
    #[serde(default)]
    pub min_chars: Option<usize>,
}

fn default_severity() -> SeverityDef {
    SeverityDef::Error
}
fn default_difficulty() -> DifficultyDef {
    DifficultyDef::Easy
}

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SeverityDef {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DifficultyDef {
    Easy,
    Normal,
    Hard,
    Painful,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Check {
    NonEmpty,
    ValidParse,
    HasFrontmatter,
    FrontmatterClosed,
    HasHeading,
    HasCommandHeading,
    HasShebang,
    MaxLines,
    MinContent,
    FieldRequired,
    FieldExists,
    FieldOneOf,
    FieldNotOneOfCi,
    FieldMinLength,
    FieldMaxLength,
    ArrayNonEmpty,
    IsObject,
    KnownKeys,
}

/// Values can be inline strings or a `$constant_name` reference.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub enum ValuesOrRef {
    Inline(Vec<String>),
    Ref(String),
}

// -------------------------------------------------------------------------
// Loading
// -------------------------------------------------------------------------

/// Load a plugin definition from a TOML file.
pub fn load_plugin_file(path: &Path) -> Result<PluginFile, String> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    load_plugin_str(&src)
}

/// Parse a plugin definition from a TOML string.
pub fn load_plugin_str(src: &str) -> Result<PluginFile, String> {
    toml::from_str(src).map_err(|e| format!("invalid plugin TOML: {e}"))
}

// -------------------------------------------------------------------------
// Runtime validator
// -------------------------------------------------------------------------

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

// -------------------------------------------------------------------------
// Parsed content abstraction
// -------------------------------------------------------------------------

enum ParsedContent {
    YamlValue(serde_yaml::Value),
    JsonValue(serde_json::Value),
    Fields(Vec<(String, String)>), // frontmatter key-value pairs
    Markdown(String),
    Error(String),
}

fn parse_yaml(src: &str) -> ParsedContent {
    if src.trim().is_empty() {
        return ParsedContent::YamlValue(serde_yaml::Value::Null);
    }
    match serde_yaml::from_str(src) {
        Ok(v) => ParsedContent::YamlValue(v),
        Err(e) => ParsedContent::Error(format!("invalid YAML: {e}")),
    }
}

fn parse_json(src: &str) -> ParsedContent {
    if src.trim().is_empty() {
        return ParsedContent::JsonValue(serde_json::Value::Null);
    }
    match serde_json::from_str(src) {
        Ok(v) => ParsedContent::JsonValue(v),
        Err(e) => ParsedContent::Error(format!("invalid JSON: {e}")),
    }
}

fn parse_frontmatter(src: &str) -> ParsedContent {
    // Simple frontmatter parser: --- delimited YAML block.
    let trimmed = src.trim_start();
    if !trimmed.starts_with("---") {
        return ParsedContent::Error("no frontmatter".into());
    }
    let after_open = &trimmed[3..];
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
    if let Some(end) = after_open.find("\n---") {
        let fm_block = &after_open[..end];
        let mut fields = Vec::new();
        for line in fm_block.lines() {
            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim().to_string();
                let val = val.trim().to_string();
                if !key.is_empty() {
                    fields.push((key, val));
                }
            }
        }
        ParsedContent::Fields(fields)
    } else {
        ParsedContent::Error("unclosed frontmatter".into())
    }
}

// -------------------------------------------------------------------------
// Field access helpers
// -------------------------------------------------------------------------

/// Check if a field exists (any value, including null/empty).
fn has_field(parsed: &ParsedContent, field: &str) -> bool {
    match parsed {
        ParsedContent::YamlValue(v) => resolve_yaml_path(v, field).is_some(),
        ParsedContent::JsonValue(v) => resolve_json_path(v, field).is_some(),
        ParsedContent::Fields(fields) => fields.iter().any(|(k, _)| k == field),
        _ => false,
    }
}

/// Check if a field exists AND has a non-empty string value.
fn has_field_with_value(parsed: &ParsedContent, field: &str) -> bool {
    match parsed {
        ParsedContent::YamlValue(v) => resolve_yaml_path(v, field)
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty()),
        ParsedContent::JsonValue(v) => resolve_json_path(v, field)
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty()),
        ParsedContent::Fields(fields) => fields
            .iter()
            .any(|(k, val)| k == field && !val.trim().is_empty()),
        _ => false,
    }
}

/// Get the string value of a field (for one-of checks).
fn get_field_str(parsed: &ParsedContent, field: &str) -> Option<String> {
    match parsed {
        ParsedContent::YamlValue(v) => {
            resolve_yaml_path(v, field).and_then(|v| v.as_str().map(String::from))
        }
        ParsedContent::JsonValue(v) => {
            resolve_json_path(v, field).and_then(|v| v.as_str().map(String::from))
        }
        ParsedContent::Fields(fields) => fields
            .iter()
            .find(|(k, _)| k == field)
            .map(|(_, v)| v.clone()),
        _ => None,
    }
}

/// Resolve dotted path like `action.type` in a YAML value.
fn resolve_yaml_path<'a>(v: &'a serde_yaml::Value, path: &str) -> Option<&'a serde_yaml::Value> {
    let mut current = v;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

/// Resolve dotted path like `action.type` in a JSON value.
fn resolve_json_path<'a>(v: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut current = v;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

// -------------------------------------------------------------------------
// Convenience: load all validators from a plugin file
// -------------------------------------------------------------------------

/// Load a plugin TOML and return a vec of boxed Validators.
pub fn load_plugin_validators(path: &Path) -> Result<Vec<Box<dyn Validator>>, String> {
    let plugin = load_plugin_file(path)?;
    Ok(validators_from_plugin(plugin))
}

/// Convert a parsed PluginFile into boxed Validators.
pub fn validators_from_plugin(plugin: PluginFile) -> Vec<Box<dyn Validator>> {
    plugin
        .validator
        .into_iter()
        .map(|def| {
            Box::new(DeclarativeValidator::new(def, &plugin.plugin.constants)) as Box<dyn Validator>
        })
        .collect()
}

/// Parse a plugin TOML string and return boxed Validators.
pub fn validators_from_str(src: &str) -> Result<Vec<Box<dyn Validator>>, String> {
    let plugin = load_plugin_str(src)?;
    Ok(validators_from_plugin(plugin))
}

// -------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const LOOPRS_TOML: &str = include_str!("../../../plugins/agentlint.looprs.toml");

    #[test]
    fn looprs_toml_parses() {
        let plugin = load_plugin_str(LOOPRS_TOML).unwrap();
        assert_eq!(plugin.plugin.name, "looprs");
        assert_eq!(plugin.plugin.prefix, "looprs");
        assert!(!plugin.validator.is_empty());
    }

    #[test]
    fn looprs_creates_validators() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        assert!(validators.len() >= 7);
    }

    // --- Commands ---

    #[test]
    fn commands_empty_errors() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("commands")))
            .unwrap();
        let diags = v.validate(Path::new(".looprs/commands/x.yaml"), "");
        assert!(diags.iter().any(|d| d.rule.contains("empty")));
    }

    #[test]
    fn commands_missing_name_errors() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("commands")))
            .unwrap();
        let src = "description: Run linter\naction:\n  type: shell\n  command: cargo clippy\n";
        let diags = v.validate(Path::new(".looprs/commands/lint.yaml"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-name")));
    }

    #[test]
    fn commands_valid_is_clean() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("commands")))
            .unwrap();
        let src = "name: lint\ndescription: Run linter\naction:\n  type: shell\n  command: cargo clippy\n";
        let diags = v.validate(Path::new(".looprs/commands/lint.yaml"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn commands_invalid_action_type_errors() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("commands")))
            .unwrap();
        let src = "name: x\ndescription: y\naction:\n  type: unknown\n";
        let diags = v.validate(Path::new(".looprs/commands/x.yaml"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("invalid-action-type")),
            "expected invalid-action-type, got: {diags:?}"
        );
    }

    // --- Hooks ---

    #[test]
    fn hooks_valid_is_clean() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("hooks")))
            .unwrap();
        let src = "name: start\ntrigger: SessionStart\nactions:\n  - type: message\n    text: hi\n";
        let diags = v.validate(Path::new(".looprs/hooks/start.yaml"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn hooks_unknown_trigger_warns() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("hooks")))
            .unwrap();
        let src = "name: start\ntrigger: FakeEvent\nactions:\n  - type: message\n    text: hi\n";
        let diags = v.validate(Path::new(".looprs/hooks/start.yaml"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("unknown-trigger")),
            "expected unknown-trigger, got: {diags:?}"
        );
    }

    #[test]
    fn hooks_empty_actions_errors() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("hooks")))
            .unwrap();
        let src = "name: start\ntrigger: SessionStart\nactions: []\n";
        let diags = v.validate(Path::new(".looprs/hooks/start.yaml"), src);
        assert!(diags.iter().any(|d| d.rule.contains("empty-actions")));
    }

    // --- Skills ---

    #[test]
    fn skills_valid_is_clean() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("skills")))
            .unwrap();
        let src = "---\nname: test-skill\ntriggers:\n  - test\n---\n# Content\n";
        let diags = v.validate(Path::new(".looprs/skills/test/SKILL.md"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn skills_missing_frontmatter_errors() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("skills")))
            .unwrap();
        let src = "# Just content\nNo frontmatter.\n";
        let diags = v.validate(Path::new(".looprs/skills/test/SKILL.md"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-frontmatter")));
    }

    // --- Rules (markdown) ---

    #[test]
    fn rules_valid_is_clean() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("rules")))
            .unwrap();
        let src = "# Security\n\nDon't commit secrets.\n";
        let diags = v.validate(Path::new(".looprs/rules/security.md"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn rules_readme_skipped() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("rules")))
            .unwrap();
        let diags = v.validate(Path::new(".looprs/rules/README.md"), "");
        assert!(diags.is_empty());
    }

    // --- Agent JSON ---

    #[test]
    fn agent_json_valid_is_clean() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("agent.json")))
            .unwrap();
        let src = r#"{"version": "1.0", "agent_schema": {}}"#;
        let diags = v.validate(Path::new("looprs.agent.json"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn agent_json_missing_version_warns() {
        let validators = validators_from_str(LOOPRS_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("agent.json")))
            .unwrap();
        let src = r#"{"agent_schema": {}}"#;
        let diags = v.validate(Path::new("looprs.agent.json"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-version")));
    }

    // =======================================================================
    // Claude plugin tests
    // =======================================================================

    const CLAUDE_TOML: &str = include_str!("../../../plugins/agentlint.claude.toml");

    #[test]
    fn claude_toml_parses() {
        let plugin = load_plugin_str(CLAUDE_TOML).unwrap();
        assert_eq!(plugin.plugin.name, "claude");
        assert_eq!(plugin.plugin.prefix, "claude");
        assert!(plugin.validator.len() >= 7);
    }

    #[test]
    fn claude_agents_valid_is_clean() {
        let validators = validators_from_str(CLAUDE_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("agents")))
            .unwrap();
        let src = "---\nname: my-agent\ndescription: does things well enough here\n---\nbody\n";
        let diags = v.validate(Path::new(".claude/agents/test.md"), src);
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn claude_agents_missing_name_errors() {
        let validators = validators_from_str(CLAUDE_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("agents")))
            .unwrap();
        let src = "---\ndescription: does things well enough here\n---\n";
        let diags = v.validate(Path::new(".claude/agents/test.md"), src);
        assert!(diags.iter().any(|d| d.rule.contains("missing-name")));
    }

    #[test]
    fn claude_agents_name_collision_errors() {
        let validators = validators_from_str(CLAUDE_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("agents")))
            .unwrap();
        let src = "---\nname: Bash\ndescription: does things well enough here\n---\n";
        let diags = v.validate(Path::new(".claude/agents/test.md"), src);
        assert!(
            diags.iter().any(|d| d.rule.contains("name-collision")),
            "expected name-collision, got: {diags:?}"
        );
    }

    #[test]
    fn claude_agents_description_too_short_warns() {
        let validators = validators_from_str(CLAUDE_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("agents")))
            .unwrap();
        let src = "---\nname: my-agent\ndescription: short\n---\n";
        let diags = v.validate(Path::new(".claude/agents/test.md"), src);
        assert!(
            diags
                .iter()
                .any(|d| d.rule.contains("description-too-short")),
            "expected description-too-short, got: {diags:?}"
        );
    }

    #[test]
    fn claude_hooks_shebang_check() {
        let validators = validators_from_str(CLAUDE_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("hooks")))
            .unwrap();
        let diags = v.validate(Path::new(".claude/hooks/my-hook"), "echo hello\n");
        assert!(diags.iter().any(|d| d.rule.contains("missing-shebang")));
    }

    #[test]
    fn claude_hooks_shebang_present_is_clean() {
        let validators = validators_from_str(CLAUDE_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("hooks")))
            .unwrap();
        let diags = v.validate(
            Path::new(".claude/hooks/my-hook"),
            "#!/usr/bin/env nu\necho hi\n",
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn claude_settings_unknown_key_errors() {
        let validators = validators_from_str(CLAUDE_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("settings")))
            .unwrap();
        let diags = v.validate(Path::new(".claude/settings.json"), r#"{"theme": "dark"}"#);
        assert!(
            diags.iter().any(|d| d.rule.contains("unknown-key")),
            "expected unknown-key, got: {diags:?}"
        );
    }

    #[test]
    fn claude_settings_valid_is_clean() {
        let validators = validators_from_str(CLAUDE_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("settings")))
            .unwrap();
        let diags = v.validate(
            Path::new(".claude/settings.json"),
            r#"{"permissions": {}, "model": "sonnet"}"#,
        );
        assert!(diags.is_empty(), "unexpected: {diags:?}");
    }

    #[test]
    fn claude_meta_no_heading_warns() {
        let validators = validators_from_str(CLAUDE_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| *p == "CLAUDE.md"))
            .unwrap();
        let diags = v.validate(Path::new("CLAUDE.md"), "Just text, no heading.\n");
        assert!(
            diags
                .iter()
                .any(|d| d.rule.contains("claude-md-no-heading")),
            "expected no-heading, got: {diags:?}"
        );
    }

    #[test]
    fn claude_meta_too_long_warns() {
        let validators = validators_from_str(CLAUDE_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| *p == "CLAUDE.md"))
            .unwrap();
        let src = "# Title\n".to_string() + &"line\n".repeat(501);
        let diags = v.validate(Path::new("CLAUDE.md"), &src);
        assert!(
            diags.iter().any(|d| d.rule.contains("claude-md-too-long")),
            "expected too-long, got: {diags:?}"
        );
    }

    #[test]
    fn claude_skills_name_too_long_errors() {
        let validators = validators_from_str(CLAUDE_TOML).unwrap();
        let v = validators
            .iter()
            .find(|v| v.patterns().iter().any(|p| p.contains("skills")))
            .unwrap();
        let long = "a".repeat(65);
        let src = format!("---\nname: {long}\ndescription: ok\n---\n");
        let diags = v.validate(Path::new(".claude/skills/test/SKILL.md"), &src);
        assert!(
            diags.iter().any(|d| d.rule.contains("name-too-long")),
            "expected name-too-long, got: {diags:?}"
        );
    }
}
