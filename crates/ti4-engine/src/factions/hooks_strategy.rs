//! Faction hooks for strategy cards, initiative and unit forms (`strategy.rs`, `strategy_cards.rs`, `supply.rs`, `production.rs`, `fleet.rs`): initiative order, strategy-card swaps, capture, mobile docks, dual-form units.
//!
//! Owned by one wave-B package at a time (`plans/BASE_FACTIONS_PLAN_2026-10-02.md`). Each
//! field is optional and called for every module; dispatch functions live here beside the
//! fields, iterate [`super::MODULES`] in order, and are what the shared engine calls. The rules
//! on `super::Hooks` (atomicity, ordering, ownership) apply. Tests install hooks with a
//! `#[cfg(test)] with_test_hooks(hooks, || ..)` that restores on panic, as in `hooks_combat.rs`.

/// Hooks for this area; empty until a package adds one.
#[derive(Debug, Clone, Copy)]
pub struct StrategyHooks {}

impl StrategyHooks {
    /// No hooks.
    pub const NONE: Self = Self {};
}
