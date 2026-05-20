# declarative — TOML plugin engine

Loads validator definitions from TOML files and evaluates them at runtime.
This is the engine behind `agentlint-plugins` and external plugin loading.

## Module layout

| File       | Responsibility                                                                                            |
| ---------- | --------------------------------------------------------------------------------------------------------- |
| `mod.rs`   | Re-exports, TOML loading (`load_plugin_file`, `load_plugin_str`), convenience constructors                |
| `types.rs` | Serde-derived TOML schema: `PluginFile`, `ValidatorDef`, `RuleDef`, `Check`, etc.                         |
| `parse.rs` | `ParsedContent` enum, format-specific parsers (YAML/JSON/frontmatter/markdown), field access helpers      |
| `eval.rs`  | `DeclarativeValidator` — implements `Validator` trait, dispatches `Check` variants against parsed content |
| `tests.rs` | Tests against embedded looprs and claude TOML plugins                                                     |

## Data flow

```
TOML plugin file
  → load_plugin_str() → PluginFile { meta, validators[] }
  → DeclarativeValidator::new(def, constants)
  → Validator::validate(path, src)
      → parse_{yaml,json,frontmatter}(src) → ParsedContent
      → eval_rule() per RuleDef → Vec<Diagnostic>
```

## Adding a new check

1. Add a variant to `Check` in `types.rs`
2. Add the match arm in `DeclarativeValidator::eval_rule()` in `eval.rs`
3. Use it in a plugin TOML file under `plugins/`
4. Add a test in `tests.rs`
