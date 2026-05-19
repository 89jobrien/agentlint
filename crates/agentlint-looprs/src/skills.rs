use agentlint_core::{Diagnostic, Difficulty};
use agentlint_frontmatter::parse;
use std::path::Path;

pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    if src.trim().is_empty() {
        diags.push(
            Diagnostic::error(path, 1, 1, "SKILL.md is empty")
                .with_rule("looprs/skills/empty", Difficulty::Easy),
        );
        return diags;
    }

    let fields = match parse(src) {
        Ok(f) => f,
        Err(_) => {
            diags.push(
                Diagnostic::error(path, 1, 1, "missing or invalid frontmatter")
                    .with_rule("looprs/skills/missing-frontmatter", Difficulty::Easy),
            );
            return diags;
        }
    };

    if fields.is_empty() {
        diags.push(
            Diagnostic::error(path, 1, 1, "missing or invalid frontmatter")
                .with_rule("looprs/skills/missing-frontmatter", Difficulty::Easy),
        );
        return diags;
    }

    let has_name = fields
        .iter()
        .any(|f| f.key == "name" && !f.value.trim().is_empty());
    if !has_name {
        diags.push(
            Diagnostic::error(path, 1, 1, "missing or empty 'name' in frontmatter")
                .with_rule("looprs/skills/missing-name", Difficulty::Easy),
        );
    }

    let has_triggers = fields.iter().any(|f| f.key == "triggers");
    if !has_triggers {
        diags.push(
            Diagnostic::error(path, 1, 1, "missing 'triggers' in frontmatter")
                .with_rule("looprs/skills/missing-triggers", Difficulty::Easy),
        );
    }

    diags
}
