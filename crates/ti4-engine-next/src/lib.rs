//! The product-facing engine crate. Facade stage: everything is re-exported from `ti4-engine-legacy`
//! (the directory `crates/ti4-engine`, docs/todo/D2-7-legacy-split.md).
//!
//! One section per lane of D2-8 (docs/todo/D2-8-wrapper.md 6.2); a lane touches only its own.

// ---- lane E (product edges, docs/D28E_RESUME.md) ----
mod product_init;
pub use product_init::product_init;

// ---- the facade ----
pub use ti4_engine_legacy::*;
