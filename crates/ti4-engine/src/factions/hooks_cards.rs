//! Faction hooks for hidden hands and card windows (`choice.rs`, `reactions.rs`, `action_cards.rs`): viewing hands, free-action discards, copying cards.
//!
//! Owned by one wave-B package at a time (`plans/BASE_FACTIONS_PLAN_2026-10-02.md`). Each
//! field is optional and called for every module; dispatch functions live here beside the
//! fields, iterate [`super::MODULES`] in order, and are what the shared engine calls. The rules
//! on `super::Hooks` (atomicity, ordering, ownership) apply. Tests install hooks with a
//! `#[cfg(test)] with_test_hooks(hooks, || ..)` that restores on panic, as in `hooks_combat.rs`.

/// Hooks for this area; empty until a package adds one.
#[derive(Debug, Clone, Copy)]
pub struct CardHooks {}

impl CardHooks {
    /// No hooks.
    pub const NONE: Self = Self {};
}
