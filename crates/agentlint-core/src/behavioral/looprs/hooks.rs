use crate::{Diagnostic, Difficulty};
use std::path::Path;

use super::KNOWN_TRIGGERS;

#[derive(serde::Deserialize)]
struct HookDef {
    #[serde(default)]
    name: String,
    #[serde(default)]
    trigger: String,
    #[serde(default)]
    actions: Vec<serde_yaml::Value>,
}

pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    if src.trim().is_empty() {
        diags.push(
            Diagnostic::error(path, 1, 1, "hook file is empty")
                .with_rule("looprs/hooks/empty", Difficulty::Easy),
        );
        return diags;
    }

    let hook: HookDef = match serde_yaml::from_str(src) {
        Ok(h) => h,
        Err(e) => {
            diags.push(
                Diagnostic::error(path, 1, 1, format!("invalid YAML: {e}"))
                    .with_rule("looprs/hooks/invalid-yaml", Difficulty::Easy),
            );
            return diags;
        }
    };

    if hook.name.trim().is_empty() {
        diags.push(
            Diagnostic::error(path, 1, 1, "missing or empty 'name' field")
                .with_rule("looprs/hooks/missing-name", Difficulty::Easy),
        );
    }

    if hook.trigger.trim().is_empty() {
        diags.push(
            Diagnostic::error(path, 1, 1, "missing or empty 'trigger' field")
                .with_rule("looprs/hooks/missing-trigger", Difficulty::Easy),
        );
    } else if !KNOWN_TRIGGERS.contains(&hook.trigger.as_str()) {
        diags.push(
            Diagnostic::warning(
                path,
                1,
                1,
                format!(
                    "unknown trigger '{}' — not in known event list",
                    hook.trigger
                ),
            )
            .with_rule("looprs/hooks/unknown-trigger", Difficulty::Hard),
        );
    }

    if hook.actions.is_empty() {
        diags.push(
            Diagnostic::error(path, 1, 1, "actions list is empty")
                .with_rule("looprs/hooks/empty-actions", Difficulty::Easy),
        );
    }

    diags
}
