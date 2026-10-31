//! The hook table (docs/ENGINE_CORE_STATE.md 4.1 cut 1, risk R7; package A5).
//!
//! Faction modules (layer L6, `ti4-engine-core-factions`) are not part of the core's cycle: the rules
//! reach them only through the typed [`HookTable`] defined here. The table is the typed port of the
//! string engine's `factions::Hooks` (99 slots in seven groups, `factions/hooks_*.rs`) **plus** the
//! direct faction calls the shared modules make today (`keleres::offer_agent`, `crimson::abilities_lost`,
//! `bastion::extra_die`, ... about 90 distinct functions, see [`direct`]), which become named hook points
//! instead of imports.
//!
//! # How a module registers
//!
//! ```ignore
//! pub fn register(t: &mut HookTable) {
//!     t.combat_modifier.push(my_modifier);           // a slot is a `Vec<fn(..)>`, filled in MODULES order
//!     t.combat.may_sustain.push(my_sustain);
//! }
//! // game setup: let mut t = HookTable::new(); for m in MODULES { m(&mut t); }
//! ```
//!
//! A hook is called for **every** seat and every module, not only for the seat playing that faction (a
//! promissory note or a copied ability belongs to whoever holds the card); each hook checks its own
//! condition and returns the neutral value otherwise. Modules are visited in registration order, which
//! is the string engine's `MODULES` order and fixes the dice stream. A hook that reports "not
//! performed" must leave the state as it found it: callers do not roll back.
//!
//! # How a slot is combined
//!
//! Every slot has a combining kind (the macro keyword), which reproduces the dispatch function of the
//! string engine: `sum`, `any`, `all`, `fold` (the accumulator is the last argument, threaded through
//! the modules in order), `first` (first `Some`), `each`, `try_each`, `try_plus`, `try_first`, `concat`,
//! `concat_sorted` (concatenate, sort, drop duplicates), `call` (the single provider, or the default).
//! The dispatch method has the slot's name: `kernel.hooks.combat_modifier(s, k, who, ctx)`.
//!
//! # Where the inventory comes from
//!
//! Every slot lists its call sites (module and line count of callers at `201f6714`) in the doc line.
//! `tools/engine_core/hook_inventory.py` regenerates the list from the string engine and fails when a
//! call to `factions::` is not accounted for by a slot.

pub mod cards;
pub mod combat;
pub mod direct;
pub mod economy;
pub mod ground;
pub mod movement;
pub mod components;
pub mod sites;
pub mod strategy;

pub use sites::*;

use crate::generated::{Leader, StrategyCard, StrategyCardKind, Technology};
use crate::ids::{PlanetIx, Seat, SystemIx};
use crate::kernel::{Cx, Kernel, TimingError};
use crate::state::CoreState;

/// Declares a group of hook slots: the struct of `Vec<fn>`, its `new`, and one dispatch method per slot
/// that combines the registered functions the way the string engine's dispatch does.
macro_rules! hook_group {
    (
        $(#[$gmeta:meta])*
        $group:ident {
            $(
                $(#[$meta:meta])*
                $kind:ident $name:ident ( $($arg:ident : $ty:ty),* $(; $acc:ident : $accty:ty)? ) -> $ret:ty ;
            )*
        }
    ) => {
        $(#[$gmeta])*
        #[derive(Clone, Default)]
        pub struct $group {
            $(
                $(#[$meta])*
                pub $name: Vec<fn($($ty,)* $($accty)?) -> $ret>,
            )*
        }

        impl $group {
            /// Number of slots in this group.
            pub const SLOTS: usize = [$(stringify!($name)),*].len();

            /// Number of registered functions across the group's slots.
            #[must_use]
            pub fn registered(&self) -> usize {
                0 $(+ self.$name.len())*
            }

            /// Reorder the functions of every *commutative* slot (a pure `sum`, `any`, `all` or
            /// `concat_sorted`); the order audit of docs/ENGINE_DISENTANGLE.md 5.3. Slots whose order
            /// is part of the result (`fold`, `first`, `concat`, anything that mutates) are left alone.
            #[allow(unused_variables)]
            pub fn permute_commutative(&mut self, p: $crate::kernel::seam::Permute) {
                $( hook_group!(@permute $kind, self.$name, p, ($($arg),*)); )*
            }

            $(
                $(#[$meta])*
                #[allow(unused_variables, unused_mut, clippy::redundant_closure_call)]
                pub fn $name(&self, $($arg: $ty,)* $($acc: $accty)?) -> $ret {
                    hook_group!(@dispatch $kind, self.$name, ($($arg),*) $(; $acc)? -> $ret)
                }
            )*
        }
    };

    (@permute sum, $slot:expr, $p:ident, (cx $(, $rest:ident)*)) => {};
    (@permute any, $slot:expr, $p:ident, (cx $(, $rest:ident)*)) => {};
    (@permute all, $slot:expr, $p:ident, (cx $(, $rest:ident)*)) => {};
    (@permute concat_sorted, $slot:expr, $p:ident, (cx $(, $rest:ident)*)) => {};
    (@permute sum, $slot:expr, $p:ident, ($($arg:ident),*)) => { $p.apply(&mut $slot) };
    (@permute any, $slot:expr, $p:ident, ($($arg:ident),*)) => { $p.apply(&mut $slot) };
    (@permute all, $slot:expr, $p:ident, ($($arg:ident),*)) => { $p.apply(&mut $slot) };
    (@permute concat_sorted, $slot:expr, $p:ident, ($($arg:ident),*)) => { $p.apply(&mut $slot) };
    (@permute $other:ident, $slot:expr, $p:ident, ($($arg:ident),*)) => {};

    (@dispatch sum, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {
        $slot.iter().map(|f| f($($arg),*)).sum()
    };
    (@dispatch any, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {
        $slot.iter().any(|f| f($($arg),*))
    };
    (@dispatch all, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {
        $slot.iter().all(|f| f($($arg),*))
    };
    (@dispatch any_some_true, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {
        if $slot.iter().any(|f| f($($arg),*) == Some(true)) { Some(true) } else { None }
    };
    (@dispatch first, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {
        $slot.iter().find_map(|f| f($($arg),*))
    };
    (@dispatch fold, $slot:expr, ($($arg:ident),*) ; $acc:ident -> $ret:ty) => {{
        let mut $acc = $acc;
        for f in &$slot {
            $acc = f($($arg,)* $acc);
        }
        $acc
    }};
    (@dispatch each, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {{
        for f in &$slot {
            f($($arg),*);
        }
    }};
    (@dispatch try_each, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {{
        for f in &$slot {
            f($($arg),*)?;
        }
        Ok(())
    }};
    (@dispatch try_plus, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {{
        let mut total = Default::default();
        for f in &$slot {
            total = $crate::kernel::hooks::Plus::plus(total, f($($arg),*)?);
        }
        Ok(total)
    }};
    (@dispatch try_first, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {{
        for f in &$slot {
            if let Some(found) = f($($arg),*)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }};
    (@dispatch concat, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {{
        let mut out = <$ret>::new();
        for f in &$slot {
            for x in f($($arg),*).iter() {
                out.push(*x);
            }
        }
        out
    }};
    (@dispatch concat_sorted, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {{
        let mut out: Vec<_> = Vec::new();
        for f in &$slot {
            for x in f($($arg),*).iter() {
                out.push(*x);
            }
        }
        out.sort();
        out.dedup();
        out.into_iter().collect::<$ret>()
    }};
    (@dispatch call, $slot:expr, ($($arg:ident),*) -> $ret:ty) => {
        match $slot.first() {
            Some(f) => f($($arg),*),
            None => Default::default(),
        }
    };
}
pub(crate) use hook_group;

/// Accumulation of a result across modules (`ProducedHits::plus`).
pub trait Plus {
    #[must_use]
    fn plus(self, other: Self) -> Self;
}

/// The unwind-free result of an asking hook: the hook asked a question and the answer was refused.
pub type Hr<T> = Result<T, TimingError>;

hook_group! {
    /// The general slots (`factions::Hooks` fields that are not in a sub-group): value hooks, leaders,
    /// component actions, strategy-card follow-ups and timing abilities.
    GeneralHooks {
        /// Shift to each combat die (all factions, summed). String: `factions::combat_modifier`, 3 callers
        /// (`combat`, `invasion`, `fleet_strength`).
        sum combat_modifier(s: &CoreState, k: &Kernel, who: Seat, ctx: CombatCtx) -> i64;
        /// Adjust the fleet-supply limit (fold). String: `supply`.
        fold fleet_supply(s: &CoreState, k: &Kernel, who: Seat; limit: i32) -> i32;
        /// Adjust command tokens gained in the status phase (fold). String: `status`.
        fold status_tokens(s: &CoreState, k: &Kernel, who: Seat; count: i32) -> i32;
        /// Prerequisites waived when researching `tech` (summed). String: `technology`.
        sum waived_prerequisites(s: &CoreState, k: &Kernel, who: Seat, tech: Technology) -> usize;
        /// Whether this strategy card's secondary is resolved as its primary. String: `strategy_cards`.
        any substitutes_primary(s: &CoreState, k: &Kernel, who: Seat, card: Option<StrategyCardKind>) -> bool;
        /// Whether this strategy card's secondary costs no token. String: `strategy_cards`, `strategy`.
        any secondary_is_free(s: &CoreState, k: &Kernel, who: Seat, card: Option<StrategyCardKind>) -> bool;
        /// Whether this player may trade action cards. String: `transactions`.
        any trades_action_cards(s: &CoreState, k: &Kernel, who: Seat) -> bool;
        /// Whether this player may transact with non-neighbours. String: `transactions`.
        any ignores_neighbours(s: &CoreState, k: &Kernel, who: Seat) -> bool;
        /// After this player gains control of a planet. Runs BEFORE the shared faction code. String: `planets`,
        /// `invasion`, `action_cards`, `legendary` (control changes).
        each control_gained(cx: &mut Cx<'_>, who: Seat, system: SystemIx, planet: PlanetIx) -> ();
        /// Component actions offered on this player's turn. String: `game` (component options).
        concat component_actions(s: &CoreState, k: &Kernel, who: Seat) -> ComponentList;
        /// Map-dependent component actions, generated only when their destinations exist.
        concat mapped_component_actions(s: &CoreState, k: &Kernel, who: Seat) -> ComponentList;
        /// Perform a component action; `true` if this module claimed it (modules claim first, in order).
        any perform_component(cx: &mut Cx<'_>, who: Seat, action: ComponentAction) -> bool;
        /// After a strategy card (primary or secondary) resolves for this player. Runs BEFORE shared code.
        each strategy_resolved(cx: &mut Cx<'_>, who: Seat, card: Option<StrategyCardKind>) -> ();
        /// Extra hits at the end of a ground-combat round in `system` (summed; module dice are rolled
        /// before Harrow's, so the order is the dice stream).
        sum ground_combat_round_ended(cx: &mut Cx<'_>, who: Seat, system: SystemIx) -> usize;
        /// At the start of a space-combat round, for each participant. Runs BEFORE shared code.
        each space_combat_round_started(cx: &mut Cx<'_>, who: Seat) -> ();
        /// Whether a commander's unlock condition is met; `None` if not this module's commander.
        first commander_unlocked(s: &CoreState, k: &Kernel, who: Seat, leader: Leader) -> Option<bool>;
        /// For an action-phase leader this module delivers: whether it can resolve now.
        first leader_action(s: &CoreState, k: &Kernel, who: Seat, leader: Leader) -> Option<bool>;
        /// Use a leader; `None` if not this module's leader, else whether it resolved.
        first use_leader(cx: &mut Cx<'_>, who: Seat, leader: Leader) -> Option<bool>;
        /// Action leaders whose effects open nested timing windows.
        try_first use_leader_timed(cx: &mut Cx<'_>, r: &mut crate::kernel::Resolver, who: Seat, leader: Leader) -> Hr<Option<bool>>;
        /// A leader that performs a strategy primary and continues through driver windows.
        try_first use_leader_strategy_primary(cx: &mut Cx<'_>, who: Seat, leader: Leader) -> Hr<Option<LeaderPrimary>>;
        /// Legal follower choices after a leader's primary.
        first leader_strategy_followers(s: &CoreState, who: Seat, leader: Leader, card: StrategyCard) -> Option<Followers>;
        /// Extra votes this player casts (summed).
        sum vote_bonus(s: &CoreState, who: Seat) -> i64;
        /// Timing abilities to register for one seat when the game is seated (`reactions::arm`).
        concat timing_abilities(s: &CoreState, owner: Seat) -> AbilityList;
        /// Shift to one unit's combat roll (summed, on top of `combat_modifier`).
        sum unit_roll_modifier(s: &CoreState, k: &Kernel, unit: CombatUnit) -> i64;
        /// One unit's combat dice, given the count so far (fold; an absolute set must say why).
        fold unit_dice(s: &CoreState, k: &Kernel, unit: CombatUnit; dice: i64) -> i64;
        /// After movement into `system`: skip directly to "Commit Ground Forces". `Some(true)` skips.
        any_some_true skip_to_commit(cx: &mut Cx<'_>, who: Seat, system: SystemIx) -> Option<bool>;
    }
}

/// The whole table: six groups of the string engine's `Hooks`, plus the direct calls.
#[derive(Clone, Default)]
pub struct HookTable {
    /// `factions::Hooks` top-level slots.
    pub general: GeneralHooks,
    /// `hooks_combat::CombatHooks`.
    pub combat: combat::CombatHooks,
    /// `hooks_ground::GroundHooks`.
    pub ground: ground::GroundHooks,
    /// `hooks_economy::EconomyHooks`.
    pub economy: economy::EconomyHooks,
    /// `hooks_movement::MovementHooks` (A2 builds `CoreRules` from these).
    pub movement: movement::MovementHooks,
    /// `hooks_cards::CardHooks`.
    pub cards: cards::CardHooks,
    /// `hooks_strategy::StrategyHooks`.
    pub strategy: strategy::StrategyHooks,
    /// The direct calls of the shared modules into faction code.
    pub direct: direct::DirectHooks,
    /// The six uniform seams (`kernel::seam`): stacks, predicates, gathers, replacements, observers and
    /// offers, filled by factions, laws, relics, technologies, promissory notes and leaders alike.
    pub seam: super::seam::Seams,
}

impl HookTable {
    /// An empty table: every hook neutral.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Slots in the table (the inventory number tests pin).
    pub const SLOTS: usize = GeneralHooks::SLOTS
        + combat::CombatHooks::SLOTS
        + ground::GroundHooks::SLOTS
        + economy::EconomyHooks::SLOTS
        + movement::MovementHooks::SLOTS
        + cards::CardHooks::SLOTS
        + strategy::StrategyHooks::SLOTS
        + direct::DirectHooks::SLOTS;

    /// Registered functions across the table.
    #[must_use]
    pub fn registered(&self) -> usize {
        self.general.registered()
            + self.combat.registered()
            + self.ground.registered()
            + self.economy.registered()
            + self.movement.registered()
            + self.cards.registered()
            + self.strategy.registered()
            + self.direct.registered()
            + self.seam.registered()
    }

    /// A copy with the commutative slots (and seams) registered in another order: reverse, or a seeded shuffle.
    #[must_use]
    pub fn permuted(&self, p: super::seam::Permute) -> Self {
        let mut t = self.clone();
        t.general.permute_commutative(p);
        t.combat.permute_commutative(p);
        t.ground.permute_commutative(p);
        t.economy.permute_commutative(p);
        t.movement.permute_commutative(p);
        t.cards.permute_commutative(p);
        t.strategy.permute_commutative(p);
        t.direct.permute_commutative(p);
        t.seam.permute_commutative(p);
        t
    }

    /// The order-mutation audit: run `probe` on this table and on the audit permutations
    /// ([`Permute::AUDIT`](super::seam::Permute::AUDIT)); the permutations whose answer differs. Empty means every
    /// slot declared commutative is, for what `probe` asks.
    pub fn audit_commutative<R: PartialEq>(&self, probe: impl Fn(&Self) -> R) -> Vec<super::seam::Permute> {
        let base = probe(self);
        super::seam::Permute::AUDIT.into_iter().filter(|p| probe(&self.permuted(*p)) != base).collect()
    }

    /// Register one faction module: `module` fills its slots.
    pub fn register(&mut self, module: fn(&mut Self)) -> &mut Self {
        module(self);
        self
    }

    /// A table built from the modules in order.
    #[must_use]
    pub fn of(modules: &[fn(&mut Self)]) -> Self {
        let mut table = Self::new();
        for m in modules {
            m(&mut table);
        }
        table
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_table_is_neutral() {
        let t = HookTable::new();
        let k = Kernel::default();
        let s = CoreState::all_loaded();
        assert_eq!(t.registered(), 0);
        assert_eq!(t.general.combat_modifier(&s, &k, Seat(0), CombatCtx::Space), 0);
        assert_eq!(t.general.fleet_supply(&s, &k, Seat(0), 7), 7);
        assert!(!t.general.trades_action_cards(&s, &k, Seat(0)));
        assert!(t.combat.may_sustain(&s, &k, CombatUnit::probe()));
    }

    #[test]
    fn combining_kinds_follow_the_string_dispatch() {
        let mut t = HookTable::new();
        let k = Kernel::default();
        let s = CoreState::all_loaded();
        t.general.fleet_supply.push(|_, _, _, l| l + 1);
        t.general.fleet_supply.push(|_, _, _, l| l * 2);
        assert_eq!(t.general.fleet_supply(&s, &k, Seat(0), 3), 8, "fold in registration order");
        t.general.combat_modifier.push(|_, _, _, _| 1);
        t.general.combat_modifier.push(|_, _, _, _| 2);
        assert_eq!(t.general.combat_modifier(&s, &k, Seat(0), CombatCtx::Ground), 3, "sum");
        t.combat.may_sustain.push(|_, _, _| true);
        t.combat.may_sustain.push(|_, _, _| false);
        assert!(!t.combat.may_sustain(&s, &k, CombatUnit::probe()), "all");
    }

    #[test]
    fn the_audit_passes_for_commutative_slots_and_catches_a_misclassified_one() {
        use crate::kernel::seam::Permute;
        let s = CoreState::all_loaded();
        let mut t = HookTable::new();
        for n in 1..=9 {
            t.general.combat_modifier.push(match n {
                1 => |_, _, _, _| 1,
                2 => |_, _, _, _| 2,
                3 => |_, _, _, _| 4,
                4 => |_, _, _, _| 8,
                5 => |_, _, _, _| 16,
                6 => |_, _, _, _| 32,
                7 => |_, _, _, _| 64,
                8 => |_, _, _, _| 128,
                _ => |_, _, _, _| 256,
            });
        }
        let probe = |t: &HookTable| {
            let k = Kernel { hooks: t.clone(), sources: crate::kernel::Sources::all() };
            t.general.combat_modifier(&s, &k, Seat(0), CombatCtx::Space)
        };
        assert_eq!(t.audit_commutative(probe), Vec::<Permute>::new(), "a sum commutes");
        // A fold is not permuted by the audit (it is not declared commutative) ...
        t.general.fleet_supply.push(|_, _, _, l| l + 1);
        t.general.fleet_supply.push(|_, _, _, l| l * 2);
        let fold = |t: &HookTable| {
            let k = Kernel { hooks: t.clone(), sources: crate::kernel::Sources::all() };
            t.general.fleet_supply(&s, &k, Seat(0), 3)
        };
        assert!(t.audit_commutative(fold).is_empty());
        // ... and a slot that was wrongly declared commutative is caught: here `all` over functions that disagree
        // in a way that depends on position is impossible, but a `concat_sorted`-like probe of the raw order is not.
        let raw_order = |t: &HookTable| t.general.combat_modifier.iter().map(|f| f(&s, &Kernel::plain(), Seat(0), CombatCtx::Space)).collect::<Vec<_>>();
        assert_eq!(t.audit_commutative(raw_order).len(), Permute::AUDIT.len(), "the raw order does change");
    }

    #[test]
    fn the_inventory_has_every_string_slot() {
        // 26 general + 9 combat + 8 ground + 20 economy + 21 movement + 7 cards + 8 strategy = 99 (the string `Hooks`),
        // plus A2's split of `may_move_through_ships` (the part that ignores the active system).
        assert_eq!(GeneralHooks::SLOTS, 26);
        assert_eq!(combat::CombatHooks::SLOTS, 9);
        assert_eq!(ground::GroundHooks::SLOTS, 8);
        assert_eq!(economy::EconomyHooks::SLOTS, 20);
        assert_eq!(movement::MovementHooks::SLOTS, 21 + 1);
        assert_eq!(cards::CardHooks::SLOTS, 7);
        assert_eq!(strategy::StrategyHooks::SLOTS, 8);
        assert_eq!(HookTable::SLOTS, 99 + 1 + direct::DirectHooks::SLOTS);
    }
}
