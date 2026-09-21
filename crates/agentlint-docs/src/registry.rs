//! Registry and discovery for documentation schema providers.

use crate::DocsSchema;
use crate::plugin::{BUILTIN_SRC, BuiltinJsonPlugin, DocsSchemaPlugin, FileJsonPlugin};
use std::path::Path;

pub struct SchemaRegistry {
    plugins: Vec<Box<dyn DocsSchemaPlugin>>,
}

impl SchemaRegistry {
    /// Built-in schemas only; no filesystem access.
    pub fn builtin() -> Self {
        let mut r = Self { plugins: vec![] };
        r.register(Box::new(BuiltinJsonPlugin {
            name: "agentlint-default",
            src: BUILTIN_SRC,
        }));
        r
    }

    /// Built-ins + JSON files discovered from `{root}/.agentlint/schemas/`.
    pub fn discover(root: &Path) -> Self {
        let mut r = Self::builtin();
        let schema_dir = root.join(".agentlint").join("schemas");
        let Ok(entries) = std::fs::read_dir(&schema_dir) else {
            return r;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                r.register(Box::new(FileJsonPlugin::new(path)));
            }
        }
        r
    }

    /// Adds a schema provider to the registry.
    pub fn register(&mut self, plugin: Box<dyn DocsSchemaPlugin>) {
        self.plugins.push(plugin);
    }

    /// Returns `Some(Result<DocsSchema>)` if a plugin with `name` exists.
    pub fn get(&self, name: &str) -> Option<Result<DocsSchema, String>> {
        self.plugins
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.load())
    }

    /// Returns provider names in registration order.
    pub fn names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

// Tests

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn builtin_registry_contains_agentlint_default() {
        let reg = SchemaRegistry::builtin();
        assert!(reg.get("agentlint-default").is_some());
    }

    #[test]
    fn discover_finds_json_files_in_schema_dir() {
        let dir = tempdir().unwrap();
        let schema_dir = dir.path().join(".agentlint").join("schemas");
        std::fs::create_dir_all(&schema_dir).unwrap();
        std::fs::write(
            schema_dir.join("my-schema.json"),
            r#"{"doctypes": ["note"]}"#,
        )
        .unwrap();
        let reg = SchemaRegistry::discover(dir.path());
        assert!(reg.get("my-schema").is_some());
        let schema = reg.get("my-schema").unwrap().unwrap();
        assert_eq!(schema.doctypes, vec!["note"]);
    }

    #[test]
    fn names_includes_builtins() {
        let reg = SchemaRegistry::builtin();
        assert!(reg.names().contains(&"agentlint-default"));
    }

    #[test]
    fn get_unknown_name_returns_none() {
        let reg = SchemaRegistry::builtin();
        assert!(reg.get("nonexistent").is_none());
    }
}
