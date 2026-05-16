# Difficulty Tiers

**Date**: 2026-05-16
**Status**: approved

## Goal

Add a named difficulty system to agentlint so users can control which rules fire.
Three levels — `easy`, `hard`, `painful` — gate rules by operational cost of
fixing them. A `.agentlint.toml` config file sets the repo default; the CLI
`--difficulty` flag always wins over config.

## Difficulty Levels

| Level     | What fires                                                                               |
| --------- | ---------------------------------------------------------------------------------------- |
| `easy`    | Definite breakage only: invalid JSON, missing shebang, empty files, credential exposure  |
| `hard`    | Breakage + operational problems: hook leaks, dangerous settings, missing required fields |
| `painful` | Everything: best-practice style, stale allows, broad permissions, naive patterns         |

Default when neither config nor CLI specifies: `hard`.

## Architecture

### Changes to `agentlint-core`

**`Diagnostic`** gains two new fields:

```rust
pub struct Diagnostic {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub severity: Severity,
    pub message: String,
    // new
    pub rule: &'static str,      // e.g. "claude/settings/broad-read"
    pub difficulty: Difficulty,  // Relaxed | Strict | Painful
}
```

**`Difficulty`** enum (ordered for comparison):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Difficulty {
    Easy,
    Hard,
    Painful,
}
```

**`Diagnostic::error` / `Diagnostic::warning`** gain `rule` and `difficulty`
parameters. All existing call sites updated.

**Runner** filters diagnostics after collection:

```rust
result.diagnostics.retain(|d| d.difficulty <= config.difficulty);
```

**`RunConfig`** (new struct passed to runner):

```rust
pub struct RunConfig {
    pub difficulty: Difficulty,
    pub rule_overrides: HashMap<&'static str, RuleOverride>, // "error"|"warning"|"off"
    pub ignores: Vec<IgnoreEntry>,
}

pub enum RuleOverride { Error, Warning, Off }

pub struct IgnoreEntry {
    pub path_glob: String,
    pub rules: Vec<String>,
}
```

### Rule ID convention

`<validator>/<category>/<slug>` — all lowercase, hyphen-separated:

- `claude/settings/invalid-json`
- `claude/settings/sleep-in-allow`
- `claude/settings/sshpass-credential`
- `claude/settings/broad-read`
- `claude/settings/skip-dangerous-mode`
- `claude/settings/ci-workflow-in-allow`
- `claude/settings/stale-one-off-allow`
- `claude/settings/sleep-in-hook`
- `claude/settings/expensive-hook-command`
- `claude/settings/too-many-hooks`
- `claude/hooks/missing-shebang`
- `claude/hooks/no-execute-bit`
- `claude/hooks/naive-str-contains`
- `claude/agents/missing-name`
- `claude/agents/missing-description`
- (etc. for all existing rules)

### `.agentlint.toml` config file

Searched for at the repo root (cwd). Not required — all fields have defaults.

```toml
[agentlint]
difficulty = "hard"   # easy | hard | painful

[rules]
# Per-rule overrides: "error" | "warning" | "off"
"claude/settings/broad-read" = "off"
"claude/hooks/naive-str-contains" = "error"

[[ignore]]
path  = ".claude/settings.local.json"
rules = ["claude/settings/broad-read"]
```

### CLI

```
agentlint [--difficulty easy|hard|painful] [--format gnu|json] [paths...]
```

`--difficulty` always overrides config. When absent, config value is used.
When config is absent, default is `strict`.

### Output

GNU format gains the rule ID:

```
.claude/settings.json:1:1: warning[claude/settings/broad-read]: ...
```

JSON format gains `rule` and `difficulty` fields on each entry.

## Crates Affected

| Crate                | Changes                                                                   |
| -------------------- | ------------------------------------------------------------------------- |
| `agentlint-core`     | `Diagnostic`, `Difficulty`, `RunConfig`, runner filter, output formatters |
| `agentlint-claude`   | All `Diagnostic::error/warning` call sites — add rule + difficulty        |
| `agentlint-cursor`   | Same                                                                      |
| `agentlint-codex`    | Same                                                                      |
| `agentlint-opencode` | Same                                                                      |
| `agentlint-gemini`   | Same                                                                      |
| `agentlint-pi`       | Same                                                                      |
| `agentlint` (bin)    | CLI flag, config file loading                                             |

## Difficulty assignments for existing rules

| Rule                                  | Difficulty |
| ------------------------------------- | ---------- |
| invalid JSON / parse errors           | easy       |
| missing shebang / no execute bit      | easy       |
| empty AGENTS.md / missing frontmatter | easy       |
| missing required frontmatter fields   | easy       |
| sshpass credential in allow           | easy       |
| sleep in hook command                 | easy       |
| sleep in allow entry                  | easy       |
| hook missing `hooks` array            | easy       |
| hook missing `command` field          | easy       |
| expensive hook command (cargo, node)  | hard       |
| too many hooks per matcher            | hard       |
| skip-dangerous-mode-permission-prompt | hard       |
| ci-workflow in allow                  | hard       |
| naive str-contains in hook script     | hard       |
| unknown top-level settings key        | hard       |
| broad Read() permission               | painful    |
| stale one-off allow entry             | painful    |

## Out of Scope

- Per-path difficulty overrides (use `[[ignore]]` instead)
- Remote config (URL-based)
- Rule authoring API for external validators
- `--fix` auto-remediation
