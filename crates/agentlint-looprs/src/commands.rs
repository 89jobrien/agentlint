use agentlint_core::{Diagnostic, Difficulty};
use std::path::Path;

#[derive(serde::Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum CommandAction {
    Prompt {},
    Shell {},
    Message {},
}

#[derive(serde::Deserialize)]
struct CommandDef {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    action: Option<CommandAction>,
}

pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    if src.trim().is_empty() {
        diags.push(
            Diagnostic::error(path, 1, 1, "command file is empty")
                .with_rule("looprs/commands/empty", Difficulty::Easy),
        );
        return diags;
    }

    let cmd: CommandDef = match serde_yaml::from_str(src) {
        Ok(c) => c,
        Err(e) => {
            diags.push(
                Diagnostic::error(path, 1, 1, format!("invalid YAML: {e}"))
                    .with_rule("looprs/commands/invalid-yaml", Difficulty::Easy),
            );
            return diags;
        }
    };

    if cmd.name.trim().is_empty() {
        diags.push(
            Diagnostic::error(path, 1, 1, "missing or empty 'name' field")
                .with_rule("looprs/commands/missing-name", Difficulty::Easy),
        );
    }

    if cmd.description.trim().is_empty() {
        diags.push(
            Diagnostic::warning(path, 1, 1, "missing or empty 'description' field")
                .with_rule("looprs/commands/missing-description", Difficulty::Hard),
        );
    }

    if cmd.action.is_none() {
        diags.push(
            Diagnostic::error(path, 1, 1, "missing 'action' field")
                .with_rule("looprs/commands/missing-action", Difficulty::Easy),
        );
    }

    diags
}
