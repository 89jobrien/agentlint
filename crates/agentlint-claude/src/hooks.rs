use agentlint_core::{Diagnostic, Difficulty};
use std::path::Path;

pub struct HooksValidator;

/// Extensions that identify directly-executable hook scripts.
/// Files with no extension are also validated (bare executables).
/// Anything else in a hooks/ dir is skipped (config, docs, compiled source, etc.).
const SCRIPT_EXTENSIONS: &[&str] = &[
    "sh", "bash", "zsh", "fish", "nu", "py", "rb", "pl", "ts", "js",
];

impl HooksValidator {
    pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
        // Only validate known script types or bare files (no extension).
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) if !SCRIPT_EXTENSIONS.contains(&ext) => return vec![],
            _ => {}
        }

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

        check_naive_str_match(path, src, &mut diags);

        diags
    }
}

// ---------------------------------------------------------------------------
// Naive string-match detector
// ---------------------------------------------------------------------------

/// Patterns that indicate the hook is reading from stdin.
const STDIN_PATTERNS: &[&str] = &[
    "/dev/stdin",
    "$stdin",
    "$(cat)",
    "read -r",
    "stdin.read",
    "sys.stdin",
    "STDIN.read",
    "$STDIN",
];

/// Patterns that indicate proper JSON parsing of the hook input.
const JSON_PARSE_PATTERNS: &[&str] = &[
    "from json",
    "| jq",
    "jq -r",
    "jq '",
    "jq \"",
    "JSON.parse",
    "json.loads",
    "json.load(",
    "serde_json::from",
    "JSON.parse(",
];

/// Patterns that indicate naive string matching on the raw input.
/// Each entry is `(pattern, label)` used to annotate the diagnostic message.
const STR_MATCH_PATTERNS: &[(&str, &str)] = &[
    ("str contains", "`str contains`"),
    ("=~ ", "`=~`"),
    ("grep -q", "`grep -q`"),
    ("grep -E", "`grep -E`"),
    ("grep -F", "`grep -F`"),
    ("[[ \"$", "`[[ ... ]]`"),
    ("case \"$", "`case`"),
    ("case $", "`case`"),
    (".contains(", "`.contains()`"),
    ("in_str(", "`in_str()`"),
];

fn reads_stdin(src: &str) -> bool {
    STDIN_PATTERNS.iter().any(|p| src.contains(p))
}

fn parses_json(src: &str) -> bool {
    JSON_PARSE_PATTERNS.iter().any(|p| src.contains(p))
}

/// Returns the 1-based line number and label of the first string-match pattern found.
fn first_str_match(src: &str) -> Option<(usize, &'static str)> {
    for (lineno, line) in src.lines().enumerate() {
        // Skip comment lines.
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        for (pat, label) in STR_MATCH_PATTERNS {
            if line.contains(pat) {
                return Some((lineno + 1, label));
            }
        }
    }
    None
}

fn check_naive_str_match(path: &Path, src: &str, diags: &mut Vec<Diagnostic>) {
    if !reads_stdin(src) {
        return;
    }
    if parses_json(src) {
        return;
    }
    let Some((line, label)) = first_str_match(src) else {
        return;
    };
    diags.push(
        Diagnostic::warning(
            path,
            line,
            1,
            &format!(
                "hook matches on raw stdin with {} without parsing JSON first; \
                 use `from json` / `jq` to extract specific fields before matching",
                label
            ),
        )
        .with_rule("claude/hooks/naive-str-match", Difficulty::Normal),
    );
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

    // --- naive-str-match tests ---

    fn diags_for(src: &str) -> Vec<Diagnostic> {
        // Use a path with no extension so the extension guard doesn't skip it.
        HooksValidator::validate(Path::new("/tmp/my-hook"), src)
            .into_iter()
            .filter(|d| d.message.contains("raw stdin"))
            .collect()
    }

    #[test]
    fn naive_nu_no_from_json_warns() {
        let src = "#!/usr/bin/env nu\n\
                   let input = open --raw /dev/stdin\n\
                   if ($input | str contains \"Bash\") { exit 0 }\n";
        let diags = diags_for(src);
        assert!(
            !diags.is_empty(),
            "expected naive-str-match warning, got none"
        );
        assert!(diags[0].message.contains("`str contains`"));
    }

    #[test]
    fn safe_nu_with_from_json_is_clean() {
        let src = "#!/usr/bin/env nu\n\
                   let input = open --raw /dev/stdin | from json\n\
                   if ($input.tool_name | str contains \"Bash\") { exit 0 }\n";
        assert!(
            diags_for(src).is_empty(),
            "from json present — should be clean"
        );
    }

    #[test]
    fn naive_bash_no_jq_warns() {
        let src = "#!/usr/bin/env bash\n\
                   input=$(cat /dev/stdin)\n\
                   if echo \"$input\" | grep -q \"Bash\"; then exit 0; fi\n";
        let diags = diags_for(src);
        assert!(
            !diags.is_empty(),
            "expected naive-str-match warning, got none"
        );
        assert!(diags[0].message.contains("`grep -q`"));
    }

    #[test]
    fn safe_bash_with_jq_is_clean() {
        let src = "#!/usr/bin/env bash\n\
                   tool=$(cat /dev/stdin | jq -r '.tool_name')\n\
                   if [ \"$tool\" = \"Bash\" ]; then exit 0; fi\n";
        assert!(diags_for(src).is_empty(), "jq present — should be clean");
    }

    #[test]
    fn no_stdin_read_is_clean() {
        let src = "#!/usr/bin/env nu\n\
                   let val = \"some fixed string\"\n\
                   if ($val | str contains \"foo\") { exit 0 }\n";
        assert!(diags_for(src).is_empty(), "no stdin read — should not warn");
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
