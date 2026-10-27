//! winnu: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CB). Owned by CB only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `ground.custodians_free` | `custodians_free` | Q | `ground.custodians_free` |
//! | `economy.planet_spend_value` | `planet_spend_value` | Q | `economy.planet_spend_value` (a fold; Hegemonic Trade Policy swap) |
//! | `unit_dice` | `unit_dice` | Q | `general.unit_dice` (a fold) |
//! | `strategy.secondary_waivers` | `secondary_waivers` | Q | `strategy.secondary_waivers` |
//! | `strategy.secondary_waived` | `secondary_waived` | Q | `strategy.secondary_waived` (gives the note back) |
//! | `direct.winnu_agent_ready` | `agent_ready` | Q | `direct.winnu_agent_ready` |
//! | `unit_roll_modifier` | `unit_roll_modifier` | Q, BLOCKED | needs `promissory::has_commander_ability` (Mahact Imperia, Alliance, grants): not in the core |
//! | `direct.winnu_exhaust_agent_for_production` | `exhaust_agent_for_production` | W | `offer.rs` (B3's `Stage::Offering`) |
//! | `component_actions`, `perform_component`, leaders, timing abilities | | W | B7 |
//! | movement `linked_systems`, `move_bonus` | | Q | `movement.rs` (A2, done) |

use ti4_engine_core::generated::{Faction, Leader, Planet, PromissoryNote, Technology};
use ti4_engine_core::hooks::strategy::Waivers;
use ti4_engine_core::hooks::{CombatCtx, CombatUnit, HookTable, Spend, Waiver};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::{Cx, Kernel};
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::state::{CoreState, LeaderStatus, NoteKey};
use ti4_engine_core::tables::Scope;

use crate::common::{flagship_has_text, flagship_of, has_tech_text, is_faction, mark, num, player};

/// The code of the Acquiescence waiver (the module is the Winnu; the adapter renders `acq`).
pub const ACQUIESCENCE: u16 = 0;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.ground.custodians_free.push(custodians_free);
    t.economy.planet_spend_value.push(planet_spend_value);
    t.general.unit_dice.push(unit_dice);
    t.strategy.secondary_waivers.push(secondary_waivers);
    t.strategy.secondary_waived.push(secondary_waived);
    t.direct.winnu_agent_ready.push(agent_ready);
}

fn is_winnu(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheWinnu)
}

/// Blood Ties: the Custodians Token costs the Winnu no influence.
pub fn custodians_free(s: &CoreState, _: &Kernel, who: Seat) -> bool {
    is_winnu(s, who)
}

/// Berekar Berekon is ready for `who`.
pub fn agent_ready(s: &CoreState, who: Seat) -> bool {
    player(s, who).is_some_and(|p| p.leaders.status.get(&Leader::BerekarBerekon) == Some(&LeaderStatus::Readied))
}

const ACQUIESCENCE_NOTE: NoteKey = NoteKey { note: PromissoryNote::Acquiescence, faction: Faction::TheWinnu };

fn holds_acquiescence(s: &CoreState, who: Seat) -> bool {
    s.cards().promissory.get(&ACQUIESCENCE_NOTE) == Some(&who)
}

/// Acquiescence: the follower holding the note may resolve the Winnu's secondary using it.
pub fn secondary_waivers(
    s: &CoreState,
    _: &Kernel,
    follower: Seat,
    primary: Seat,
    _: Option<ti4_engine_core::generated::StrategyCardKind>,
) -> Waivers {
    let mut out = Waivers::new();
    if holds_acquiescence(s, follower) && follower != primary && is_winnu(s, primary) {
        out.push(Waiver { module: Faction::TheWinnu, code: ACQUIESCENCE });
    }
    out
}

/// `promissory::give_back`: the note goes home to the first seat (in seating order) that plays its faction.
fn give_back(s: &mut CoreState, note: NoteKey) {
    let Some(owner) = s.seats().seating_order().iter().copied().find(|seat| player(s, *seat).is_some_and(|p| p.faction == note.faction)) else {
        return;
    };
    if s.cards().promissory.get(&note) == Some(&owner) {
        return;
    }
    let cards = s.cards_mut();
    cards.promissory.edit().insert(note, owner);
    cards.promissory_faceup.edit().remove(&note);
}

/// The follower used Acquiescence: the note goes back.
pub fn secondary_waived(cx: &mut Cx<'_>, follower: Seat, primary: Seat, waiver: Waiver) {
    if waiver.module == Faction::TheWinnu
        && waiver.code == ACQUIESCENCE
        && holds_acquiescence(cx.state, follower)
        && is_winnu(cx.state, primary)
    {
        give_back(cx.state, ACQUIESCENCE_NOTE);
    }
}

/// The scope `production::swapped_value` reads the printed values in (`content_types::DEFAULT`, every source).
const ALL_SOURCES: Scope = Scope(u32::MAX);

/// `production::swapped_planet`: the planet whose values are swapped in the use now in progress.
fn swapped_planet(s: &CoreState) -> Option<Planet> {
    let planet = s.transient().production.value_swapped_planet?;
    (mark(s, MarkKey::ProductionValueSwapSeq()).and_then(num) == Some(s.meta().production_seq)).then_some(planet)
}

/// Hegemonic Trade Policy: the swapped planet reads as its other value.
pub fn planet_spend_value(s: &CoreState, k: &Kernel, who: Seat, planet: Planet, kind: Spend, value: i64) -> i64 {
    use ti4_engine_core::query::economy::Spend as Kind;
    if !has_tech_text(s, who, Technology::HegemonicTradePolicy) || !ti4_engine_core::query::board::controls(s, who, planet) {
        return value;
    }
    if swapped_planet(s) != Some(planet) {
        return value;
    }
    let other = match kind {
        Spend::Resources => Kind::Influence,
        Spend::Influence => Kind::Resources,
    };
    // `planet_value_now` also adds the Last Bastion's Helios dock to resources (a hook term).
    let dock = if other == Kind::Resources { k.hooks.direct.bastion_units_resource_bonus(s, planet) } else { 0 };
    ti4_engine_core::query::economy::value_now(s, ALL_SOURCES, ti4_engine_core::query::reach::blanked(s), planet, other, dock)
}

/// Salai Sai Corian: the flagship rolls one die per non-fighter ship of the other players in the system.
pub fn unit_dice(s: &CoreState, k: &Kernel, unit: CombatUnit, dice: i64) -> i64 {
    let flagship = flagship_of(Faction::TheWinnu);
    let own = Some(unit.unit) == flagship && is_winnu(s, unit.owner);
    let lent = Some(unit.unit) != flagship && flagship.is_some_and(|f| flagship_has_text(s, unit.owner, unit.unit, f));
    if !(own || lent) || unit.ctx != CombatCtx::Space {
        return dice;
    }
    let Some(system) = unit.system else { return dice };
    let scope = crate::common::scope(k);
    let theirs = s
        .board()
        .sys(system)
        .units
        .as_slice()
        .iter()
        .filter(|p| p.owner() != unit.owner)
        .filter(|p| scope.unit(p.kind()).is_some_and(|d| d.ship && d.base_type != ti4_engine_core::generated::UnitBaseType::Fighter))
        .count();
    let theirs = i64::try_from(theirs).unwrap_or(0);
    if Some(unit.unit) == flagship { dice + theirs } else { theirs }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::cb_economy::testkit::table;
    use ti4_engine_core::generated::Unit;
    use ti4_engine_core::keys::{Atom, MarkVal};
    use ti4_engine_core::state::Piece;

    const W: Faction = Faction::TheWinnu;
    const SOL: Faction = Faction::TheFederationOfSol;

    #[test]
    fn blood_ties_and_the_agent() {
        let (mut s, k) = table(&[W, SOL]);
        assert!(custodians_free(&s, &k, Seat(0)));
        assert!(!custodians_free(&s, &k, Seat(1)));
        assert!(!agent_ready(&s, Seat(0)));
        s.players_mut().edit(0).leaders.status.insert(Leader::BerekarBerekon, LeaderStatus::Readied);
        assert!(agent_ready(&s, Seat(0)));
        s.players_mut().edit(0).leaders.status.insert(Leader::BerekarBerekon, LeaderStatus::Exhausted);
        assert!(!agent_ready(&s, Seat(0)));
    }

    #[test]
    fn acquiescence_is_offered_to_its_holder_and_goes_home_when_used() {
        let (mut s, k) = table(&[W, SOL]);
        s.cards_mut().promissory.edit().insert(ACQUIESCENCE_NOTE, Seat(1));
        s.cards_mut().promissory_faceup.edit().insert(ACQUIESCENCE_NOTE, ());
        let offered = secondary_waivers(&s, &k, Seat(1), Seat(0), None);
        assert_eq!(offered.as_slice(), &[Waiver { module: Faction::TheWinnu, code: ACQUIESCENCE }]);
        assert!(secondary_waivers(&s, &k, Seat(0), Seat(0), None).is_empty(), "the Winnu holds none");
        assert!(secondary_waivers(&s, &k, Seat(1), Seat(1), None).is_empty(), "the primary must be the Winnu");
        let mut ask = ti4_engine_core::kernel::FirstOption;
        let mut cx = Cx::new(&mut s, &k, &mut ask);
        secondary_waived(&mut cx, Seat(1), Seat(0), Waiver { module: Faction::TheWinnu, code: ACQUIESCENCE });
        assert_eq!(s.cards().promissory.get(&ACQUIESCENCE_NOTE), Some(&Seat(0)));
        assert!(s.cards().promissory_faceup.get(&ACQUIESCENCE_NOTE).is_none());
    }

    #[test]
    fn the_flagship_rolls_a_die_per_opposing_non_fighter_ship() {
        let (mut s, k) = table(&[W, SOL]);
        let sys = ti4_engine_core::ids::system_at(30);
        let flagship = flagship_of(Faction::TheWinnu).unwrap();
        let theirs = [Unit::CruiserI, Unit::FighterI, Unit::DreadnoughtI];
        for u in theirs {
            s.board_mut().sys_mut(sys).units.push(Piece::new(u, Seat(1)));
        }
        let unit = CombatUnit { owner: Seat(0), system: Some(sys), planet: None, unit: flagship, ctx: CombatCtx::Space };
        assert_eq!(unit_dice(&s, &k, unit, 0), 2, "a fighter does not count");
        assert_eq!(unit_dice(&s, &k, CombatUnit { ctx: CombatCtx::Ground, ..unit }, 0), 0);
        assert_eq!(unit_dice(&s, &k, CombatUnit { owner: Seat(1), unit: Unit::CruiserI, ..unit }, 3), 3, "not the flagship");
    }

    #[test]
    fn trade_policy_swaps_the_marked_planet_for_the_use_in_progress() {
        let (mut s, k) = table(&[W, SOL]);
        let planet = Planet::Abaddon;
        let (res, inf) = (i64::from(planet.data().resources), i64::from(planet.data().influence));
        s.players_mut().edit(0).tech.owned.insert(Technology::HegemonicTradePolicy);
        s.board_mut().planet_mut(planet).control = Some(Seat(0));
        s.transient_mut().production.value_swapped_planet = Some(planet);
        s.meta_mut().production_seq = 4;
        assert_eq!(planet_spend_value(&s, &k, Seat(0), planet, Spend::Resources, res), res, "no seq mark: no swap");
        s.marks_mut().tail.edit().insert(MarkKey::ProductionValueSwapSeq(), MarkVal::Atom(Atom::Num(4)));
        assert_eq!(planet_spend_value(&s, &k, Seat(0), planet, Spend::Resources, res), inf);
        assert_eq!(planet_spend_value(&s, &k, Seat(0), planet, Spend::Influence, inf), res);
        assert_eq!(planet_spend_value(&s, &k, Seat(1), planet, Spend::Resources, res), res, "not the holder");
    }
}
