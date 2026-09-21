//! Schema providers for built-in and filesystem-backed documentation schemas.

use crate::DocsSchema;
use std::path::PathBuf;

pub(crate) const BUILTIN_SRC: &str = include_str!("../schemas/agentlint-default.json");

/// A named schema provider that resolves to a [`DocsSchema`].
pub trait DocsSchemaPlugin: Send + Sync {
    /// Returns the schema provider's registry name.
    fn name(&self) -> &str;
    /// Loads and parses the provider's schema.
    fn load(&self) -> Result<DocsSchema, String>;
}

/// A built-in schema compiled into the binary.
pub struct BuiltinJsonPlugin {
    pub name: &'static str,
    pub src: &'static str,
}

impl DocsSchemaPlugin for BuiltinJsonPlugin {
    fn name(&self) -> &str {
        self.name
    }

    fn load(&self) -> Result<DocsSchema, String> {
        serde_json::from_str(self.src)
            .map_err(|e| format!("built-in schema '{}' is invalid: {e}", self.name))
    }
}

/// A schema loaded from a JSON file at runtime.
pub struct FileJsonPlugin {
    name: String,
    path: PathBuf,
}

impl FileJsonPlugin {
    /// Creates a file-backed plugin named after the JSON file stem.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        Self { name, path }
    }
}

impl DocsSchemaPlugin for FileJsonPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn load(&self) -> Result<DocsSchema, String> {
        let src = std::fs::read_to_string(&self.path)
            .map_err(|e| format!("could not read schema {}: {e}", self.path.display()))?;
        serde_json::from_str(&src)
            .map_err(|e| format!("invalid schema {}: {e}", self.path.display()))
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_default_plugin_loads() {
        let p = BuiltinJsonPlugin {
            name: "agentlint-default",
            src: BUILTIN_SRC,
        };
        let schema = p.load().expect("load");
        assert!(!schema.doctypes.is_empty());
        assert_eq!(p.name(), "agentlint-default");
    }

    #[test]
    fn file_plugin_name_is_stem() {
        let p = FileJsonPlugin::new("/tmp/my-team-docs.json");
        assert_eq!(p.name(), "my-team-docs");
    }
}
