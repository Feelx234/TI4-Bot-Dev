//! The faction packages of wave C on the string engine's side (docs/ENGINE_CORE_FACTIONS_PLAN.md 4): one file per package
//! with the hook-level dual of its factions. `dual_hook!` at the string call site compares the string faction function
//! with the same function of `ti4-engine-core-factions` over a typed view of the position.

#[cfg(feature = "typed-core")]
pub mod cb;

/// `dual_hook!` when the typed core is compiled, the string expression alone otherwise. `$typed` is an `Option`: `None`
/// (an id the tables do not know, a value the typed state cannot hold) keeps the string answer, as `dual_typed!` does.
#[macro_export]
macro_rules! cb_dual {
    ($group:expr, $slot:expr, $string:expr, $typed:expr) => {{
        #[cfg(feature = "typed-core")]
        let answer = $crate::dual_hook!($group, $slot, $string, ($typed).unwrap_or_else(|| $string));
        #[cfg(not(feature = "typed-core"))]
        let answer = $string;
        answer
    }};
}
