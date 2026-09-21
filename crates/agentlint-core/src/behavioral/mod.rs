//! Behavioral validation functions — complex rules not expressible in TOML.
//!
//! Each harness has its own sub-module. Functions are registered in a global
//! registry keyed by name, and called from the declarative engine via
//! `check = "custom"` + `custom_fn = "name"`.

pub mod claude;
pub mod cursor;
pub mod looprs;

use crate::Diagnostic;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Frontmatter key-value pair as produced by `ParsedContent::Fields`.
pub type FieldPair = (String, String);

/// Signature for behavioral validation functions.
pub type BehavioralFn = fn(&Path, &str, &[FieldPair]) -> Vec<Diagnostic>;

/// Get the global behavioral function registry.
pub fn registry() -> &'static HashMap<&'static str, BehavioralFn> {
    static REG: OnceLock<HashMap<&'static str, BehavioralFn>> = OnceLock::new();
    REG.get_or_init(|| {
        let mut m = HashMap::new();
        claude::register(&mut m);
        cursor::register(&mut m);
        looprs::register(&mut m);
        m
    })
}
