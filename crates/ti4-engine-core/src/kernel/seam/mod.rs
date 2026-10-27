//! The six uniform seams (docs/ENGINE_DISENTANGLE.md section 2; package C0, the seam steward).
//!
//! A rule window (combat, production, invasion, ...) never names a faction, a law, a relic or a card. It
//! asks one of six *seams*, each a typed slot of the [`Seams`] table that sits on the [`HookTable`]:
//!
//! | pattern | type | what the window does | module |
//! |---|---|---|---|
//! | P1 modifier on a number | [`Stack`] | `stack.eval(q, &site, base, clamp)` | [`stack`] |
//! | P2 predicate or veto | [`Pred`] | `pred.decide(q, &site, default)` | [`pred`] |
//! | P3 contribution list | [`Gather`] | `gather.collect(q, &site)` | [`gather`] |
//! | P4 replacement of a rule step | [`Replacements`] | `first(q, &site)` then `perform` | [`replace`] |
//! | P5 observer on an event | [`Observers`] | `cx.observe(&Obs::X, Phase::After)` | [`observe`] |
//! | P6 offer point | [`Offers`] | the stage iterates the offers of a [`Point`] | [`offer`] |
//!
//! Providers are *anyone*: a faction module (`ti4-engine-core-factions`), but equally a law, a relic, a
//! technology, a promissory note or a leader, each as a plain function of `(Q, &site)` tagged with its
//! [`Source`]. State-derived effects ("owns Antimass Deflectors") are queries over [`CoreState`] and need
//! no hook of their own: the provider is a three-line function that calls the query.
//!
//! # Order
//!
//! The string engine's order of evaluation is written down here once. A [`Stack`] orders providers by
//! `(Layer, registration index)`; a [`Gather`] keeps registration order (or sorts, when it says so); first-wins
//! ([`Replacements`], [`Offers`]) is registration order. Registration order is the string `MODULES`
//! order, which fixes the dice stream. Slots whose combination commutes (adds, predicates, sorted
//! gathers) can be proven so by [`Permute`]: see `HookTable::audit_commutative`.
//!
//! # The old seams
//!
//! [`adapters`] maps the existing A5 slots, B1's `Faction` trait and B3's `Env` trait onto these types.
//! Existing slot names stay as registration names; `adapters::bridge_legacy` registers one provider per
//! legacy slot so a window can switch to the seam before any faction has been ported to it.

pub mod adapters;
pub mod data;
pub mod gather;
pub mod observe;
pub mod offer;
pub mod pred;
pub mod providers;
pub mod replace;
pub mod sites;
pub mod stack;

pub use data::{ModifierRow, RowKind};
pub use gather::{Contrib, Gather, Pass};
pub use observe::{Obs, ObsKind, Observer, Observers, Phase};
pub use offer::{Offer, OfferSite, Offers, Point};
pub use pred::{Combine, Pred, Rule, Verdict};
pub use replace::{Replacement, Replacements, Step};
pub use sites::*;
pub use stack::{Clamp, Delta, Entry, Evaluated, Layer, Provider, SetWhy, Stack};

use crate::generated::{ActionCard, Agenda, Breakthrough, Faction, Leader, PromissoryNote, Relic, Technology};
use crate::kernel::Kernel;
use crate::state::CoreState;

/// What every pure provider gets: the state and the kernel (hook table and source scope). `Copy`, two pointers.
#[derive(Clone, Copy)]
pub struct Q<'a> {
    pub s: &'a CoreState,
    pub k: &'a Kernel,
}

impl Source {
    /// The layer a data row of this source takes in a [`Stack`].
    #[must_use]
    pub const fn layer(self) -> Layer {
        match self {
            Self::Printed | Self::Rule(_) => Layer::Printed,
            Self::Technology(_) => Layer::Technology,
            Self::Relic(_) => Layer::Relic,
            Self::Law(_) | Self::Promissory(_) => Layer::Law,
            Self::Faction(_) | Self::Breakthrough(_) => Layer::Faction,
            Self::Leader(_) => Layer::Leader,
            Self::ActionCard(_) => Layer::Card,
        }
    }
}

impl<'a> Q<'a> {
    #[must_use]
    pub const fn new(s: &'a CoreState, k: &'a Kernel) -> Self {
        Self { s, k }
    }
}

/// A rule of the game that is not a card (a step of the rules reference), for [`Source::Rule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleRef(pub u16);

/// Where a contribution comes from: the display facts ("details") of a window, and the order audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    Faction(Faction),
    Technology(Technology),
    Relic(Relic),
    /// A law in play (`Agenda`).
    Law(Agenda),
    Promissory(PromissoryNote),
    Leader(Leader),
    Breakthrough(Breakthrough),
    ActionCard(ActionCard),
    /// The printed value of a unit or a planet.
    Printed,
    Rule(RuleRef),
}

/// The seams of one game: every typed slot. Part of [`HookTable`](crate::kernel::HookTable) as `seam`.
#[derive(Clone)]
pub struct Seams {
    /// P1: shift to the number a die must show (`combat_modifier` + `unit_roll_modifier` + bonuses + assail + laws).
    pub hit_shift: Stack<RollSite>,
    /// P1: dice a unit rolls (`unit_dice` fold + Bastion extra die + round-agent extra die).
    pub dice: Stack<RollSite>,
    /// P1: the spend value of a planet (`planet_spend_value`, `resource_bonus`, `planet_value_bonus`).
    pub planet_spend: Stack<SpendSite>,
    /// P1: PRODUCTION a system or planet grants (`extra_production[_planet]`).
    pub extra_production: Stack<ProdSite>,
    /// P1: cost reduction of a production (`production_cost_reduction`).
    pub production_discount: Stack<ProdSite>,
    /// P2: may this unit sustain damage (every rule must allow; default allowed).
    pub may_sustain: Pred<CombatUnit>,
    /// P2: the allow side of `may_sustain`: a unit that gains SUSTAIN DAMAGE from a card or ability (`grants_sustain`;
    /// one rule is enough, default not granted). `may_sustain` only forbids; this one only grants.
    pub grants_sustain: Pred<CombatUnit>,
    /// P2: immune to direct hits (one rule is enough).
    pub hit_immune: Pred<CombatUnit>,
    /// P2: a piece whose abilities are off (`context_silenced`, `ralnel_silenced_in_space`, `crimson_abilities_lost`, ...).
    pub silenced: Pred<SilenceSite>,
    /// P2: space cannon cannot fire at the target.
    pub cannon_barred: Pred<CannonSite>,
    /// P2: `cannot_produce` (one rule is enough).
    pub cannot_produce: Pred<ProduceSite>,
    /// P2: a faction deploy or commit that is blocked.
    pub commit_blocked: Pred<CommitSite>,
    /// P3: ground forces that fight as ships (Alastor).
    pub alastor_participants: Gather<SeatSystemSite, (crate::generated::Planet, u16), 4>,
    /// P3: extra places a production may place in (`production_destinations`).
    pub production_destinations: Gather<DestSite, crate::kernel::hooks::economy::Destination, 4>,
    /// P3: planets an invader may deploy to (`crimson_deploy_planets`).
    pub deploy_planets: Gather<DeploySite, crate::generated::Planet, 4>,
    /// P4: a unit produced in exchange for another (`production_unit_exchange` and `_performed`).
    pub unit_exchange: Replacements<ExchangeSite, ()>,
    /// P4: stats a flagship borrows (`borrowed_stats`).
    pub borrowed_stats: Replacements<SeatSystemSite, crate::kernel::hooks::BorrowedStats>,
    /// The data rows ([`ModifierRow`]) that are read through helpers instead of a number stack.
    pub rows: Vec<&'static ModifierRow>,
    /// P5: bookkeeping at program points.
    pub observers: Observers,
    /// P6: offers at named points of a window.
    pub offers: Offers,
}

impl Default for Seams {
    fn default() -> Self {
        Self {
            hit_shift: Stack::new(),
            dice: Stack::new(),
            planet_spend: Stack::new(),
            extra_production: Stack::new(),
            production_discount: Stack::new(),
            may_sustain: Pred::new(Combine::ForbidWins),
            grants_sustain: Pred::new(Combine::AllowWins),
            hit_immune: Pred::new(Combine::AllowWins),
            silenced: Pred::new(Combine::AllowWins),
            cannon_barred: Pred::new(Combine::AllowWins),
            cannot_produce: Pred::new(Combine::AllowWins),
            commit_blocked: Pred::new(Combine::AllowWins),
            alastor_participants: Gather::new(gather::Order::Registration),
            production_destinations: Gather::new(gather::Order::Sorted),
            deploy_planets: Gather::new(gather::Order::Registration),
            unit_exchange: Replacements::new(),
            borrowed_stats: Replacements::new(),
            rows: Vec::new(),
            observers: Observers::default(),
            offers: Offers::default(),
        }
    }
}

impl Seams {
    /// Typed slots: 5 stacks, 7 predicates, 3 gathers, 2 replacements, one per [`ObsKind`] and per [`Point`].
    pub const SLOTS: usize = 5 + 7 + 3 + 2 + ObsKind::COUNT + Point::COUNT;

    /// Registered providers across the seams.
    #[must_use]
    pub fn registered(&self) -> usize {
        self.hit_shift.len()
            + self.dice.len()
            + self.planet_spend.len()
            + self.extra_production.len()
            + self.production_discount.len()
            + self.may_sustain.len()
            + self.grants_sustain.len()
            + self.hit_immune.len()
            + self.silenced.len()
            + self.cannon_barred.len()
            + self.cannot_produce.len()
            + self.commit_blocked.len()
            + self.alastor_participants.len()
            + self.production_destinations.len()
            + self.deploy_planets.len()
            + self.unit_exchange.len()
            + self.borrowed_stats.len()
            + self.rows.len()
            + self.observers.len()
            + self.offers.len()
    }

    /// Register data rows: shifts and extra dice go on their stack, the rest are read through [`data`] helpers.
    pub fn register_rows(&mut self, rows: &'static [ModifierRow]) {
        for row in rows {
            match row.kind {
                RowKind::RollShift(_) => self.hit_shift.push_row(row),
                RowKind::ExtraDice(_) => self.dice.push_row(row),
                _ => self.rows.push(row),
            }
        }
    }

    /// Permute the providers of every commutative seam (the order audit).
    pub fn permute_commutative(&mut self, p: Permute) {
        self.hit_shift.permute_commutative(p);
        self.dice.permute_commutative(p);
        self.planet_spend.permute_commutative(p);
        self.extra_production.permute_commutative(p);
        self.production_discount.permute_commutative(p);
        self.may_sustain.permute(p);
        self.grants_sustain.permute(p);
        self.hit_immune.permute(p);
        self.silenced.permute(p);
        self.cannon_barred.permute(p);
        self.cannot_produce.permute(p);
        self.commit_blocked.permute(p);
        self.alastor_participants.permute_if_sorted(p);
        self.production_destinations.permute_if_sorted(p);
        self.deploy_planets.permute_if_sorted(p);
    }
}

/// A reordering of a list of providers, for the order-mutation audit (docs/ENGINE_DISENTANGLE.md 5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permute {
    /// Last registered first.
    Reverse,
    /// A seeded Fisher-Yates shuffle (the same seed gives the same order for the same length).
    Shuffle(u64),
}

impl Permute {
    /// The permutations the audit runs.
    pub const AUDIT: [Self; 3] = [Self::Reverse, Self::Shuffle(0x9E37_79B9_7F4A_7C15), Self::Shuffle(0xD1B5_4A32_D192_ED03)];

    /// Reorder `items` in place.
    pub fn apply<T>(self, items: &mut [T]) {
        match self {
            Self::Reverse => items.reverse(),
            Self::Shuffle(seed) => {
                let mut x = seed | 1;
                for i in (1..items.len()).rev() {
                    // xorshift64*
                    x ^= x >> 12;
                    x ^= x << 25;
                    x ^= x >> 27;
                    let r = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
                    items.swap(i, (r % (i as u64 + 1)) as usize);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permute_changes_the_order_but_keeps_the_items() {
        for p in Permute::AUDIT {
            let mut v: Vec<u32> = (0..12).collect();
            p.apply(&mut v);
            assert_ne!(v, (0..12).collect::<Vec<_>>(), "{p:?} must move something");
            v.sort_unstable();
            assert_eq!(v, (0..12).collect::<Vec<_>>());
        }
    }

    #[test]
    fn an_empty_seam_table_is_neutral() {
        let k = Kernel::plain();
        let s = CoreState::all_loaded();
        let q = Q::new(&s, &k);
        assert_eq!(k.hooks.seam.registered(), 0);
        assert_eq!(k.hooks.seam.hit_shift.eval(q, &RollSite::probe(), 8, Clamp::None).value, 8);
        assert!(k.hooks.seam.may_sustain.decide(q, &CombatUnit::probe(), true));
        assert!(!k.hooks.seam.hit_immune.decide(q, &CombatUnit::probe(), false));
    }
}
