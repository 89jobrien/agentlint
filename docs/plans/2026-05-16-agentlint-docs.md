# agentlint-docs — Design Document

**Date**: 2026-05-16
**Status**: approved
**Author**: Joseph O'Brien

---

## Goal

Add an `agentlint-docs` crate that validates frontmatter in `docs/**/*.md` files using
agentlint's own engine. Establishes a filename convention and frontmatter schema for project
documentation so that doc structure is machine-checkable.

---

## Filename Convention

```
docs/{doctype}.{stub}.md
```

- `doctype` — document type; must be one of the known enum values (see below)
- `stub` — project or topic identifier; treated as the project name mechanically
- Derived `id` = `{stub}-{doctype}` (e.g. `roadmap.agentlint.md` → `agentlint-roadmap`)

Files under `docs/` that do not start with a frontmatter fence (`---`) are silently skipped.
Files that do not match the `{doctype}.{stub}.md` pattern emit a filename-format warning.

---

## Frontmatter Schema

```yaml
---
title: agentlint-roadmap # required — must equal {stub}-{doctype}
doctype: roadmap # required — must equal doctype parsed from filename
project: agentlint # required — must equal stub parsed from filename
status: active # required — one of the known status values
created: 2026-05-16 # required — YYYY-MM-DD
updated: 2026-05-16 # required — YYYY-MM-DD
meta: # optional — opaque; presence valid, no internal validation
  author: Joseph O'Brien
  version: 0.0.1
---
```

### `doctype` enum

| Value               | Description                                               |
| ------------------- | --------------------------------------------------------- |
| `roadmap`           | Project roadmap — shipped milestones and planned work     |
| `plan`              | Design or implementation plan for a specific feature      |
| `adr`               | Architecture Decision Record                              |
| `spec`              | Detailed specification for a component or protocol        |
| `guide`             | How-to or tutorial document                               |
| `reference`         | Reference material (API, CLI flags, config keys)          |
| `runbook`           | Operational runbook — incident response, deployment steps |
| `architecture`      | System architecture overview                              |
| `capability-matrix` | Feature/platform capability comparison                    |
| `testing`           | Testing strategy or test plan                             |
| `development`       | Development setup, conventions, contributing guide        |
| `readme`            | Project-level README equivalent in docs form              |

### `status` enum

| Value        | Meaning                                       |
| ------------ | --------------------------------------------- |
| `draft`      | Work in progress; not authoritative           |
| `active`     | Current and maintained                        |
| `archived`   | Retained for history; superseded or obsolete  |
| `superseded` | Replaced by another document (link in `meta`) |

### Date format

`created` and `updated` must match `YYYY-MM-DD`. Any other format is an error.

### `meta` block

Optional. The `meta` key is accepted as an opaque string — the validator checks it is
present and non-empty when supplied, but does not validate internal keys. This allows
`author`, `version`, `superseded-by`, and other per-doctype fields without schema churn.

---

## Architecture

### New crate: `agentlint-docs`

```
crates/agentlint-docs/
  Cargo.toml
  src/
    lib.rs      # DocsValidator — implements Validator trait
```

**Cargo.toml dependencies:**

```toml
agentlint-core        = { workspace = true }
agentlint-frontmatter = { workspace = true }
```

No new external dependencies.

### File patterns

```rust
fn patterns(&self) -> &[&str] {
    &["docs/**/*.md"]
}
```

### Validation logic (DocsValidator::validate)

1. **Fence check** — if `src` does not start with `---\n`, return `vec![]` (skip).
2. **Parse frontmatter** — call `agentlint_frontmatter::parse(src)`. Unclosed fence → error.
3. **Filename parse** — split `path.file_stem()` on `.` to extract `(doctype_str, stub)`.
   If the filename does not match `{two-part-stem}.md`, emit
   `docs/frontmatter/invalid-filename` warning.
4. **Required field presence** — check `title`, `doctype`, `project`, `status`, `created`,
   `updated` all exist and are non-empty.
5. **Cross-field rules**:
   - `title` must equal `{stub}-{doctype_str}`
   - `doctype` field value must equal `doctype_str` from filename
   - `project` field value must equal `stub` from filename
6. **Enum validation**:
   - `doctype` value must be in `KNOWN_DOCTYPES`
   - `status` value must be in `KNOWN_STATUSES`
7. **Date validation** — `created` and `updated` must match `YYYY-MM-DD` (regex or manual
   parse: four digits, hyphen, two digits, hyphen, two digits).
8. **`meta` check** — if present, value must not be empty/whitespace.

The validator uses `agentlint_frontmatter::parse` directly (not `FrontmatterValidator`
builder) because cross-field rules (title derived from filename) cannot be expressed in
`FieldRule`.

### Rules emitted

| Rule ID                             | Severity | Condition                                     |
| ----------------------------------- | -------- | --------------------------------------------- |
| `docs/frontmatter/invalid-filename` | Warning  | Filename does not match `{doctype}.{stub}.md` |
| `docs/frontmatter/missing-title`    | Error    | `title` absent or empty                       |
| `docs/frontmatter/missing-doctype`  | Error    | `doctype` absent or empty                     |
| `docs/frontmatter/missing-project`  | Error    | `project` absent or empty                     |
| `docs/frontmatter/missing-status`   | Error    | `status` absent or empty                      |
| `docs/frontmatter/missing-created`  | Error    | `created` absent or empty                     |
| `docs/frontmatter/missing-updated`  | Error    | `updated` absent or empty                     |
| `docs/frontmatter/title-mismatch`   | Error    | `title` ≠ `{stub}-{doctype}`                  |
| `docs/frontmatter/doctype-mismatch` | Error    | `doctype` field ≠ doctype from filename       |
| `docs/frontmatter/project-mismatch` | Error    | `project` field ≠ stub from filename          |
| `docs/frontmatter/unknown-doctype`  | Error    | `doctype` value not in known enum             |
| `docs/frontmatter/unknown-status`   | Error    | `status` value not in known enum              |
| `docs/frontmatter/invalid-date`     | Error    | `created` or `updated` not `YYYY-MM-DD`       |
| `docs/frontmatter/empty-meta`       | Warning  | `meta` key present but value is empty         |

---

## Workspace integration

### `Cargo.toml` additions

Add `crates/agentlint-docs` to workspace members and wire as a dependency of the root
`agentlint` binary:

```toml
# workspace.members
"crates/agentlint-docs",

# workspace.dependencies
agentlint-docs = { path = "crates/agentlint-docs", version = "0.6.0" }

# [dependencies] in root Cargo.toml
agentlint-docs = { workspace = true }
```

### `src/main.rs`

Register `DocsValidator` alongside the existing validators in the runner dispatch list.

---

## Existing doc updates

`docs/roadmap.agentlint.md` already has frontmatter that is close to the schema but will
need `doctype: roadmap` and `project: agentlint` fields added. The `meta` block already
carries `author` and `version`. This file will serve as the first integration test fixture.

`docs/plans/2026-05-15-agentlint.md` uses a different naming convention (`YYYY-MM-DD-name`)
and no frontmatter fence — it will be silently skipped (no fence → skip).

---

## Out of scope

- Internal validation of `meta` block keys
- `FieldFormat::OneOf` or `FieldFormat::IsoDate` additions to `agentlint-frontmatter` (YAGNI)
- Linting non-`docs/` markdown files (README.md, CLAUDE.md, etc.)
- Auto-fix (`--fix`) mode
- Cross-document validation (e.g. `superseded-by` points to a real file)
