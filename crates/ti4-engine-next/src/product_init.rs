//! `product_init()`: the one call a product binary makes before it plays a game (package P1 of
//! `docs/todo/D2-8-wrapper.md`, section 5.1; owner decision 2026-11-03).
//!
//! The library defaults stay **off**: the dev tools (training, review, self-play with the wide
//! roster, the unit tests with the fixture faction) keep running tables the product never creates,
//! and every such position stays a counted bail with its reason. The product binaries (the server,
//! the wasm module) call this once, which turns on
//!
//! * the product assertions ([`product::set_strict`], the positions the product never reaches),
//! * the strict refusal of the codecs ([`core_sync::unmapped::set_strict`]: a value the typed core
//!   cannot hold is a defect, not a quiet fallback), and
//! * the panic policy for switched-off features
//!   ([`core_sync::unsupported::Policy::Panic`], docs/DISABLE_XF123.md).
//!
//! Two rules go with it. A *known* disabled feature (an older rules version, Thunder's Edge
//! content, a dropped faction, structured diplomacy, the deck reserve) must never reach a panic in
//! production: `ti4_host::disabled::check_playable` refuses it with an `Err` at creation, load,
//! recovery, replay and resume, and the panic is only for positions that cannot arise. On wasm
//! (`panic = "abort"`) a panic ends the page, so every refusable value has to be an `Err` first.
//!
//! `TI4_CORE_STRICT=1` stays the spelling for the gates and the nightly sweep. Tests use the
//! thread-local forms (`product::with_strict`, `core_sync::unsupported::with_disabled_policy`),
//! never this function: it sets process-wide switches.

use crate::{core_sync, product};

/// Turns the strict product policy on for this process. Call it once, from the `main` of a product
/// binary (or the wasm entry), never from library code.
pub fn product_init() {
    product::set_strict(true);
    core_sync::unmapped::set_strict(true);
    core_sync::unsupported::set_disabled_policy(core_sync::unsupported::Policy::Panic);
    #[cfg(feature = "lend-string")]
    crate::lend_string::product_init_legacy();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The switches are process-wide, so the test runs in a child process of its own.
    #[test]
    fn product_init_turns_the_three_switches_on_and_the_library_defaults_stay_off() {
        const CHILD: &str = "TI4_PRODUCT_INIT_CHILD";
        if std::env::var_os(CHILD).is_some() {
            assert!(!product::strict());
            assert_eq!(
                core_sync::unsupported::disabled_policy(),
                core_sync::unsupported::Policy::Off
            );
            product_init();
            assert!(product::strict());
            assert_eq!(
                core_sync::unsupported::disabled_policy(),
                core_sync::unsupported::Policy::Panic
            );
            return;
        }
        let exe = std::env::current_exe().expect("the test binary");
        let status = std::process::Command::new(exe)
            .args([
                "--exact",
                "product_init::tests::product_init_turns_the_three_switches_on_and_the_library_defaults_stay_off",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env_remove("TI4_CORE_STRICT")
            .env_remove("TI4_UNMAPPED")
            .status()
            .expect("the child runs");
        assert!(status.success(), "the child process asserted: {status}");
    }
}
