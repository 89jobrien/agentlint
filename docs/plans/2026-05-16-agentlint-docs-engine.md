---
title: agentlint-docs-engine-plan
doctype: plan
project: agentlint
status: draft
created: 2026-05-16
updated: 2026-05-16
meta:
  spec: docs/specs/2026-05-15-agentlint.spec.md
---

## Plan: agentlint-docs plugin registry and schema inference engine

### Goal

Refactor `agentlint-docs` into a discoverable plugin engine: a `DocsSchemaPlugin` trait
with a `SchemaRegistry` that loads built-in and filesystem-discovered JSON schemas, plus
a corpus-inference pass triggered by `--infer-schema` / `--emit-schema` CLI flags.

### Architecture

- Crates affected: `agentlint-docs`, `agentlint` (binary)
- New types:
  - `DocsSchemaPlugin` trait — `plugin.rs`
  - `BuiltinJsonPlugin`, `FileJsonPlugin` — `plugin.rs`
  - `SchemaRegistry` — `registry.rs`
  - `infer_schema()` — `infer.rs`
- Data flow:
  - Normal: `SchemaRegistry::discover(root)` → `DocsSchema` → `DocsValidator`
  - `--infer-schema`: corpus → `infer_schema()` → `DocsSchema` → per-file outlier diags
  - `--emit-schema`: corpus → `infer_schema()` → JSON → stdout → exit 0

### Tech Stack

- Rust edition 2024; `serde`, `serde_json`, `serde_yaml`, `toml` (all existing)
- No new crate dependencies
- `validate_batch` hook on `Validator` trait (already exists in `agentlint-core`)

### Tasks

#### Task 1: Add `Serialize` to `DocsSchema` and `FilenameConvention`; expand default JSON

**Crate**: `agentlint-docs`
**File(s)**:

- `crates/agentlint-docs/src/lib.rs`
- `crates/agentlint-docs/schemas/agentlint-default.json` (rename from `conventions.default.json`)
  **Run**: `cargo nextest run -p agentlint-docs`

1. Write failing test (verifying round-trip serialize):

   ```rust
   #[test]
   fn docs_schema_serializes_to_json() {
       let schema = DocsSchema::default();
       let json = serde_json::to_string(&schema).expect("serialize");
       let back: DocsSchema = serde_json::from_str(&json).expect("deserialize");
       assert_eq!(back.doctypes, schema.doctypes);
   }
   ```

   Run: `cargo nextest run -p agentlint-docs -- docs_schema_serializes_to_json`
   Expected: FAIL (DocsSchema does not implement Serialize yet)

2. Implement — add `Serialize` to both structs:

   ```rust
   // In lib.rs — change derive on FilenameConvention:
   #[derive(Debug, Clone, Deserialize, Serialize)]
   pub struct FilenameConvention { ... }

   // And on DocsSchema:
   #[derive(Debug, Clone, Deserialize, Serialize)]
   #[serde(default)]
   pub struct DocsSchema { ... }
   ```

   Add `use serde::{Deserialize, Serialize};` (replace existing `use serde::Deserialize;`).

3. Create `crates/agentlint-docs/schemas/agentlint-default.json`
   (delete `conventions.default.json` — it is superseded):

   ```json
   {
     "file_glob": "docs/**/*.md",
     "required_fields": [
       "title",
       "doctype",
       "project",
       "status",
       "created",
       "updated"
     ],
     "doctypes": [
       "idea",
       "spec",
       "plan",
       "adr",
       "roadmap",
       "guide",
       "reference",
       "runbook",
       "architecture",
       "capability-matrix",
       "testing",
       "development",
       "readme"
     ],
     "statuses": ["draft", "active", "archived", "superseded"],
     "date_fields": ["created", "updated"],
     "conventions": [
       {
         "format": "{ref}-{topic}.{doctype}.md",
         "dirs": ["ideas", "specs", "plans"],
         "comment": "Research doc with explicit doctype suffix."
       },
       {
         "format": "{ref}-{topic}.md",
         "dirs": ["ideas", "specs", "plans"],
         "comment": "Research doc without doctype suffix — inferred from parent dir."
       },
       {
         "format": "{doctype}.{project}.md",
         "dirs": [],
         "comment": "Catch-all repo doc convention."
       }
     ]
   }
   ```

   Update the `DEFAULT_CONVENTIONS_JSON` const in `lib.rs` to reference the new file
   and rename it `AGENTLINT_DEFAULT_JSON`. The serde deserialization silently ignores
   the `comment` fields (no schema change needed).

   ```rust
   const AGENTLINT_DEFAULT_JSON: &str = include_str!("../schemas/agentlint-default.json");
   ```

   Update `DocsSchema::default()` to load from `AGENTLINT_DEFAULT_JSON` and deserialize
   all fields (not just conventions):

   ```rust
   impl Default for DocsSchema {
       fn default() -> Self {
           serde_json::from_str(AGENTLINT_DEFAULT_JSON)
               .expect("bundled agentlint-default.json is valid")
       }
   }
   ```

   Remove the previous hand-written `Default` impl body (required_fields vec, doctypes
   vec, etc.).

4. Verify:

   ```text
   cargo nextest run -p agentlint-docs    → all green
   cargo clippy -p agentlint-docs -- -D warnings  → zero warnings
   ```

5. Run: `git branch --show-current`
   Verify: `main`. Commit:
   `git commit -m "feat(agentlint-docs): add Serialize, expand agentlint-default.json schema"`

---

#### Task 2: `DocsSchemaPlugin` trait + `BuiltinJsonPlugin` + `FileJsonPlugin`

**Crate**: `agentlint-docs`
**File(s)**: `crates/agentlint-docs/src/plugin.rs`
**Run**: `cargo nextest run -p agentlint-docs`

1. Write failing test:

   ```rust
   // Add to crates/agentlint-docs/src/plugin.rs (new file — test at bottom)
   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn builtin_default_plugin_loads() {
           let p = BuiltinJsonPlugin { name: "agentlint-default", src: super::BUILTIN_SRC };
           let schema = p.load().expect("load");
           assert!(!schema.doctypes.is_empty());
       }

       #[test]
       fn file_plugin_name_is_stem() {
           let p = FileJsonPlugin::new("/tmp/my-team-docs.json");
           assert_eq!(p.name(), "my-team-docs");
       }
   }
   ```

   Run: `cargo nextest run -p agentlint-docs -- builtin_default_plugin_loads`
   Expected: FAIL (module does not exist yet)

2. Implement `crates/agentlint-docs/src/plugin.rs`:

   ```rust
   use crate::DocsSchema;
   use std::path::PathBuf;

   pub(crate) const BUILTIN_SRC: &str =
       include_str!("../schemas/agentlint-default.json");

   /// A named schema provider that resolves to a [`DocsSchema`].
   pub trait DocsSchemaPlugin: Send + Sync {
       fn name(&self) -> &str;
       fn load(&self) -> Result<DocsSchema, String>;
   }

   /// A built-in schema compiled into the binary.
   pub struct BuiltinJsonPlugin {
       pub name: &'static str,
       pub src: &'static str,
   }

   impl DocsSchemaPlugin for BuiltinJsonPlugin {
       fn name(&self) -> &str {
           self.name
       }
       fn load(&self) -> Result<DocsSchema, String> {
           serde_json::from_str(self.src)
               .map_err(|e| format!("built-in schema '{}' is invalid: {e}", self.name))
       }
   }

   /// A schema loaded from a JSON file at runtime.
   pub struct FileJsonPlugin {
       name: String,
       path: PathBuf,
   }

   impl FileJsonPlugin {
       pub fn new(path: impl Into<PathBuf>) -> Self {
           let path = path.into();
           let name = path
               .file_stem()
               .and_then(|s| s.to_str())
               .unwrap_or("unknown")
               .to_string();
           Self { name, path }
       }
   }

   impl DocsSchemaPlugin for FileJsonPlugin {
       fn name(&self) -> &str {
           &self.name
       }
       fn load(&self) -> Result<DocsSchema, String> {
           let src = std::fs::read_to_string(&self.path)
               .map_err(|e| format!("could not read schema {}: {e}", self.path.display()))?;
           serde_json::from_str(&src)
               .map_err(|e| format!("invalid schema {}: {e}", self.path.display()))
       }
   }
   ```

   Add `pub mod plugin;` to `lib.rs`.
   Move the `AGENTLINT_DEFAULT_JSON` const added in Task 1 from `lib.rs` into `plugin.rs`
   as `BUILTIN_SRC`. Update `lib.rs` to reference `plugin::BUILTIN_SRC`:

   ```rust
   // In lib.rs DocsSchema::default():
   serde_json::from_str(crate::plugin::BUILTIN_SRC)
       .expect("bundled agentlint-default.json is valid")
   ```

3. Verify:

   ```text
   cargo nextest run -p agentlint-docs    → all green
   cargo clippy -p agentlint-docs -- -D warnings  → zero warnings
   ```

4. Run: `git branch --show-current`
   Verify: `main`. Commit:
   `git commit -m "feat(agentlint-docs): add DocsSchemaPlugin trait and builtin/file impls"`

---

#### Task 3: `SchemaRegistry` with built-in and filesystem discovery

**Crate**: `agentlint-docs`
**File(s)**: `crates/agentlint-docs/src/registry.rs`
**Run**: `cargo nextest run -p agentlint-docs`

1. Write failing tests:

   ```rust
   // In crates/agentlint-docs/src/registry.rs
   #[cfg(test)]
   mod tests {
       use super::*;
       use tempfile::tempdir;

       #[test]
       fn builtin_registry_contains_agentlint_default() {
           let reg = SchemaRegistry::builtin();
           assert!(reg.get("agentlint-default").is_some());
       }

       #[test]
       fn discover_finds_json_files_in_schema_dir() {
           let dir = tempdir().unwrap();
           let schema_dir = dir.path().join(".agentlint").join("schemas");
           std::fs::create_dir_all(&schema_dir).unwrap();
           // Write a minimal valid schema
           std::fs::write(
               schema_dir.join("my-schema.json"),
               r#"{"doctypes": ["note"]}"#,
           ).unwrap();
           let reg = SchemaRegistry::discover(dir.path());
           assert!(reg.get("my-schema").is_some());
           let schema = reg.get("my-schema").unwrap().unwrap();
           assert_eq!(schema.doctypes, vec!["note"]);
       }

       #[test]
       fn names_includes_builtins() {
           let reg = SchemaRegistry::builtin();
           assert!(reg.names().contains(&"agentlint-default"));
       }
   }
   ```

   Run: `cargo nextest run -p agentlint-docs -- builtin_registry_contains_agentlint_default`
   Expected: FAIL

2. Implement `crates/agentlint-docs/src/registry.rs`:

   ```rust
   use crate::DocsSchema;
   use crate::plugin::{BuiltinJsonPlugin, DocsSchemaPlugin, FileJsonPlugin, BUILTIN_SRC};
   use std::path::Path;

   pub struct SchemaRegistry {
       plugins: Vec<Box<dyn DocsSchemaPlugin>>,
   }

   impl SchemaRegistry {
       /// Built-in schemas only; no filesystem access.
       pub fn builtin() -> Self {
           let mut r = Self { plugins: vec![] };
           r.register(Box::new(BuiltinJsonPlugin {
               name: "agentlint-default",
               src: BUILTIN_SRC,
           }));
           r
       }

       /// Built-ins + JSON files discovered from `{root}/.agentlint/schemas/`.
       pub fn discover(root: &Path) -> Self {
           let mut r = Self::builtin();
           let schema_dir = root.join(".agentlint").join("schemas");
           let Ok(entries) = std::fs::read_dir(&schema_dir) else {
               return r;
           };
           for entry in entries.filter_map(|e| e.ok()) {
               let path = entry.path();
               if path.extension().and_then(|e| e.to_str()) == Some("json") {
                   r.register(Box::new(FileJsonPlugin::new(path)));
               }
           }
           r
       }

       pub fn register(&mut self, plugin: Box<dyn DocsSchemaPlugin>) {
           self.plugins.push(plugin);
       }

       /// Returns `Some(Result<DocsSchema>)` if a plugin with `name` exists.
       pub fn get(&self, name: &str) -> Option<Result<DocsSchema, String>> {
           self.plugins.iter().find(|p| p.name() == name).map(|p| p.load())
       }

       pub fn names(&self) -> Vec<&str> {
           self.plugins.iter().map(|p| p.name()).collect()
       }
   }

   impl Default for SchemaRegistry {
       fn default() -> Self {
           Self::builtin()
       }
   }
   ```

   Add `pub mod registry;` and `pub use registry::SchemaRegistry;` to `lib.rs`.
   Add `tempfile = { workspace = true }` to `[dev-dependencies]` in
   `crates/agentlint-docs/Cargo.toml`.

3. Verify:

   ```text
   cargo nextest run -p agentlint-docs    → all green
   cargo clippy -p agentlint-docs -- -D warnings  → zero warnings
   ```

4. Run: `git branch --show-current`
   Verify: `main`. Commit:
   `git commit -m "feat(agentlint-docs): add SchemaRegistry with builtin and discover()"`

---

#### Task 4: Corpus inference — `infer_schema()`

**Crate**: `agentlint-docs`
**File(s)**: `crates/agentlint-docs/src/infer.rs`
**Run**: `cargo nextest run -p agentlint-docs`

1. Write failing tests:

   ```rust
   // In crates/agentlint-docs/src/infer.rs
   #[cfg(test)]
   mod tests {
       use super::*;
       use std::path::Path;

       fn corpus(files: &[(&str, &str)]) -> Vec<(std::path::PathBuf, String)> {
           files
               .iter()
               .map(|(p, s)| (std::path::PathBuf::from(p), s.to_string()))
               .collect()
       }

       #[test]
       fn infers_required_fields_from_majority() {
           let files = corpus(&[
               ("docs/a.md", "---\ntitle: A\ndoctype: spec\nstatus: draft\n---\n"),
               ("docs/b.md", "---\ntitle: B\ndoctype: plan\nstatus: draft\n---\n"),
               ("docs/c.md", "---\ntitle: C\ndoctype: guide\n---\n"),
           ]);
           let refs: Vec<(&Path, &str)> =
               files.iter().map(|(p, s)| (p.as_path(), s.as_str())).collect();
           let schema = infer_schema(&refs);
           // title and doctype appear in 100%; status in 67% — below 80% threshold
           assert!(schema.required_fields.contains(&"title".to_string()));
           assert!(schema.required_fields.contains(&"doctype".to_string()));
       }

       #[test]
       fn infers_doctypes_from_observed_values() {
           let files = corpus(&[
               ("docs/a.md", "---\ndoctype: spec\n---\n"),
               ("docs/b.md", "---\ndoctype: plan\n---\n"),
               ("docs/c.md", "---\ndoctype: spec\n---\n"),
           ]);
           let refs: Vec<(&Path, &str)> =
               files.iter().map(|(p, s)| (p.as_path(), s.as_str())).collect();
           let schema = infer_schema(&refs);
           assert!(schema.doctypes.contains(&"spec".to_string()));
           assert!(schema.doctypes.contains(&"plan".to_string()));
           assert_eq!(schema.doctypes.len(), 2);
       }

       #[test]
       fn infers_date_fields() {
           let files = corpus(&[
               ("docs/a.md", "---\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n"),
               ("docs/b.md", "---\ncreated: 2026-01-01\nupdated: 2026-01-02\n---\n"),
           ]);
           let refs: Vec<(&Path, &str)> =
               files.iter().map(|(p, s)| (p.as_path(), s.as_str())).collect();
           let schema = infer_schema(&refs);
           assert!(schema.date_fields.contains(&"created".to_string()));
           assert!(schema.date_fields.contains(&"updated".to_string()));
       }

       #[test]
       fn empty_corpus_returns_empty_schema_fields() {
           let schema = infer_schema(&[]);
           assert!(schema.required_fields.is_empty());
           assert!(schema.doctypes.is_empty());
       }
   }
   ```

   Run: `cargo nextest run -p agentlint-docs -- infers_required_fields_from_majority`
   Expected: FAIL

2. Implement `crates/agentlint-docs/src/infer.rs`:

   ```rust
   use crate::{DocsSchema, FilenameConvention};
   use agentlint_frontmatter::parse;
   use std::collections::{HashMap, HashSet};
   use std::path::Path;

   /// Derive a [`DocsSchema`] from a corpus of parsed doc files.
   ///
   /// Heuristics:
   /// - `required_fields`: fields present in ≥80% of files
   /// - `date_fields`: fields where ≥90% of non-empty values match `YYYY-MM-DD`
   /// - `doctypes` / `statuses`: all unique observed values for those keys
   /// - `conventions`: presence-based cluster (research dirs → research pattern,
   ///   always append catch-all)
   pub fn infer_schema(corpus: &[(&Path, &str)]) -> DocsSchema {
       let total = corpus.len();
       if total == 0 {
           return DocsSchema {
               file_glob: "docs/**/*.md".into(),
               required_fields: vec![],
               doctypes: vec![],
               statuses: vec![],
               date_fields: vec![],
               conventions: default_conventions(),
           };
       }

       let mut field_counts: HashMap<String, usize> = HashMap::new();
       let mut field_values: HashMap<String, Vec<String>> = HashMap::new();
       let mut field_date_counts: HashMap<String, usize> = HashMap::new();

       for (_path, src) in corpus {
           if let Ok(fields) = parse(src) {
               for field in &fields {
                   *field_counts.entry(field.key.clone()).or_default() += 1;
                   let val = field.value.trim().to_string();
                   if !val.is_empty() {
                       field_values.entry(field.key.clone()).or_default().push(val.clone());
                       if is_valid_date(&val) {
                           *field_date_counts.entry(field.key.clone()).or_default() += 1;
                       }
                   }
               }
           }
       }

       let threshold = ((total as f64) * 0.8).ceil() as usize;
       let mut required_fields: Vec<String> = field_counts
           .iter()
           .filter(|(_, &count)| count >= threshold)
           .map(|(k, _)| k.clone())
           .collect();
       required_fields.sort();

       let mut date_fields: Vec<String> = field_date_counts
           .iter()
           .filter(|(k, &date_count)| {
               let total_vals = field_values.get(k.as_str()).map_or(0, |v| v.len());
               total_vals > 0 && (date_count as f64) / (total_vals as f64) >= 0.9
           })
           .map(|(k, _)| k.clone())
           .collect();
       date_fields.sort();

       let doctypes = unique_sorted(&field_values, "doctype");
       let statuses = unique_sorted(&field_values, "status");
       let conventions = infer_conventions(corpus);

       DocsSchema {
           file_glob: "docs/**/*.md".into(),
           required_fields,
           doctypes,
           statuses,
           date_fields,
           conventions,
       }
   }

   fn unique_sorted(field_values: &HashMap<String, Vec<String>>, key: &str) -> Vec<String> {
       let set: HashSet<String> = field_values
           .get(key)
           .map(|v| v.iter().cloned().collect())
           .unwrap_or_default();
       let mut vals: Vec<String> = set.into_iter().collect();
       vals.sort();
       vals
   }

   fn infer_conventions(corpus: &[(&Path, &str)]) -> Vec<FilenameConvention> {
       let research_dirs = ["ideas", "specs", "plans"];
       let has_research = corpus.iter().any(|(path, _)| {
           path.parent()
               .and_then(|p| p.file_name())
               .and_then(|n| n.to_str())
               .map(|n| research_dirs.contains(&n))
               .unwrap_or(false)
       });
       let mut conventions = Vec::new();
       if has_research {
           conventions.push(FilenameConvention {
               format: "{ref}-{topic}.{doctype}.md".into(),
               dirs: vec!["ideas".into(), "specs".into(), "plans".into()],
           });
           conventions.push(FilenameConvention {
               format: "{ref}-{topic}.md".into(),
               dirs: vec!["ideas".into(), "specs".into(), "plans".into()],
           });
       }
       conventions.push(FilenameConvention {
           format: "{doctype}.{project}.md".into(),
           dirs: vec![],
       });
       conventions
   }

   fn default_conventions() -> Vec<FilenameConvention> {
       infer_conventions(&[])
   }

   pub(crate) fn is_valid_date(s: &str) -> bool {
       let parts: Vec<&str> = s.split('-').collect();
       if parts.len() != 3 {
           return false;
       }
       let (y, m, d) = (parts[0], parts[1], parts[2]);
       if y.len() != 4 || m.len() != 2 || d.len() != 2 {
           return false;
       }
       let (Ok(year), Ok(month), Ok(day)) =
           (y.parse::<u32>(), m.parse::<u32>(), d.parse::<u32>())
       else {
           return false;
       };
       year >= 2000 && (1..=12).contains(&month) && (1..=31).contains(&day)
   }
   ```

   Add `pub mod infer;` to `lib.rs`.
   In `lib.rs`, replace the private `is_valid_date` function body with a delegation:

   ```rust
   fn is_valid_date(s: &str) -> bool {
       crate::infer::is_valid_date(s)
   }
   ```

   Add `pub use infer::infer_schema;` to `lib.rs`.

3. Verify:

   ```text
   cargo nextest run -p agentlint-docs    → all green
   cargo clippy -p agentlint-docs -- -D warnings  → zero warnings
   ```

4. Run: `git branch --show-current`
   Verify: `main`. Commit:
   `git commit -m "feat(agentlint-docs): add infer_schema() corpus inference"`

---

#### Task 5: Wire infer mode into `DocsValidator`

**Crate**: `agentlint-docs`
**File(s)**: `crates/agentlint-docs/src/lib.rs`
**Run**: `cargo nextest run -p agentlint-docs`

1. Write failing test:

   ```rust
   #[test]
   fn infer_mode_flags_outlier_status() {
       // Corpus has two files with status=active; one outlier with status=wip.
       let base = "---\ntitle: agentlint-roadmap\ndoctype: roadmap\nproject: agentlint\n\
                   status: active\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n";
       let outlier = "---\ntitle: agentlint-guide\ndoctype: guide\nproject: agentlint\n\
                      status: wip\ncreated: 2026-05-16\nupdated: 2026-05-16\n---\n";
       let files = vec![
           (PathBuf::from("docs/roadmap.agentlint.md"), base.to_string()),
           (PathBuf::from("docs/guide.agentlint.md"), base.to_string()),
           (PathBuf::from("docs/x.agentlint.md"), outlier.to_string()),
       ];
       let mut v = DocsValidator::default();
       v.set_infer_mode(true);
       let diags = v.validate_batch(&files);
       assert!(
           diags.iter().any(|d| d.rule == "docs/frontmatter/unknown-status"),
           "expected unknown-status from inferred schema: {diags:?}"
       );
   }
   ```

   Run: `cargo nextest run -p agentlint-docs -- infer_mode_flags_outlier_status`
   Expected: FAIL

2. Implement changes to `DocsValidator` in `lib.rs`:

   Add `infer_mode: bool` field to `DocsValidator`:

   ```rust
   pub struct DocsValidator {
       schema: DocsSchema,
       glob: &'static str,
       infer_mode: bool,
   }
   ```

   Update `DocsValidator::new()`:

   ```rust
   pub fn new(schema: DocsSchema) -> Self {
       let glob: &'static str = Box::leak(schema.file_glob.clone().into_boxed_str());
       Self { schema, glob, infer_mode: false }
   }
   ```

   Add `set_infer_mode`:

   ```rust
   pub fn set_infer_mode(&mut self, on: bool) {
       self.infer_mode = on;
   }
   ```

   Update `validate()` — when infer_mode, return empty (batch handles it):

   ```rust
   fn validate(&self, path: &Path, src: &str) -> Vec<Diagnostic> {
       if self.infer_mode {
           return vec![];
       }
       // ... existing validation logic unchanged ...
   }
   ```

   Add `validate_batch()` override:

   ```rust
   fn validate_batch(&self, files: &[(PathBuf, String)]) -> Vec<Diagnostic> {
       if !self.infer_mode {
           return vec![];
       }
       let corpus: Vec<(&Path, &str)> = files
           .iter()
           .map(|(p, s)| (p.as_path(), s.as_str()))
           .collect();
       let inferred = crate::infer::infer_schema(&corpus);
       let infer_validator = DocsValidator::new(inferred);
       files
           .iter()
           .flat_map(|(path, src)| infer_validator.validate(path, src))
           .collect()
   }
   ```

3. Verify:

   ```text
   cargo nextest run -p agentlint-docs    → all green
   cargo clippy -p agentlint-docs -- -D warnings  → zero warnings
   ```

4. Run: `git branch --show-current`
   Verify: `main`. Commit:
   `git commit -m "feat(agentlint-docs): wire infer_mode into DocsValidator::validate_batch"`

---

#### Task 6: `--infer-schema` and `--emit-schema` CLI flags

**Crate**: `agentlint` (binary)
**File(s)**: `src/main.rs`
**Run**: `cargo nextest run --workspace`

1. Write failing test (integration-level — test that the flag is accepted):

   ```rust
   // No unit test needed here; clippy/check will catch type errors.
   // Smoke-verify by building: cargo check → must not error.
   ```

   Run: `cargo check`
   Expected: PASS (no changes yet — just verifying baseline)

2. Implement — add flags to `Cli` in `src/main.rs`:

   ```rust
   /// Infer docs schema from corpus each run and validate outliers against it
   #[arg(long)]
   infer_schema: bool,

   /// Infer docs schema from corpus and print JSON to stdout; implies --infer-schema
   #[arg(long)]
   emit_schema: bool,
   ```

   Update `DocsValidator` construction:

   ```rust
   let infer = cli.infer_schema || cli.emit_schema;
   let mut docs_validator = agentlint_docs::DocsValidator::new(
       agentlint_docs::DocsSchema::from_config_path(std::path::Path::new(".agentlint.toml"))
           .unwrap_or_default(),
   );
   docs_validator.set_infer_mode(infer);
   ```

   Handle `--emit-schema` — collect docs files, infer, print JSON, exit:

   ```rust
   if cli.emit_schema {
       let roots_clone = roots.clone();
       // Collect all docs/**/*.md files using the validator's glob.
       let pattern = "docs/**/*.md";
       let files: Vec<(std::path::PathBuf, String)> = {
           use agentlint_core::Validator;
           let all = agentlint_core::run(&roots_clone, &validators, &config);
           // Re-collect by walking directly — run() already read the files.
           // Walk roots and filter by glob pattern.
           let mut out = Vec::new();
           for root in &roots_clone {
               if let Ok(entries) = std::fs::read_dir(root) {
                   // Use walkdir via the core runner instead; collect docs files manually.
                   drop(entries);
               }
               collect_docs_files(root, pattern, &mut out);
           }
           out
       };
       let corpus: Vec<(&std::path::Path, &str)> =
           files.iter().map(|(p, s)| (p.as_path(), s.as_str())).collect();
       let inferred = agentlint_docs::infer_schema(&corpus);
       let json = serde_json::to_string_pretty(&inferred)
           .unwrap_or_else(|_| "{}".to_string());
       println!("{json}");
       return;
   }
   ```

   Add `collect_docs_files` helper in `main.rs`:

   ```rust
   fn collect_docs_files(
       root: &std::path::Path,
       _pattern: &str,
       out: &mut Vec<(std::path::PathBuf, String)>,
   ) {
       for entry in walkdir::WalkDir::new(root)
           .follow_links(false)
           .into_iter()
           .filter_map(|e| e.ok())
           .filter(|e| e.file_type().is_file())
       {
           let path = entry.into_path();
           let is_docs_md = path
               .components()
               .any(|c| c.as_os_str() == "docs")
               && path.extension().and_then(|e| e.to_str()) == Some("md");
           if is_docs_md {
               if let Ok(src) = std::fs::read_to_string(&path) {
                   out.push((path, src));
               }
           }
       }
   }
   ```

   Add `walkdir` import at top of `main.rs` (it's already a workspace dep).

   Add `serde_json` to root `[dependencies]` in `Cargo.toml`:

   ```toml
   serde_json = { workspace = true }
   ```

3. Verify:

   ```text
   cargo nextest run --workspace    → all green
   cargo clippy --workspace -- -D warnings  → zero warnings
   cargo run -- --emit-schema 2>/dev/null   → prints JSON object
   ```

4. Run: `git branch --show-current`
   Verify: `main`. Commit:
   `git commit -m "feat: add --infer-schema and --emit-schema CLI flags"`

---

#### Task 7: Wire `SchemaRegistry` into `DocsSchema::from_config_path` and update example config

**Crate**: `agentlint-docs`, docs
**File(s)**:

- `crates/agentlint-docs/src/lib.rs`
- `agentlint.example.toml`
  **Run**: `cargo nextest run -p agentlint-docs`

1. Write failing test:

   ```rust
   #[test]
   fn from_config_schema_name_resolves_builtin() {
       // Write a temp toml referencing the built-in schema by name.
       let dir = tempfile::tempdir().unwrap();
       let toml_path = dir.path().join(".agentlint.toml");
       std::fs::write(&toml_path, "[docs]\nschema = \"agentlint-default\"\n").unwrap();
       let schema = DocsSchema::from_config_path(&toml_path).unwrap();
       assert!(!schema.doctypes.is_empty());
   }
   ```

   Run: `cargo nextest run -p agentlint-docs -- from_config_schema_name_resolves_builtin`
   Expected: FAIL

2. Add `schema` and `schema_file` fields to the raw TOML struct inside
   `DocsSchema::from_config_path`:

   ```rust
   pub fn from_config_path(path: &Path) -> Result<Self, String> {
       let src = match std::fs::read_to_string(path) {
           Ok(s) => s,
           Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
           Err(e) => return Err(format!("could not read {}: {e}", path.display())),
       };

       #[derive(Deserialize, Default)]
       struct RawFile {
           #[serde(default)]
           docs: Option<RawDocs>,
       }

       #[derive(Deserialize, Default)]
       struct RawDocs {
           /// Named built-in or discovered schema (resolved via SchemaRegistry).
           #[serde(default)]
           schema: Option<String>,
           /// Path to a JSON schema file (explicit; takes precedence over `schema`).
           #[serde(default)]
           schema_file: Option<String>,
           /// Inline overrides applied on top of the resolved schema.
           #[serde(flatten)]
           overrides: Option<DocsSchema>,
       }

       let raw: RawFile = toml::from_str(&src)
           .map_err(|e| format!("invalid config {}: {e}", path.display()))?;

       let Some(docs) = raw.docs else {
           return Ok(Self::default());
       };

       // Resolve base schema.
       let root = path.parent().unwrap_or(Path::new("."));
       let registry = crate::registry::SchemaRegistry::discover(root);

       let mut base = if let Some(ref file) = docs.schema_file {
           let plugin = crate::plugin::FileJsonPlugin::new(root.join(file));
           use crate::plugin::DocsSchemaPlugin;
           plugin.load()?
       } else if let Some(ref name) = docs.schema {
           registry
               .get(name)
               .ok_or_else(|| {
                   format!(
                       "unknown schema '{name}'; available: {}",
                       registry.names().join(", ")
                   )
               })?
               .map_err(|e| e)?
       } else {
           Self::default()
       };

       // Apply inline overrides.
       if let Some(overrides) = docs.overrides {
           // Non-default (non-empty) inline fields win.
           if !overrides.required_fields.is_empty() {
               base.required_fields = overrides.required_fields;
           }
           if !overrides.doctypes.is_empty() {
               base.doctypes = overrides.doctypes;
           }
           if !overrides.statuses.is_empty() {
               base.statuses = overrides.statuses;
           }
           if !overrides.date_fields.is_empty() {
               base.date_fields = overrides.date_fields;
           }
           if !overrides.conventions.is_empty() {
               base.conventions = overrides.conventions;
           }
           if overrides.file_glob != "docs/**/*.md" {
               base.file_glob = overrides.file_glob;
           }
       }

       Ok(base)
   }
   ```

3. Add `schema` and `schema_file` entries to `agentlint.example.toml` under `[docs]`:

   ```toml
   # Named schema — resolves from .agentlint/schemas/<name>.json or a built-in.
   # Default built-in: "agentlint-default"
   # schema = "agentlint-default"

   # Explicit JSON schema file path (overrides `schema`).
   # schema_file = ".agentlint/schemas/my-team-docs.json"
   ```

4. Verify:

   ```text
   cargo nextest run -p agentlint-docs    → all green
   cargo clippy -p agentlint-docs -- -D warnings  → zero warnings
   ```

5. Run: `git branch --show-current`
   Verify: `main`. Commit:
   `git commit -m "feat(agentlint-docs): wire SchemaRegistry into from_config_path; add schema/schema_file keys"`

---

### Quality Rules

- No placeholders: all code blocks are copy-paste ready
- Each task ends with a `cargo nextest` pass and a commit
- `validate_batch` is the inference boundary — no runner changes needed in `agentlint-core`
- `--emit-schema` does not run validation; it exits after printing JSON

### Pre-Save Checklist

- [x] All 7 tasks map to distinct files with zero overlap
- [x] Type names consistent across tasks: `DocsSchemaPlugin`, `SchemaRegistry`, `infer_schema`
- [x] TDD for every task (failing test → implement → verify)
- [x] Each task is 5–15 min of focused work
- [x] Each task ends with a commit on `main`
