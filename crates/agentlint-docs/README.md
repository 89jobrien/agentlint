# agentlint-docs

Schema-driven validation for Markdown documentation frontmatter and filenames. The crate provides
the `DocsValidator` used directly by the CLI, schema loading and discovery, and corpus-based schema
inference.

## Workspace role

`agentlint-docs` depends on `agentlint-core` for the `Validator` and diagnostic contracts and on
`agentlint-frontmatter` for line-aware parsing. It is separate from the general TOML plugin engine:
docs schemas describe frontmatter fields and naming conventions, while declarative plugins describe
ordered lint rules.

By default the validator claims `docs/**/*.md`. Markdown without an opening frontmatter fence is
silently skipped. A present but unclosed fence is an error.

## Default schema

The built-in `agentlint-default` JSON schema is compiled into the crate.

| Property             | Default                                                       |
| -------------------- | ------------------------------------------------------------- |
| Required fields      | `title`, `doctype`, `project`, `status`, `created`, `updated` |
| Statuses             | `draft`, `active`, `archived`, `superseded`                   |
| Date fields          | `created`, `updated` as `YYYY-MM-DD` values                   |
| Repo filename        | `{doctype}.{project}.md`                                      |
| Research filenames   | `{ref}-{topic}.{doctype}.md` or `{ref}-{topic}.md`            |
| Research directories | `ideas`, `specs`, `plans`                                     |

Allowed doctypes are `idea`, `spec`, `plan`, `adr`, `roadmap`, `guide`, `reference`, `runbook`,
`architecture`, `capability-matrix`, `testing`, `development`, and `readme`.

Date validation checks four-digit years from 2000 onward, months 1 through 12, and days 1 through 31. It validates shape and ranges, not calendar-specific month lengths or leap days.

## Validation behavior

`DocsValidator` accumulates findings for:

- missing or empty required fields;
- unknown `status` or `doctype` values;
- malformed configured date fields;
- filenames that match no configured convention;
- `doctype`, `project`, `status`, or derived `title` values that disagree with filename tokens;
- missing plan metadata and malformed, empty, or non-mapping `meta` values.

Filename conventions are tried in order after directory scoping. Supported tokens are
`{doctype}`, `{project}`, `{topic}`, `{ref}`, and `{status}`. `{ref}` recognizes a leading
`YYYY-MM-DD` or numeric prefix. Field tokens map back to frontmatter. A title is expected as
`{topic}-{doctype}`, or `{project}-{doctype}` for repo-style names.

Plan files identified by a matched `doctype` token warn when no `meta` field exists. If `meta` is
present, it must parse as a non-empty YAML mapping; inline JSON mappings and indented YAML mappings
are accepted. The validator asks for a spec reference in its warning, but currently verifies the
presence and mapping shape of `meta`, not a specific `meta.spec` key.

## Public API

```rust
use agentlint_core::Validator;
use agentlint_docs::{DocsSchema, DocsValidator};
use std::path::Path;

let schema = DocsSchema::default();
let validator = DocsValidator::new(schema);
let diagnostics = validator.validate(
    Path::new("docs/guide.agentlint.md"),
    "---\ntitle: agentlint-guide\ndoctype: guide\nproject: agentlint\n\
     status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n",
);
assert!(diagnostics.is_empty());
```

Primary types and functions:

| API                             | Purpose                                                          |
| ------------------------------- | ---------------------------------------------------------------- |
| `DocsSchema`                    | Serializable schema fields, enums, dates, glob, and conventions. |
| `FilenameConvention`            | Directory-scoped filename template with `match_stem`.            |
| `DocsValidator::new`            | Construct a validator and expose the schema's file glob.         |
| `DocsValidator::set_infer_mode` | Defer per-file checks and infer in `validate_batch`.             |
| `DocsSchema::from_config_path`  | Resolve a schema and apply inline `.agentlint.toml` overrides.   |
| `infer_schema`                  | Derive schema properties from `&[(&Path, &str)]`.                |
| `SchemaRegistry`                | Register, discover, name, and load schema providers.             |

## Schema configuration

`DocsSchema::from_config_path` reads the `[docs]` section. `schema_file` takes precedence over
`schema`; explicit inline fields then override the selected base schema.

```toml
[docs]
schema = "agentlint-default"
file_glob = "docs/**/*.md"
required_fields = ["title", "doctype", "status"]
doctypes = ["spec", "plan", "guide"]
statuses = ["draft", "active", "archived"]
date_fields = ["created", "updated"]

[[docs.conventions]]
dirs = ["specs", "plans"]
format = "{ref}-{topic}.{doctype}.md"

[[docs.conventions]]
format = "{doctype}.{project}.md"
```

Named JSON schemas are discovered from `.agentlint/schemas/*.json` relative to the configuration
file's directory. The file stem becomes the schema name. `plugin::DocsSchemaPlugin` is the provider
contract; `BuiltinJsonPlugin` and `FileJsonPlugin` implement it. `SchemaRegistry::builtin` contains
`agentlint-default`, while `SchemaRegistry::discover` adds local JSON providers.

All `DocsSchema` fields use serde defaults. Consequently a partial JSON schema fills omitted fields
from `DocsSchema::default`, while inline TOML overrides replace whole vectors rather than merging
individual values.

## Schema inference

`infer_schema` uses files whose frontmatter parses successfully:

- fields present in at least 80% of all corpus files become required;
- fields with valid-looking dates in at least 90% of their non-empty values become date fields;
- observed `doctype` and `status` values become sorted unique enum lists;
- research conventions are added when the corpus includes `ideas`, `specs`, or `plans`, followed by
  the repo convention.

The CLI exposes two paths:

```console
agentlint --infer-schema docs
agentlint --emit-schema docs
```

`--infer-schema` infers once from the matched batch and validates that corpus against the result.
`--emit-schema` walks Markdown under directories named `docs`, prints the inferred schema as JSON,
and exits without normal lint output.

## Testing and development

Tests cover default and custom schemas, registry discovery, inference thresholds, filename token
matching, cross-field consistency, dates, metadata forms, and infer-mode batch validation.

```console
cargo nextest run -p agentlint-docs
cargo clippy -p agentlint-docs -- -D warnings
cargo fmt --all --check
```
