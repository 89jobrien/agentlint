# agentlint-docs

Validates documentation frontmatter against a configurable schema.

## Files validated

| Pattern        | Description                               |
| -------------- | ----------------------------------------- |
| `docs/**/*.md` | Documentation files with YAML frontmatter |

Files without a frontmatter fence are silently skipped.

## Default schema

**Required fields**: `title`, `doctype`, `project`, `status`, `created`,
`updated`.

**Enum values**:

- `status`: `draft`, `active`, `archived`, `superseded`
- `doctype`: `idea`, `spec`, `plan`, `adr`, `roadmap`, `guide`,
  `reference`, `runbook`, `architecture`, `capability-matrix`,
  `testing`, `development`, `readme`

**Date fields**: `created` and `updated` must be `YYYY-MM-DD`.

**`meta` field**: optional YAML mapping. Plan docs must include
`meta.spec`.

**Filename conventions**: validated against configurable format
templates with `{doctype}`, `{project}`, `{topic}`, `{ref}` tokens.

## Modules

| Module     | Description                                            |
| ---------- | ------------------------------------------------------ |
| `plugin`   | `DocsSchemaPlugin` trait, builtin/file impls           |
| `registry` | `SchemaRegistry` -- discovers and loads schema plugins |
| `infer`    | `infer_schema()` -- corpus-based schema inference      |

## CLI flags

- `--infer-schema` -- infer a schema from the docs corpus
- `--emit-schema` -- print the active schema as JSON
