//! The dev-tool surface (`lend-string` feature, off by default and never on in a production build: `lib.rs` stops the build when both
//! `lend-string` and `production` are on). Everything that names `ti4-engine-legacy` outside `#[cfg(test)]` code lives here
//! (`tools/engine_core/check_layers.py thin`). No lend code: `reverse_lend`, `lend_table` and `BAILS` stay in the legacy crate.

/// The legacy crate itself, for dev tools that read legacy positions (`ti4_engine::lend_string::legacy::...`).
pub use ti4_engine_legacy as legacy;

/// A wire value of the legacy crate read as the same value of this one (a `Choice`, a `DecisionRecord`, ...): one serde shape,
/// `tests/wire_parity.rs`. For tests and oracles that still drive a string window; never for the game loop.
///
/// # Panics
/// Never for the wire types.
pub fn from_legacy_wire<A: serde::Serialize, B: serde::de::DeserializeOwned>(a: &A) -> B {
    serde_json::from_value(serde_json::to_value(a).expect("a wire value serialises")).expect("this crate reads the same wire value")
}

/// The legacy crate's own product switches, set next to the native ones by [`crate::product_init`] (a dev tool that calls it and still
/// plays legacy games gets the same policy on both sides).
pub fn product_init_legacy() {
    legacy::product::set_strict(true);
    legacy::core_sync::unmapped::set_strict(true);
    legacy::core_sync::unsupported::set_disabled_policy(legacy::core_sync::unsupported::Policy::Panic);
}
