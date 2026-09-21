use std::path::Path;
use std::{env, fs};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let plugins_dir = Path::new(&manifest_dir).join("../../plugins");
    let out_dir = env::var("OUT_DIR").unwrap();

    println!("cargo::rerun-if-changed={}", plugins_dir.display());

    let mut entries: Vec<_> = fs::read_dir(&plugins_dir)
        .expect("plugins/ directory must exist")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "toml")
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut code = String::from("static BUILTIN_PLUGINS: &[(&str, &str)] = &[\n");
    for entry in &entries {
        let path = entry.path().canonicalize().unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let name = stem.strip_prefix("agentlint.").unwrap_or(stem);
        code.push_str(&format!(
            "    (\"{name}\", include_str!(\"{}\")),\n",
            path.display()
        ));
        println!("cargo::rerun-if-changed={}", path.display());
    }
    code.push_str("];\n");

    fs::write(Path::new(&out_dir).join("builtin_plugins.rs"), code).unwrap();
}
