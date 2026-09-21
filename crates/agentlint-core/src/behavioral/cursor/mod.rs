//! Cursor behavioral validators.

pub mod rules;
pub use rules::CursorValidator;

use super::BehavioralFn;
use std::collections::HashMap;

pub fn register(m: &mut HashMap<&'static str, BehavioralFn>) {
    let _ = m;
}
