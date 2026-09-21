# agentlint-frontmatter

Line-oriented YAML frontmatter parsing and reusable field validation for agentlint. The crate
depends on `agentlint-core` only for diagnostics and is consumed by `agentlint-docs`; the
consolidated native validators use the smaller parser in `agentlint_core::frontmatter` to avoid a
dependency cycle.

## Accepted input

Input must begin with an opening `---` line and contain a closing `---` line:

```text
---
name: review-agent
description:
  Reviews a change before merge
---
# Instructions
```

`parse` returns `Vec<Field>`, where each field contains `key`, normalized `value`, and the one-based
line where the key appeared. `ParseError::NoFence` means line 1 was not a fence;
`ParseError::UnclosedFence` means an opening fence had no close.

This is deliberately not a complete YAML parser. Field keys may contain Unicode alphanumeric
characters, hyphens, and underscores; the parser uses `char::is_alphanumeric`. Lines that cannot be
read as `key: value` are skipped. A fully quoted single-line value loses one matching quote pair.
Indented lines continue the previous field and are trimmed before being joined with newlines, which
supports the frontmatter shapes agentlint needs for block values and nested mappings.

```rust
use agentlint_frontmatter::parse;

let fields = parse("---\nname: review-agent\n---\n").expect("valid frontmatter");
assert_eq!(fields[0].key, "name");
assert_eq!(fields[0].line, 2);
```

## Required-field helper

`check_required(path, src, required)` parses the document and accumulates diagnostics for every
missing or empty required field. Fence errors become diagnostics at line 1; an empty value is
reported at its field line.

```rust
use agentlint_frontmatter::check_required;
use std::path::Path;

let diagnostics = check_required(
    Path::new(".claude/agents/reviewer.md"),
    "---\nname: reviewer\n---\n",
    &["name", "description"],
);
assert_eq!(diagnostics.len(), 1);
```

## Builder API

`FrontmatterValidator::builder()` composes required and optional `FieldRule` values:

```rust
use agentlint_frontmatter::{FieldFormat, FieldRule, FrontmatterValidator};
use std::path::Path;

let validator = FrontmatterValidator::builder()
    .only_file("SKILL.md")
    .required(
        FieldRule::new("name")
            .max_len(64)
            .format(FieldFormat::KebabCase)
            .matches_dir_name(),
    )
    .required(FieldRule::new("description"))
    .optional(FieldRule::new("slug").format(FieldFormat::KebabCase))
    .build();

let diagnostics = validator.validate(
    Path::new(".claude/skills/review/SKILL.md"),
    "---\nname: review\ndescription: Reviews changes\n---\n",
);
assert!(diagnostics.is_empty());
```

| Builder method                  | Contract                                                                     |
| ------------------------------- | ---------------------------------------------------------------------------- |
| `only_file(name)`               | Skip paths whose final component differs; no missing-fence error is emitted. |
| `required(rule)`                | Require the field and a non-empty value, then apply its constraints.         |
| `optional(rule)`                | Apply constraints only when the field exists and is non-empty.               |
| `FieldRule::max_len(n)`         | Limit the value's byte length to `n`.                                        |
| `FieldRule::format(KebabCase)`  | Allow lowercase ASCII, digits, and single inner hyphens.                     |
| `FieldRule::matches_dir_name()` | Require equality with the immediate parent directory name.                   |

Without `only_file`, missing frontmatter is an error. With `only_file`, a matching file with an
unclosed fence is still an error, while nonmatching support files are skipped before parsing.

## Testing and development

Unit tests cover fences, quoting, line numbers, continuation values, accumulated required fields,
builder filters, and each constraint. Proptest cases verify parser panic resistance, valid
frontmatter round trips, kebab-case invariants, length limits, and directory-name mismatches.

```console
cargo nextest run -p agentlint-frontmatter
cargo clippy -p agentlint-frontmatter -- -D warnings
cargo fmt --all --check
```
