//! nekro: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CA).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `COMBAT_HOOKS` | `nekro_units::COMBAT_HOOKS` (= `CombatHooks::NONE`) | - | nothing to port |
//! | `nekro_units_mech_roll_bonus` | `nekro_units::mech_roll_bonus` | Q | seam `hit_shift` (space and ground combat) |
//! | `nekro_units_alastor_participants` | `nekro_units::alastor_participants` | Q | direct (`alastor_participants`' bridge reads it) |
//! | `nekro_units_alastor_covers` | `nekro_units::alastor_covers` | Q | direct |
//! | `nekro_pending_propagation` | `nekro::pending_propagation` | Q | direct |
//! | `nekro_units_alastor_decrement`, `_clear` | `nekro_units::alastor_{decrement,clear}` | O | `observe.rs` (they write marks) |
//! | `timing_abilities`, `commander_unlocked`, leaders, component actions | | W | `offer.rs` |

use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Planet, System, Technology, Unit};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::hooks::direct::Seats;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, DecimalTextOrd, MarkKey, MarkVal};
use ti4_engine_core::kernel::seam::{Delta, Layer, Q, RollKind, RollSite, Source};
use ti4_engine_core::query::board;
use ti4_engine_core::state::CoreState;

use crate::common::{is_faction, mark, num};

const SOURCE: Source = Source::Faction(Faction::TheNekroVirus);
const MECH_BONUS: i64 = 2;
const ASSIMILATORS: [Technology; 2] = [Technology::ValefarAssimilatorX, Technology::ValefarAssimilatorY];

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.seam.hit_shift.push_add(SOURCE, Layer::Faction, mordred);
    let d = &mut t.direct;
    d.nekro_units_alastor_participants.push(alastor_participants);
    d.nekro_units_alastor_covers.push(alastor_covers);
    d.nekro_pending_propagation.push(pending_propagation);
}

/// What `who`'s Alastor chose in this activation's combat in `system`: `(planet, unit, count)` in key order.
fn alastor_marks(s: &CoreState, system: System, who: Seat) -> SmallList<(Planet, Unit, u32), 4> {
    let seq = DecimalTextOrd(s.meta().activation_seq);
    let mut out = SmallList::default();
    for (key, value) in s.marks().tail.iter() {
        if let MarkKey::NekroAlastor(k_seq, k_system, k_owner, planet, unit) = *key
            && k_seq == seq
            && k_system == system
            && k_owner == who
            && let MarkVal::Atom(Atom::Num(count)) = value
        {
            out.push((planet, unit, *count));
        }
    }
    out
}

/// The ground forces of `who` standing in `system` that an Alastor chose to fight this combat, as
/// `(planet, index in that planet's unit list)`.
fn alastor_participants(s: &CoreState, who: Seat, system: System) -> SmallList<(Planet, u16), 4> {
    let marks = alastor_marks(s, system, who);
    let mut found: Vec<(Planet, u16)> = Vec::new();
    for (planet, unit, count) in marks.iter() {
        if board::home_system(s, *planet) != Some(system) {
            continue;
        }
        found.extend(
            board::planet_units(s, *planet)
                .iter()
                .enumerate()
                .filter(|(_, held)| held.owner() == who && held.kind() == *unit)
                .map(|(index, _)| (*planet, u16::try_from(index).unwrap_or(u16::MAX)))
                .take(*count as usize),
        );
    }
    found.sort_unstable();
    found.dedup();
    found.into_iter().collect()
}

/// Whether `owner` has chosen ground forces of `unit` for this combat in `system`.
fn alastor_covers(s: &CoreState, system: System, owner: Seat, unit: Unit) -> bool {
    alastor_marks(s, system, owner).iter().any(|(_, kind, count)| *kind == unit && *count > 0)
}

/// Seats owing Propagation tokens, once per pending research, in seat order.
fn pending_propagation(s: &CoreState) -> Seats {
    let mut owed = Seats::default();
    for p in s.players().iter() {
        let count = mark(s, MarkKey::NekroPropagation(p.seat)).and_then(num).unwrap_or(0);
        for _ in 0..count {
            owed.push(p.seat);
        }
    }
    owed
}

/// `nekro::live_token`: the technology the Valefar Assimilator `card` of `who` stands for, while another player
/// owns it.
fn live_token(s: &CoreState, who: Seat, card: Technology) -> Option<Technology> {
    let p = s.players().by_seat(who)?;
    if p.faction != Faction::TheNekroVirus || !p.tech.owned.contains(card) {
        return None;
    }
    let target = *p.tech.assimilated.get(&card)?;
    s.players().iter().any(|o| o.seat != who && o.tech.owned.contains(target)).then_some(target)
}

/// Whether `opponent` has an "X" or "Y" assimilator token on one of their technologies.
fn has_token_on_technology(s: &CoreState, owner: Seat, opponent: Seat) -> bool {
    s.players().by_seat(opponent).is_some_and(|o| {
        ASSIMILATORS.iter().any(|card| live_token(s, owner, *card).is_some_and(|target| o.tech.owned.contains(target)))
    })
}

/// `combat::ships_of(..).is_empty()` negated: a ship in the space area, or a ground force the Alastor chose.
fn has_ships(s: &CoreState, who: Seat, system: System) -> bool {
    s.board().sys(system).units.as_slice().iter().any(|p| p.owner() == who && p.kind().data().ship)
        || !alastor_participants(s, who, system).is_empty()
}

/// Mordred: +2 to its rolls against an opponent with an "X" or "Y" assimilator token on a technology.
fn mordred(q: Q<'_>, site: &RollSite, _: i64) -> Delta {
    let u = &site.unit;
    if !matches!(site.kind, RollKind::Combat | RollKind::Ground) || u.unit != Unit::Mordred || !is_faction(q.s, u.owner, Faction::TheNekroVirus) {
        return Delta::None;
    }
    let Some(system) = u.system else {
        return Delta::None;
    };
    let against = q.s.players().iter().map(|p| p.seat).filter(|seat| *seat != u.owner);
    let hit = match u.planet {
        Some(planet) => {
            if board::home_system(q.s, planet) != Some(system) {
                return Delta::None;
            }
            against.filter(|seat| board::planet_units(q.s, planet).iter().any(|p| p.owner() == *seat))
                .any(|seat| has_token_on_technology(q.s, u.owner, seat))
        }
        None => against.filter(|seat| has_ships(q.s, *seat, system)).any(|seat| has_token_on_technology(q.s, u.owner, seat)),
    };
    if hit { Delta::Add(MECH_BONUS) } else { Delta::None }
}
