# agentlint-frontmatter

Shared YAML frontmatter parser used by Claude Code, Cursor, and Docs
validators.

## Grammar

```
"---" newline field* "---" newline body
```

Produces `Vec<Field { key, value, line }>` with 1-indexed line numbers
for accurate diagnostics.

## Modules

| Module       | Description                                                    |
| ------------ | -------------------------------------------------------------- |
| `lib.rs`     | `parse()` -- nom-based frontmatter extraction                  |
| `builder.rs` | `FrontmatterValidator` builder API for declarative field rules |

## Builder API

```rust
let v = FrontmatterValidator::builder()
    .required(FieldRule::new("name").max_len(64).format(FieldFormat::KebabCase))
    .required(FieldRule::new("description"))
    .optional(FieldRule::new("slug").format(FieldFormat::KebabCase))
    .build();
```

Constraints: `max_len`, `format(KebabCase)`, `matches_dir_name`.

## Testing

Property tests (proptest) cover kebab-case validation. See
`TODO(testing/property)` and `TODO(testing/fuzz)` comments for planned
coverage.
