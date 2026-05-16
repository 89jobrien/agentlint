use agentlint_core::{Diagnostic, Difficulty};
use std::path::Path;

pub struct HooksValidator;

impl HooksValidator {
    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        // Must have a shebang on line 1.
        let first_line = src.lines().next().unwrap_or("");
        if !first_line.starts_with("#!") {
            diags.push(
                Diagnostic::error(
                    path,
                    1,
                    1,
                    "hook file must have a shebang line (#!) on line 1",
                )
                .with_rule("claude/hooks/missing-shebang", Difficulty::Easy),
            );
        }

        // Must have the execute bit set on Unix.
        if !has_execute_bit(path) {
            diags.push(
                Diagnostic::error(
                    path,
                    1,
                    1,
                    "hook file must have the execute bit set (chmod +x)",
                )
                .with_rule("claude/hooks/no-execute-bit", Difficulty::Easy),
            );
        }

        diags
    }
}

#[cfg(unix)]
fn has_execute_bit(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn has_execute_bit(_path: &Path) -> bool {
    // Windows has no execute bit — skip this check.
    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agentlint_core::testing::{assert_clean, assert_error_contains};
    use std::path::Path;

    #[test]
    fn missing_shebang_is_error() {
        let src = "echo hello\n";
        let diags = HooksValidator::validate(Path::new("/tmp/hook"), src);
        assert_error_contains(&diags, "shebang");
    }

    #[test]
    fn shebang_present_no_shebang_error() {
        let src = "#!/usr/bin/env nu\necho hello\n";
        // Only check the shebang diagnostic — execute-bit is path-dependent.
        let shebang_errors: Vec<_> = HooksValidator::validate(Path::new("/tmp/nonexistent"), src)
            .into_iter()
            .filter(|d| d.message.contains("shebang"))
            .collect();
        assert!(
            shebang_errors.is_empty(),
            "shebang present — no shebang error expected"
        );
    }

    #[cfg(unix)]
    #[test]
    fn executable_hook_with_shebang_is_clean() {
        use agentlint_core::testing::FixtureDir;
        use std::os::unix::fs::PermissionsExt;

        let fixture = FixtureDir::new();
        let path = fixture.write("my-hook", "#!/usr/bin/env nu\necho hello\n");
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();

        let src = std::fs::read_to_string(&path).unwrap();
        assert_clean(&HooksValidator::validate(&path, &src));
    }
}
