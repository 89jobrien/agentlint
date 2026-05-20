//! Declarative plugin engine: load validator definitions from TOML files.

mod eval;
pub(crate) mod parse;
pub mod types;

#[cfg(test)]
mod tests;

use crate::Validator;
use std::path::Path;

pub use eval::DeclarativeValidator;
pub use types::{
    Check, DifficultyDef, Format, PluginFile, PluginMeta, RuleDef, SeverityDef, ValidatorDef,
    ValuesOrRef,
};

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
