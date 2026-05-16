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
        check_no_exit_code(path, src, &mut diags);
        check_infinite_loop(path, src, &mut diags);
        check_sleep_in_body(path, src, &mut diags);

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
            format!(
                "hook matches on raw stdin with {} without parsing JSON first; \
                 use `from json` / `jq` to extract specific fields before matching",
                label
            ),
        )
        .with_rule("claude/hooks/naive-str-match", Difficulty::Normal),
    );
}

// ---------------------------------------------------------------------------
// no-exit-code-check detector
// ---------------------------------------------------------------------------

const HEAVY_COMMANDS: &[&str] = &[
    "cargo ", "git ", "npm ", "make ", "docker ", "kubectl ", "yarn ", "pip ", "go ",
];

const EXIT_CODE_GUARDS: &[&str] = &[
    "if.*exit_code",
    "| complete",
    ".exit_code",
    "$?",
    "exit_code != 0",
    "exit_code == 0",
    "$status",
    "PIPESTATUS",
    "|| exit",
    "|| return",
    "&& exit",
];

const UNCONDITIONAL_APPROVE: &[&str] = &[
    "\"approve\"",
    "permissionDecision.*approve",
    "\"decision\": \"approve\"",
    "\"decision\":\"approve\"",
    "'approve'",
];

fn has_heavy_command(src: &str) -> bool {
    HEAVY_COMMANDS.iter().any(|p| src.contains(p))
}

fn has_unconditional_approve(src: &str) -> bool {
    UNCONDITIONAL_APPROVE.iter().any(|p| src.contains(p))
}

fn has_exit_code_guard(src: &str) -> bool {
    // Some guards are plain strings, some are pseudo-regex — just do substring match here.
    EXIT_CODE_GUARDS.iter().any(|p| src.contains(p))
}

fn check_no_exit_code(path: &Path, src: &str, diags: &mut Vec<Diagnostic>) {
    if !has_heavy_command(src) {
        return;
    }
    if !has_unconditional_approve(src) {
        return;
    }
    if has_exit_code_guard(src) {
        return;
    }
    // Find the line with the first heavy command for reporting.
    let line = src
        .lines()
        .enumerate()
        .find(|(_, l)| HEAVY_COMMANDS.iter().any(|p| l.contains(p)))
        .map(|(i, _)| i + 1)
        .unwrap_or(1);
    diags.push(
        Diagnostic::warning(
            path,
            line,
            1,
            "hook runs an external command but outputs an unconditional approve without \
             checking the exit code; add an exit-code guard (e.g. `| complete`, `$?`, \
             `|| exit 1`) before approving",
        )
        .with_rule("claude/hooks/no-exit-code-check", Difficulty::Hard),
    );
}

// ---------------------------------------------------------------------------
// infinite-loop detector
// ---------------------------------------------------------------------------

const BASH_TOOL_MATCH_PATTERNS: &[&str] = &[
    "\"Bash\"",
    "'Bash'",
    "tool_name.*Bash",
    "Bash.*tool_name",
    "tool_name == \"Bash\"",
    "tool_name == 'Bash'",
];

fn is_post_tool_use_hook(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("PostToolUse") || s.contains("post-") || s.contains("post_")
}

fn matches_bash_tool(src: &str) -> bool {
    BASH_TOOL_MATCH_PATTERNS.iter().any(|p| src.contains(p))
}

fn check_infinite_loop(path: &Path, src: &str, diags: &mut Vec<Diagnostic>) {
    if !is_post_tool_use_hook(path) {
        return;
    }
    if !matches_bash_tool(src) {
        return;
    }
    if !has_heavy_command(src) {
        return;
    }
    // Re-entrancy guard check.
    const REENTRY_GUARDS: &[&str] = &[
        "AGENTLINT_HOOK_RUNNING",
        "HOOK_RUNNING",
        "RUNNING",
        "__HOOK_",
    ];
    if REENTRY_GUARDS.iter().any(|g| src.contains(g)) {
        return;
    }
    let line = src
        .lines()
        .enumerate()
        .find(|(_, l)| BASH_TOOL_MATCH_PATTERNS.iter().any(|p| l.contains(p)))
        .map(|(i, _)| i + 1)
        .unwrap_or(1);
    diags.push(
        Diagnostic::warning(
            path,
            line,
            1,
            "PostToolUse hook matches on Bash and runs shell commands (cargo/git/npm) that \
             would re-trigger this hook, causing an infinite loop; add a re-entrancy guard \
             (e.g. check/set `AGENTLINT_HOOK_RUNNING` env var) or avoid running Bash \
             commands from a Bash PostToolUse hook",
        )
        .with_rule("claude/hooks/infinite-loop", Difficulty::Hard),
    );
}

// ---------------------------------------------------------------------------
// sleep-in-body detector
// ---------------------------------------------------------------------------

/// Patterns that indicate a blocking sleep call in the hook body.
const SLEEP_PATTERNS: &[&str] = &["sleep ", "Start-Sleep"];

fn check_sleep_in_body(path: &Path, src: &str, diags: &mut Vec<Diagnostic>) {
    for (lineno, line) in src.lines().enumerate() {
        let trimmed = line.trim_start();
        // Skip comment lines.
        if trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        for pat in SLEEP_PATTERNS {
            if line.contains(pat) {
                diags.push(
                    Diagnostic::error(
                        path,
                        lineno + 1,
                        1,
                        "hook body contains a `sleep` call; sleep blocks the agent for the \
                         duration of every tool call the hook fires on — remove it",
                    )
                    .with_rule("claude/hooks/sleep-in-body", Difficulty::Easy),
                );
                return;
            }
        }
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

    // --- no-exit-code-check tests ---

    fn exit_code_diags(path: &str, src: &str) -> Vec<Diagnostic> {
        HooksValidator::validate(Path::new(path), src)
            .into_iter()
            .filter(|d| d.rule == "claude/hooks/no-exit-code-check")
            .collect()
    }

    #[test]
    fn no_exit_code_check_warns_when_cargo_and_approve_no_guard() {
        let src = "#!/usr/bin/env bash\ncargo build\necho '{\"decision\":\"approve\"}'\n";
        let diags = exit_code_diags("/tmp/my-hook", src);
        assert!(!diags.is_empty(), "expected no-exit-code-check warning");
    }

    #[test]
    fn no_exit_code_check_clean_when_guard_present() {
        let src = "#!/usr/bin/env bash\n\
                   result=$(cargo build 2>&1)\n\
                   if [ $? -ne 0 ]; then exit 1; fi\n\
                   echo '{\"decision\":\"approve\"}'\n";
        let diags = exit_code_diags("/tmp/my-hook", src);
        assert!(
            diags.is_empty(),
            "exit-code guard present — should be clean"
        );
    }

    #[test]
    fn no_exit_code_check_clean_when_no_approve() {
        let src = "#!/usr/bin/env bash\ncargo build\necho done\n";
        let diags = exit_code_diags("/tmp/my-hook", src);
        assert!(diags.is_empty(), "no approve output — should be clean");
    }

    #[test]
    fn no_exit_code_check_nu_pipe_complete_is_clean() {
        let src = "#!/usr/bin/env nu\n\
                   let r = cargo build | complete\n\
                   if $r.exit_code != 0 { exit 1 }\n\
                   echo '{\"decision\":\"approve\"}'\n";
        let diags = exit_code_diags("/tmp/my-hook", src);
        assert!(diags.is_empty(), "| complete guard — should be clean");
    }

    // --- infinite-loop tests ---

    fn loop_diags(path: &str, src: &str) -> Vec<Diagnostic> {
        HooksValidator::validate(Path::new(path), src)
            .into_iter()
            .filter(|d| d.rule == "claude/hooks/infinite-loop")
            .collect()
    }

    #[test]
    fn infinite_loop_warns_on_bash_post_hook_running_cargo() {
        let src = "#!/usr/bin/env bash\n\
                   tool=$(cat /dev/stdin | jq -r '.tool_name')\n\
                   if [ \"$tool\" = \"Bash\" ]; then\n\
                     cargo fmt\n\
                   fi\n";
        let diags = loop_diags("/hooks/post-tool.sh", src);
        assert!(!diags.is_empty(), "expected infinite-loop warning");
    }

    #[test]
    fn infinite_loop_clean_with_reentry_guard() {
        let src = "#!/usr/bin/env bash\n\
                   [ -n \"$AGENTLINT_HOOK_RUNNING\" ] && exit 0\n\
                   export AGENTLINT_HOOK_RUNNING=1\n\
                   tool=$(cat /dev/stdin | jq -r '.tool_name')\n\
                   if [ \"$tool\" = \"Bash\" ]; then\n\
                     cargo fmt\n\
                   fi\n";
        let diags = loop_diags("/hooks/post-tool.sh", src);
        assert!(
            diags.is_empty(),
            "re-entrancy guard present — should be clean"
        );
    }

    #[test]
    fn infinite_loop_clean_when_not_post_hook() {
        let src = "#!/usr/bin/env bash\n\
                   tool=$(cat /dev/stdin | jq -r '.tool_name')\n\
                   if [ \"$tool\" = \"Bash\" ]; then\n\
                     cargo fmt\n\
                   fi\n";
        let diags = loop_diags("/hooks/pre-tool.sh", src);
        assert!(diags.is_empty(), "not a PostToolUse hook — should be clean");
    }

    #[test]
    fn infinite_loop_clean_when_no_bash_match() {
        let src = "#!/usr/bin/env bash\n\
                   tool=$(cat /dev/stdin | jq -r '.tool_name')\n\
                   if [ \"$tool\" = \"Edit\" ]; then\n\
                     cargo fmt\n\
                   fi\n";
        let diags = loop_diags("/hooks/post-tool.sh", src);
        assert!(diags.is_empty(), "matches Edit not Bash — should be clean");
    }

    // --- sleep-in-body tests ---

    fn sleep_diags(src: &str) -> Vec<Diagnostic> {
        HooksValidator::validate(Path::new("/tmp/my-hook"), src)
            .into_iter()
            .filter(|d| d.rule == "claude/hooks/sleep-in-body")
            .collect()
    }

    #[test]
    fn sleep_in_body_bash_is_error() {
        let src = "#!/usr/bin/env bash\nsleep 5\necho done\n";
        let diags = sleep_diags(src);
        assert!(!diags.is_empty(), "expected sleep-in-body error");
        assert!(diags[0].message.contains("sleep"));
    }

    #[test]
    fn sleep_in_body_nu_is_error() {
        let src = "#!/usr/bin/env nu\nsleep 0.5sec\necho done\n";
        let diags = sleep_diags(src);
        assert!(
            !diags.is_empty(),
            "expected sleep-in-body error for nu sleep"
        );
    }

    #[test]
    fn sleep_in_body_powershell_is_error() {
        let src = "#!/usr/bin/env pwsh\nStart-Sleep -Seconds 3\nWrite-Output done\n";
        let diags = sleep_diags(src);
        assert!(
            !diags.is_empty(),
            "expected sleep-in-body error for Start-Sleep"
        );
    }

    #[test]
    fn sleep_in_comment_is_clean() {
        let src = "#!/usr/bin/env bash\n# sleep 5 was here but removed\necho done\n";
        let diags = sleep_diags(src);
        assert!(diags.is_empty(), "sleep in comment — should be clean");
    }

    #[test]
    fn no_sleep_is_clean() {
        let src = "#!/usr/bin/env bash\necho done\n";
        let diags = sleep_diags(src);
        assert!(diags.is_empty(), "no sleep — should be clean");
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
