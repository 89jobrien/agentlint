# agentlint roadmap

Tracks shipped milestones and planned work. Issues are on GitHub; this file captures the
high-level narrative.

---

## Shipped

### v0.1.0 — foundation

- Cargo workspace skeleton: `agentlint-core`, `agentlint-claude`, `agentlint-cursor`
- `Diagnostic` type, `Validator` trait, glob-based file dispatch, runner
- GNU (`path:line:col: severity: msg`) and JSON output formatters
- `--format`, `--exit-zero`, `--version` flags

### v0.2.0 — false-positive reduction pass

- Binary file skip
- YAML quote stripping in frontmatter parser
- Namespaced skill names (`namespace:slug`)
- Hooks extension allowlist
- `.maestro` directory skip
- Per-rule deduplication in settings validator
- Pretty output format with TTY auto-detect

### v0.3.0 — codex + opencode + mcp

- `agentlint-codex`: `AGENTS.md` non-empty check
- `agentlint-opencode`: `AGENTS.md` + `opencode.json` valid-JSON check
- `agentlint-claude`: `.mcp.json` hardcoded-secret and transport validation

### v0.4.0 — settings + cursor depth

- `claude/settings/broad-bash-allow` (#47) — warn on bare `Bash` / `Bash(*)` allow entries
- `cursor/frontmatter/never-fires` (#48) — warn when rule has no globs and `alwaysApply` unset
- `cursor/frontmatter/unknown-key` (#49) — warn on unrecognised frontmatter keys
- `codex/content/no-commands-section` (#50) — warn when `AGENTS.md` lacks a commands section

### v0.4.1 — sentinel review fixes

- `Bash(**)` added to broad-bash-allow gate
- `alwaysApply` comparison made case-insensitive
- Word-boundary keyword matching (fixes `"run"` inside `"runner"` false positive)
- `never-fires` gated to `.mdc`/`.md` only (not `.cursorrules`)
- Message text alignment

### v0.5.0 — gemini / pi / opencode structural parity

- `gemini/content/no-heading`, `gemini/content/too-sparse`,
  `gemini/content/no-commands-section` (#51)
- `pi/content/no-heading`, `pi/content/too-sparse` for `AGENTS.md` and `SYSTEM.md` (#52)
- `opencode/content/no-heading`, `opencode/content/too-sparse`,
  `opencode/content/no-commands-section` (#53)
- 257 tests passing across all crates

---

## In progress / planned

### Code quality — quick wins (#54 – #57)

| Issue | Description                                                   |
| ----- | ------------------------------------------------------------- |
| [#54] | Extract magic numbers to named constants (6 sites)            |
| [#55] | Suppress dead-code false positives on cross-crate public API  |
| [#56] | `ConfigError` missing `std::error::Error` impl (BP-002)       |
| [#57] | Suppress intentional `expect`/`unwrap` in test infrastructure |

Resolves ~11 `cargo qual` findings; raises quality score from 87% toward 92%.

### Refactoring — validator internals (#58 – #63)

| Issue | Description                                                                 |
| ----- | --------------------------------------------------------------------------- |
| [#58] | Resolve IOSP violations in validator functions (logic + calls mixed)        |
| [#59] | Split `settings.rs::validate` — 295 lines, cyclomatic complexity 32         |
| [#60] | Split `mcp.rs::validate_server_entry` — 144 lines, cyclomatic complexity 21 |
| [#61] | Split `skills.rs::validate_name` — 128 lines                                |
| [#62] | Split `cursor/lib.rs::validate` — 154 lines                                 |
| [#63] | `agentlint-core/src/lib.rs` — long `format_pretty`, SRP split               |

Target: `cargo qual` score ≥ 95%, zero LONG_FN / COMPLEXITY findings.

### New rules (backlog)

These are identified gaps not yet assigned issues:

- **Claude Code / agents**: warn when `tools:` list contains an unknown tool name
- **Claude Code / settings**: `hooks` section missing `matcher` field
- **Claude Code / settings**: `permissions.deny` overriding a `permissions.allow` entry
- **Cursor**: `globs` field present but value is a YAML list (Cursor only accepts a string)
- **Codex / OpenCode / Pi**: warn when file is valid but contains no code-fence blocks
- **All platforms**: `--fix` mode for auto-repairable diagnostics (missing fields with safe
  defaults, trailing commas in globs)

### Distribution

- Publish to crates.io on each version tag (currently `publish = true`, CI gate needed)
- Pre-built binaries via GitHub Releases (musl + Apple Silicon)
- Homebrew formula

---

## Quality metrics

| Version | Tests | `cargo qual` score | Findings |
| ------- | ----- | ------------------ | -------- |
| v0.5.0  | 257   | 87.2%              | 38       |
| target  | —     | ≥ 95%              | ≤ 10     |

[#54]: https://github.com/89jobrien/agentlint/issues/54
[#55]: https://github.com/89jobrien/agentlint/issues/55
[#56]: https://github.com/89jobrien/agentlint/issues/56
[#57]: https://github.com/89jobrien/agentlint/issues/57
[#58]: https://github.com/89jobrien/agentlint/issues/58
[#59]: https://github.com/89jobrien/agentlint/issues/59
[#60]: https://github.com/89jobrien/agentlint/issues/60
[#61]: https://github.com/89jobrien/agentlint/issues/61
[#62]: https://github.com/89jobrien/agentlint/issues/62
[#63]: https://github.com/89jobrien/agentlint/issues/63
