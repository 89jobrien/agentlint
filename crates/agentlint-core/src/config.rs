//! Config file loading for `.agentlint.toml`.
//!
//! Enabled via the `config` Cargo feature so downstream library users can opt
//! out of the `toml`/`serde` dependency if they build their own config loading.

use std::collections::HashMap;
use std::path::Path;

use crate::{Difficulty, IgnoreEntry, RuleOverride, RunConfig};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error returned when a config file exists but cannot be parsed.
#[derive(Debug)]
pub struct ConfigError(pub String);

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Raw TOML structs (internal)
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize, Default)]
struct RawConfig {
    agentlint: Option<RawAgentlintSection>,
    rules: Option<HashMap<String, RuleOverride>>,
    #[serde(rename = "ignore", default)]
    ignores: Vec<RawIgnoreEntry>,
}

#[derive(serde::Deserialize, Default)]
struct RawAgentlintSection {
    difficulty: Option<String>,
}

#[derive(serde::Deserialize)]
struct RawIgnoreEntry {
    path: String,
    #[serde(default)]
    rules: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load `.agentlint.toml` from `path`.
///
/// - Returns `Ok(None)` when the file does not exist.
/// - Returns `Ok(Some(RunConfig))` on success.
/// - Returns `Err(ConfigError)` when the file exists but is malformed.
pub fn load_config(path: &Path) -> Result<Option<RunConfig>, ConfigError> {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(ConfigError(format!(
                "could not read {}: {e}",
                path.display()
            )));
        }
    };

    let raw: RawConfig = toml::from_str(&src)
        .map_err(|e| ConfigError(format!("invalid config {}: {e}", path.display())))?;

    let difficulty = match raw.agentlint.as_ref().and_then(|s| s.difficulty.as_deref()) {
        None => Difficulty::Hard,
        Some(s) => s.parse::<Difficulty>().map_err(ConfigError)?,
    };

    let rule_overrides = raw.rules.unwrap_or_default();

    let ignores = raw
        .ignores
        .into_iter()
        .map(|e| IgnoreEntry {
            path: e.path,
            rules: e.rules,
        })
        .collect();

    Ok(Some(RunConfig {
        difficulty,
        rule_overrides,
        ignores,
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_toml(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{content}").unwrap();
        f
    }

    #[test]
    fn missing_file_returns_none() {
        let result = load_config(Path::new("/tmp/does-not-exist-agentlint.toml"));
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn empty_toml_returns_default_config() {
        let f = write_toml("");
        let cfg = load_config(f.path()).unwrap().unwrap();
        assert_eq!(cfg.difficulty, Difficulty::Hard);
        assert!(cfg.rule_overrides.is_empty());
        assert!(cfg.ignores.is_empty());
    }

    #[test]
    fn difficulty_field_parsed() {
        let f = write_toml("[agentlint]\ndifficulty = \"easy\"\n");
        let cfg = load_config(f.path()).unwrap().unwrap();
        assert_eq!(cfg.difficulty, Difficulty::Easy);
    }

    #[test]
    fn painful_difficulty_parsed() {
        let f = write_toml("[agentlint]\ndifficulty = \"painful\"\n");
        let cfg = load_config(f.path()).unwrap().unwrap();
        assert_eq!(cfg.difficulty, Difficulty::Painful);
    }

    #[test]
    fn rules_section_parsed() {
        let f = write_toml(
            "[rules]\n\
             \"claude/settings/broad-read\" = \"off\"\n\
             \"claude/hooks/naive-str-contains\" = \"error\"\n",
        );
        let cfg = load_config(f.path()).unwrap().unwrap();
        assert!(matches!(
            cfg.rule_overrides.get("claude/settings/broad-read"),
            Some(RuleOverride::Off)
        ));
        assert!(matches!(
            cfg.rule_overrides.get("claude/hooks/naive-str-contains"),
            Some(RuleOverride::Error)
        ));
    }

    #[test]
    fn ignore_section_parsed() {
        let f = write_toml(
            "[[ignore]]\n\
             path = \".claude/settings.local.json\"\n\
             rules = [\"claude/settings/broad-read\"]\n",
        );
        let cfg = load_config(f.path()).unwrap().unwrap();
        assert_eq!(cfg.ignores.len(), 1);
        assert_eq!(cfg.ignores[0].path, ".claude/settings.local.json");
        assert_eq!(cfg.ignores[0].rules, ["claude/settings/broad-read"]);
    }

    #[test]
    fn ignore_without_rules_defaults_to_empty_vec() {
        let f = write_toml("[[ignore]]\npath = \"settings.json\"\n");
        let cfg = load_config(f.path()).unwrap().unwrap();
        assert_eq!(cfg.ignores.len(), 1);
        assert!(cfg.ignores[0].rules.is_empty());
    }

    #[test]
    fn invalid_toml_returns_err() {
        let f = write_toml("not = [valid toml\n");
        assert!(matches!(load_config(f.path()), Err(_)));
    }

    #[test]
    fn unknown_difficulty_returns_err() {
        let f = write_toml("[agentlint]\ndifficulty = \"medium\"\n");
        assert!(matches!(load_config(f.path()), Err(_)));
    }

    #[test]
    fn full_config_roundtrip() {
        let f = write_toml(
            "[agentlint]\n\
             difficulty = \"painful\"\n\
             \n\
             [rules]\n\
             \"claude/settings/broad-read\" = \"warning\"\n\
             \n\
             [[ignore]]\n\
             path = \".claude/settings.local.json\"\n\
             rules = [\"claude/settings/broad-read\"]\n\
             \n\
             [[ignore]]\n\
             path = \"settings.json\"\n",
        );
        let cfg = load_config(f.path()).unwrap().unwrap();
        assert_eq!(cfg.difficulty, Difficulty::Painful);
        assert!(matches!(
            cfg.rule_overrides.get("claude/settings/broad-read"),
            Some(RuleOverride::Warning)
        ));
        assert_eq!(cfg.ignores.len(), 2);
    }
}
