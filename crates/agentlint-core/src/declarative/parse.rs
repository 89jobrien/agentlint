//! Parsed content abstraction and field access helpers.
//!
// TODO(testing/fuzz): add fuzz target for parse_frontmatter — it handles
// untrusted string input with delimiter search and line splitting.
// TODO(testing/property): add proptest for resolve_yaml_path and
// resolve_json_path — dotted path resolution over nested structures.

pub(crate) enum ParsedContent {
    YamlValue(serde_yaml::Value),
    JsonValue(serde_json::Value),
    Fields(Vec<(String, String)>), // frontmatter key-value pairs
    Markdown(String),
    Error(String),
}

pub(crate) fn parse_yaml(src: &str) -> ParsedContent {
    if src.trim().is_empty() {
        return ParsedContent::YamlValue(serde_yaml::Value::Null);
    }
    match serde_yaml::from_str(src) {
        Ok(v) => ParsedContent::YamlValue(v),
        Err(e) => ParsedContent::Error(format!("invalid YAML: {e}")),
    }
}

pub(crate) fn parse_json(src: &str) -> ParsedContent {
    if src.trim().is_empty() {
        return ParsedContent::JsonValue(serde_json::Value::Null);
    }
    match serde_json::from_str(src) {
        Ok(v) => ParsedContent::JsonValue(v),
        Err(e) => ParsedContent::Error(format!("invalid JSON: {e}")),
    }
}

pub(crate) fn parse_frontmatter(src: &str) -> ParsedContent {
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
pub(crate) fn has_field(parsed: &ParsedContent, field: &str) -> bool {
    match parsed {
        ParsedContent::YamlValue(v) => resolve_yaml_path(v, field).is_some(),
        ParsedContent::JsonValue(v) => resolve_json_path(v, field).is_some(),
        ParsedContent::Fields(fields) => fields.iter().any(|(k, _)| k == field),
        _ => false,
    }
}

/// Check if a field exists AND has a non-empty string value.
pub(crate) fn has_field_with_value(parsed: &ParsedContent, field: &str) -> bool {
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
pub(crate) fn get_field_str(parsed: &ParsedContent, field: &str) -> Option<String> {
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
