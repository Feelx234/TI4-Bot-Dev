//! The offline duals of the ported faction packages (docs/ENGINE_CORE_FACTIONS_PLAN.md 4): one file per package, each
//! owned by its package. `PORTED` is the union of their ported-module lists.
//!
//! The faction packages of wave C on the string engine's side: one file per package with the hook-level dual of its
//! factions. `dual_hook!` at the string call site compares the string faction function with the same function of
//! `ti4-engine-core-factions` over a typed view of the position.

#[cfg(feature = "typed-core")]
pub mod ca;
#[cfg(feature = "typed-core")]
pub mod cb;
#[cfg(feature = "typed-core")]
pub mod cc;
#[cfg(feature = "typed-core")]
pub mod cd;
#[cfg(feature = "typed-core")]
pub mod ce;

/// `dual_hook!` when the typed core is compiled, the string expression alone otherwise. `$typed` is an `Option`: `None`
/// (an id the tables do not know, a value the typed state cannot hold) keeps the string answer, as `dual_typed!` does.
#[macro_export]
macro_rules! cb_dual {
    ($group:expr, $slot:expr, $string:expr, $typed:expr) => {{
        #[cfg(feature = "typed-core")]
        let answer = if $crate::dual::backend($crate::dual::Area::Factions) == $crate::dual::Backend::Dual {
            $crate::dual_hook!($group, $slot, $string, ($typed).unwrap_or_else(|| $string))
        } else {
            $string
        };
        #[cfg(not(feature = "typed-core"))]
        let answer = $string;
        answer
    }};
}
