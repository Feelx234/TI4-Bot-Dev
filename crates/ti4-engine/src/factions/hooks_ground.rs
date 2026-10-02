//! Faction hooks for invasion and ground combat (`invasion.rs`): bombardment, commitment, ground rounds, sustain, custodians.
//!
//! Owned by one wave-B package at a time (`plans/BASE_FACTIONS_PLAN_2026-10-02.md`). Each
//! field is optional and called for every module; dispatch functions live here beside the
//! fields, iterate [`super::MODULES`] in order, and are what the shared engine calls. The rules
//! on `super::Hooks` (atomicity, ordering, ownership) apply.

/// Hooks for this area; empty until a package adds one.
#[derive(Debug, Clone, Copy)]
pub struct GroundHooks {}

impl GroundHooks {
    /// No hooks.
    pub const NONE: Self = Self {};
}
