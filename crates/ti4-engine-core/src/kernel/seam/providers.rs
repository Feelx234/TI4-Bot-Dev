//! Providers that are not factions (docs/ENGINE_DISENTANGLE.md 2.8): laws, relics, technologies, promissory
//! notes and leaders register on the same seams as a faction does, as plain functions of `(Q, &site)` tagged
//! with their [`Source`]. State-derived effects are queries; a provider is the three-line adapter that calls
//! the query. Where the effect is "has X, then +n" it is a data row instead ([`COMBAT_ROWS`]).
//!
//! [`register_core`] is what a game's kernel setup calls next to `ti4_engine_core_factions::register_all`.
//! Each area owner adds its providers here, in their own function (`laws`, `relics`, ...), not in a window.

use super::data::COMBAT_ROWS;
use super::{Delta, Layer, RuleRef, Source, SpendSite};
use crate::kernel::HookTable;
use crate::query::{laws, reach};

/// `Source::Rule` code of the planet-value laws (Minister of Commerce style effects; `ValueLaw`).
pub const LAW_PLANET_VALUE: Source = Source::Rule(RuleRef(9101));

/// The hook table's `Spend` and the query layer's are two types of the same two values.
const fn query_spend(s: super::Spend) -> crate::query::economy::Spend {
    match s {
        super::Spend::Resources => crate::query::economy::Spend::Resources,
        super::Spend::Influence => crate::query::economy::Spend::Influence,
    }
}

/// Register the providers of the laws, relics and technologies the core carries.
pub fn register_core(t: &mut HookTable) {
    t.seam.register_rows(&COMBAT_ROWS);
    // laws::planet_value_bonus: a law that changes what a planet is worth when spent.
    t.seam.planet_spend.push_add(LAW_PLANET_VALUE, Layer::Law, |q, site: &SpendSite, _| {
        match laws::planet_value_bonus(q.s, reach::blanked(q.s), site.planet, query_spend(site.spend)) {
            0 => Delta::None,
            n => Delta::Add(n),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    use crate::containers::SeatSet;
    use crate::generated::{Faction, Leader, PromissoryNote, Relic, Technology};
    use crate::ids::Seat;
    use crate::kernel::Kernel;
    use crate::state::{CoreState, Player, Players};

    fn state() -> CoreState {
        let mut s = CoreState::all_loaded();
        *s.players_mut() = Players::from_vec(vec![
            Player::new(Seat(0), Faction::SardakkNorr),
            Player::new(Seat(1), Faction::TheUniversitiesOfJolNar),
        ]);
        s
    }

    #[test]
    fn rows_feed_the_hit_shift_stack_by_seat() {
        let mut k = Kernel::plain();
        register_core(&mut k.hooks);
        let s = state();
        let q = Q::new(&s, &k);
        let mut site = RollSite::probe();
        site.unit.owner = Seat(0);
        assert_eq!(k.hooks.seam.hit_shift.eval(q, &site, 8, Clamp::None).value, 9, "Unrelenting");
        site.unit.owner = Seat(1);
        let e = k.hooks.seam.hit_shift.eval(q, &site, 8, Clamp::None);
        assert_eq!((e.value, e.parts.len()), (7, 1), "Fragile");
        site.kind = RollKind::SpaceCannon;
        assert_eq!(k.hooks.seam.hit_shift.eval(q, &site, 8, Clamp::None).value, 8, "combat rows skip a cannon shot");
    }

    #[test]
    fn every_kind_of_non_faction_provider_registers_on_the_seams() {
        let mut t = HookTable::new();
        register_core(&mut t);
        let base = t.seam.registered();
        // One stub per source kind, on three different seams.
        t.seam.hit_shift.push_add(Source::Technology(Technology::AntimassDeflectors), Layer::Technology, |_, _, _| Delta::Add(1));
        t.seam.hit_shift.push_add(Source::Relic(Relic::LightrailOrdnance), Layer::Relic, |_, _, _| Delta::Add(1));
        t.seam.hit_shift.push_add(Source::Promissory(PromissoryNote::ALL[0]), Layer::Law, |_, _, _| Delta::Add(1));
        t.seam.dice.push_add(Source::Leader(Leader::ALL[0]), Layer::Leader, |_, _, _| Delta::Add(1));
        t.seam.may_sustain.push(Source::Technology(Technology::NonEuclideanShielding), |_, _| Verdict::Forbid);
        t.seam.alastor_participants.push(Source::Relic(Relic::LightrailOrdnance), Pass::Add, |_, _, _| {});
        assert_eq!(t.seam.registered(), base + 6);
        let k = Kernel { hooks: t, sources: crate::kernel::Sources::all() };
        let s = state();
        let q = Q::new(&s, &k);
        assert_eq!(k.hooks.seam.hit_shift.eval(q, &RollSite::probe(), 0, Clamp::None).value, 3 + 1, "three stubs and Unrelenting");
        assert!(!k.hooks.seam.may_sustain.decide(q, &CombatUnit::probe(), true));
    }

    #[test]
    fn the_order_audit_holds_for_rows_and_stubs() {
        let mut t = HookTable::new();
        register_core(&mut t);
        for _ in 0..6 {
            t.seam.hit_shift.push_add(Source::Printed, Layer::Faction, |_, _, _| Delta::Add(1));
        }
        let s = state();
        let probe = |t: &HookTable| {
            let k = Kernel { hooks: t.clone(), sources: crate::kernel::Sources::all() };
            let q = Q::new(&s, &k);
            let mut site = RollSite::probe();
            site.foes = SeatSet::EMPTY;
            let e = k.hooks.seam.hit_shift.eval(q, &site, 8, Clamp::None);
            (e.value, e.parts.len())
        };
        assert!(t.audit_commutative(probe).is_empty());
    }

    #[test]
    fn the_legacy_bridge_answers_what_the_old_slot_answers() {
        let mut t = HookTable::new();
        t.general.combat_modifier.push(|_, _, _, _| 2);
        t.general.unit_roll_modifier.push(|_, _, _| 3);
        t.general.unit_dice.push(|_, _, _, d| d + 1);
        t.combat.may_sustain.push(|_, _, _| false);
        adapters::bridge_legacy(&mut t.seam);
        let k = Kernel { hooks: t, sources: crate::kernel::Sources::all() };
        let s = state();
        let q = Q::new(&s, &k);
        let site = RollSite::probe();
        let old = k.hooks.general.combat_modifier(&s, &k, site.unit.owner, site.unit.ctx) + k.hooks.general.unit_roll_modifier(&s, &k, site.unit);
        assert_eq!(k.hooks.seam.hit_shift.eval(q, &site, 8, Clamp::None).value, 8 + old);
        assert_eq!(k.hooks.seam.dice.eval(q, &site, 1, Clamp::Min(0)).value, k.hooks.general.unit_dice(&s, &k, site.unit, 1));
        assert!(!k.hooks.seam.may_sustain.decide(q, &site.unit, true));
    }
}
