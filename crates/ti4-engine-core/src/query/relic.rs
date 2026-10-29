//! Relics held by a seat (`relics.rs`): who holds which, and the standing grants of the passive ones.
//!
//! Package B5. Pure reads over `&CoreState` (slice `Players`); each function names the string
//! function it answers and returns the same value. The relic deck and its draws are not here (a
//! draw is a mutation with an rng position; package B5 ports reads only, like A1, A3 and A4).

use crate::generated::{ExploreType, Relic};
use crate::ids::Seat;
use crate::state::{CoreState, FragmentCounts};

fn cards(core: &CoreState, seat: Seat) -> Option<&crate::state::PlayerCards> {
    core.players().by_seat(seat).map(|p| &p.cards)
}

/// `relics::holds`.
#[must_use]
pub fn holds(core: &CoreState, seat: Seat, relic: Relic) -> bool {
    cards(core, seat).is_some_and(|c| c.relics.as_slice().contains(&relic))
}

/// `relics::ready`: held and not exhausted.
#[must_use]
pub fn ready(core: &CoreState, seat: Seat, relic: Relic) -> bool {
    holds(core, seat, relic) && cards(core, seat).is_some_and(|c| !c.relics_exhausted.contains(relic))
}

/// The number of fragment types the seat owns at least one of (`seat.relic_fragments` values > 0).
#[must_use]
pub fn fragment_kinds(fragments: &FragmentCounts) -> usize {
    [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial]
        .into_iter()
        .filter(|k| fragments.get(*k).is_some_and(|n| n > 0))
        .count()
}

/// `relics::triad_value`: 3 plus the number of different fragment types, `None` when not held.
#[must_use]
pub fn triad_value(core: &CoreState, seat: Seat) -> Option<i64> {
    if !holds(core, seat, Relic::TheTriad) {
        return None;
    }
    let kinds = cards(core, seat).map_or(0, |c| fragment_kinds(&c.fragments));
    Some(3 + i64::try_from(kinds).unwrap_or(0))
}

/// `relics::substitutes_strategy_token` (Scepter of Emelpar).
#[must_use]
pub fn substitutes_strategy_token(core: &CoreState, seat: Seat) -> bool {
    holds(core, seat, Relic::ScepterOfEmelpar)
}

/// `relics::grants_sustain` (Metali Void Shielding).
#[must_use]
pub fn grants_sustain(core: &CoreState, seat: Seat) -> bool {
    holds(core, seat, Relic::MetaliVoidShielding)
}

/// `relics::extra_barrage` (Metali Void Armaments): ANTI-FIGHTER BARRAGE 6 (x3).
#[must_use]
pub fn extra_barrage(core: &CoreState, seat: Seat) -> Option<(u32, usize)> {
    holds(core, seat, Relic::MetaliVoidArmaments).then_some((6, 3))
}

/// `relics::space_dock_cannon` (Lightrail Ordnance): SPACE CANNON 5 (x2).
#[must_use]
pub fn space_dock_cannon(core: &CoreState, seat: Seat) -> Option<(i64, i64)> {
    holds(core, seat, Relic::LightrailOrdnance).then_some((5, 2))
}

/// `relics::prerequisite_waivers` (The Prophet's Tears): one while held.
#[must_use]
pub fn prerequisite_waivers(core: &CoreState, seat: Seat) -> usize {
    usize::from(holds(core, seat, Relic::TheProphetsTears))
}

/// `relics::secret_objective_bonus` (The Obsidian).
#[must_use]
pub fn secret_objective_bonus(core: &CoreState, seat: Seat) -> usize {
    usize::from(holds(core, seat, Relic::TheObsidian))
}

/// `relics::ignores_gravity_rifts` (Circlet of the Void).
#[must_use]
pub fn ignores_gravity_rifts(core: &CoreState, seat: Seat) -> bool {
    holds(core, seat, Relic::CircletOfTheVoid)
}

/// The arm of `relics::use_relic` a relic takes: the dispatch of the component action of a relic (the effect behind
/// an arm is the area's windows and writes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UseArm {
    /// The seat does not hold the relic (`Used::NotHeld`).
    NotHeld,
    EnigmaticDevice,
    MawOfWorlds,
    Codex,
    StellarConverter,
    TitanPrototype,
    DynamisCore,
    BookOfLatvinia,
    SilverFlame,
    /// Held, but the engine has no handler (`Used::Unresolved`).
    Unresolved,
}

/// The relics with a printed ACTION the engine resolves, in the order of `relics::action_aliases` (alphabetical by
/// alias, not used for ordering anywhere: [`action_relics`] keeps the order of the hand).
#[must_use]
pub const fn has_action(relic: Relic) -> bool {
    matches!(
        relic,
        Relic::BookOfLatvinia
            | Relic::TheCodex
            | Relic::DynamisCore
            | Relic::EnigmaticDevice
            | Relic::MawOfWorlds
            | Relic::StellarConverter
            | Relic::JRXS455O
            | Relic::TheSilverFlame
    )
}

/// `relics::use_relic`'s `match`: which arm `relic` takes for `seat`.
#[must_use]
pub fn use_arm(core: &CoreState, seat: Seat, relic: Relic) -> UseArm {
    if !holds(core, seat, relic) {
        return UseArm::NotHeld;
    }
    match relic {
        Relic::EnigmaticDevice => UseArm::EnigmaticDevice,
        Relic::MawOfWorlds => UseArm::MawOfWorlds,
        Relic::TheCodex => UseArm::Codex,
        Relic::StellarConverter => UseArm::StellarConverter,
        Relic::JRXS455O => UseArm::TitanPrototype,
        Relic::DynamisCore => UseArm::DynamisCore,
        Relic::BookOfLatvinia => UseArm::BookOfLatvinia,
        Relic::TheSilverFlame => UseArm::SilverFlame,
        _ => UseArm::Unresolved,
    }
}

/// The relics `relics::available_actions` offers a use of, in hand order: held, with a printed ACTION the engine
/// resolves, and able to resolve (22.3). `enigmatic_resolvable` and `stellar_resolvable` are the two facts of other
/// areas (the research prerequisites and six resources; a target in range of the Converter), asked only for a seat
/// that holds the relic.
#[must_use]
pub fn action_relics(core: &CoreState, seat: Seat, enigmatic_resolvable: bool, stellar_resolvable: bool) -> crate::containers::SmallList<Relic, 8> {
    let mut out = crate::containers::SmallList::new();
    let Some(c) = cards(core, seat) else { return out };
    for relic in c.relics.as_slice() {
        let resolvable = match relic {
            Relic::EnigmaticDevice => enigmatic_resolvable,
            Relic::StellarConverter => stellar_resolvable,
            _ => true,
        };
        if has_action(*relic) && resolvable {
            out.push(*relic);
        }
    }
    out
}
