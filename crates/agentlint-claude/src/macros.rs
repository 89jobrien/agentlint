//! `frontmatter_validator!` — implement `validate` for frontmatter-based agent files.
//!
//! Generates an inherent `validate` associated function on the named type that
//! parses YAML frontmatter and checks all `required` fields are present and
//! non-empty. The actual parsing logic lives in [`crate::frontmatter`].
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
