//! The facts the tactical composer builds from the position alone (the adapter's `tactical_dual::limits_of`, `cannon_dual::aftermath_head`
//! and `combat::note_holdings`, which read the string `GameState`, the content store and the card texts).
//!
//! L5: reads the fleet, the laws, the abilities and the promissory notes.

use crate::generated::{Faction, System, Unit};
use crate::ids::Seat;
use crate::kernel::ability::TacticalCodes;
use crate::kernel::Kernel;
use crate::query::{board, fleet, laws, reach};
use crate::rules::abilities;
use crate::rules::combat::space_cannon_offense::Facts as CannonFacts;
use crate::rules::tactical::fleet::Limits;
use crate::state::{CoreState, NoteHold};

use crate::driver::kernel_env::KernelEnv;
use crate::driver::windows::aftermath::Facts as HeadFacts;

/// The rules version from which an unlimited fleet supply (Darktalon Treilla's hero) lifts the enforcement too
/// (`ti4_model::state::RULES_FACTION_BUGFIXES`).
const RULES_FACTION_BUGFIXES: u32 = 17;

/// The unit forms whose card lets their SPACE CANNON reach an adjacent system (`combat::reaching_units`: "SPACE CANNON against ships that are
/// ... adjacent"; the string pins this set with `every_unit_that_reaches_an_adjacent_system_is_found`): PDS II, Hel-Titan II, the Xxcha
/// flagship and mech.
pub const REACHING: [Unit; 4] = [Unit::PDSII, Unit::HelTitanII, Unit::LoncaraSsodu, Unit::Indomitus];

/// The fleet supply of a seat as the rules compute it: the pool (with the tokens other seats hold in it), capped by Fleet Regulations,
/// lifted by Armada and the faction modules (`fleet::limit`).
#[must_use]
pub fn fleet_limit(core: &CoreState, k: &Kernel, seat: Seat) -> i32 {
    let foreign = k.hooks.direct.mahact_foreign_tokens(core, seat);
    let base = fleet::pool_base(core, seat, foreign);
    let capped = laws::fleet_pool_cap(core, reach::blanked(core), base);
    let env = KernelEnv::new(core, k);
    abilities::fleet_supply(&abilities::Cx { core, env: &env }, seat, capped)
}

/// The limits the enforcement of `seat` in `system` judges by (`tactical_dual::limits_of`).
///
/// At most two half-weight fighter forms are kept (the core `Limits` holds two; no table has more).
#[must_use]
pub fn limits(core: &CoreState, k: &Kernel, seat: Seat, system: System) -> Limits {
    let unlimited = fleet::is_unlimited(core, seat);
    let bill = fleet::UNLIMITED_FLEET_BILL;
    let limit = i64::from(fleet_limit(core, k, seat)).max(0);
    let enforce = if core.meta().rules_at_least(RULES_FACTION_BUGFIXES) && unlimited { bill } else { limit };
    let shown = if unlimited { bill } else { limit };
    let mut forms: Vec<Unit> = board::space_units(core, system).iter().filter(|p| p.owner() == seat).map(|p| p.kind()).collect();
    forms.sort_unstable_by_key(|u| *u as usize);
    forms.dedup();
    let mut halves = [None, None];
    let mut found = 0;
    for unit in forms {
        if k.hooks.combat.fighter_fleet_weight_halves(core, k, seat, unit) {
            if found >= halves.len() {
                break;
            }
            halves[found] = Some(unit);
            found += 1;
        }
    }
    Limits { enforce, shown, halves }
}

/// `combat::note_holdings`: for each seat, the issuers of the promissory notes it holds faceup in its play area (and the Supports for
/// the Throne lent to it), as `(holder, issuer)` pairs, holders in seat order and a holder's issuers in seat order. A faction played by
/// two seats names no issuer (the string never has one).
#[must_use]
pub fn note_holdings(core: &CoreState) -> NoteHold {
    let mut out = NoteHold::new();
    for holder in core.players().iter().map(|p| p.seat) {
        let mut issuers: Vec<Seat> = Vec::new();
        for (key, held_by) in core.cards().promissory.iter() {
            if *held_by == holder && core.cards().promissory_faceup.contains_key(key) {
                if let Some(issuer) = seat_of(core, key.faction) {
                    issuers.push(issuer);
                }
            }
        }
        for (owner, held_by) in core.trade().support_holders.iter() {
            if *held_by == holder {
                issuers.push(*owner);
            }
        }
        issuers.sort_unstable();
        issuers.dedup();
        for issuer in issuers {
            out.push((holder, issuer));
        }
    }
    out
}

/// The one seat playing `faction` (`promissory::seat_of`).
fn seat_of(core: &CoreState, faction: Faction) -> Option<Seat> {
    let mut seats = core.players().iter().filter(|p| p.faction == faction).map(|p| p.seat);
    let first = seats.next()?;
    seats.next().is_none().then_some(first)
}

/// Whether the position holds a hook of the activation that this window does not carry (A2b-bail-02: `activation_unported`): a technology
/// or an ability that asks, pays or ends the turn, a law, a relic, a note that denies the move.
#[must_use]
pub fn activation_hook_held(core: &CoreState, _k: &Kernel, seat: Seat, system: System) -> bool {
    use crate::generated::{Agenda, Relic, Technology};
    let tech = [Technology::SpatialConduitCylinders, Technology::ScanlinkDroneNetwork, Technology::EResSiphons, Technology::NullificationField];
    core.players().iter().any(|p| tech.iter().any(|t| p.tech.owned.contains(*t)))
        || mahact_in_play(core)
        || laws::active(core, reach::blanked(core), Agenda::MinisterOfPeace)
        || core.players().by_seat(seat).is_some_and(|p| p.cards.relics.as_slice().contains(&Relic::DominusOrb))
        || crate::query::promissory::denies_movement_into(core, seat, system)
}

/// Mahact at the table (Starlancer, the commander's re-activation).
#[must_use]
pub fn mahact_in_play(core: &CoreState) -> bool {
    core.players().iter().any(|p| p.faction == Faction::TheMahactGeneSorcerers)
}

/// The map edits recorded so far (`movement::recorded_edits`): a tile replaced or swapped by an effect.
#[must_use]
pub fn map_edits(core: &CoreState) -> usize {
    core.marks().tail.iter().filter(|(k, _)| matches!(k, crate::keys::MarkKey::MapEdit(_))).count()
}

/// The facts of the head of the aftermath for `seat`'s action in `system`.
#[must_use]
pub fn head_facts(core: &CoreState, k: &Kernel, codes: &TacticalCodes, seat: Seat, system: System, notes: &NoteHold, skip_to_commit: bool) -> HeadFacts {
    let cannon = CannonFacts {
        active: seat,
        system,
        // A live driver always carries the timing machinery.
        announce: true,
        // `supply::staging_enabled`: every faction of the agreed tables has a module.
        staging: true,
        reaching: REACHING.to_vec(),
        mapped: core.map().galaxy.is_some(),
        kind: codes.space_cannon,
        cause: codes.space_cannon_cause,
        casualty: codes.casualty,
        commander: codes.commander,
    };
    HeadFacts { player: seat, system, limits: limits(core, k, seat, system), cannon, notes: notes.clone(), skip_to_commit, pre_roll: codes.pre_roll }
}

