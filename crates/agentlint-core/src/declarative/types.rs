//! TOML schema types for declarative plugin definitions.

use std::collections::HashMap;

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
    /// Name of the behavioral function in the registry.
    #[serde(default)]
    pub custom_fn: Option<String>,
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
    Custom,
}

/// Values can be inline strings or a `$constant_name` reference.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub enum ValuesOrRef {
    Inline(Vec<String>),
    Ref(String),
}
