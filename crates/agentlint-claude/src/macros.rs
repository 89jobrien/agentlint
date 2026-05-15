//! `frontmatter_validator!` — implement `validate` for frontmatter-based agent files.
//!
//! Generates an inherent `validate` associated function on the named type that
//! parses YAML frontmatter and checks all `required` fields are present and
//! non-empty. The actual parsing logic lives in [`crate::frontmatter`].
//!
//! # Design note — inherent fn, not `Validator` trait
//!
//! The generated method is `impl Type { pub fn validate(...) }` with **no `&self`
//! receiver**. This means the generated type does **not** implement the
//! [`agentlint_core::Validator`] trait and cannot be registered as
//! `Box<dyn Validator>` directly.
//!
//! The intended dispatch path is:
//! 1. `ClaudeValidator` implements `Validator` and is registered in `main.rs`.
//! 2. `ClaudeValidator::validate` classifies the path and delegates to the
//!    appropriate inherent `*Validator::validate` function.
//!
//! If a sub-validator ever needs direct `Box<dyn Validator>` registration, add a
//! `&self` receiver to the generated method and a `impl Validator for $t` block.
//!
//! # Examples
//!
//! ```rust,ignore
//! pub struct AgentsValidator;
//! frontmatter_validator!(AgentsValidator, required: ["name", "description"]);
//!
//! // Expands to:
//! impl AgentsValidator {
//!     pub fn validate(path: &Path, src: &str) -> Vec<Diagnostic> {
//!         crate::frontmatter::check_required(path, src, &["name", "description"])
//!     }
//! }
//! ```

macro_rules! frontmatter_validator {
    ($t:ty, required: [$($field:literal),+ $(,)?]) => {
        impl $t {
            pub fn validate(
                path: &std::path::Path,
                src: &str,
            ) -> Vec<agentlint_core::Diagnostic> {
                crate::frontmatter::check_required(path, src, &[$($field),+])
            }
        }
    };
}

pub(crate) use frontmatter_validator;
