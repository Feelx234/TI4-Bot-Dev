//! The core halves of the supply mutations (package B3, `Area::Production`): spending trade goods, capturing
//! and returning units. Each function runs `ti4_engine_core::rules::production::supply` on the position and
//! gives back the string twin's result, or `None` when the position holds something the core does not carry (a
//! table of more than 16 owners, a faction whose production hooks are still string, an id the tables do not
//! know): the string result stands then and nothing is compared.

use ti4_engine_core::generated::{Planet, System, Unit, UnitBaseType};
use ti4_engine_core::rules::production::supply as rules;
use ti4_engine_core::state::{Piece, Slice};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;
use ti4_model::units::Unit as StrUnit;

use crate::core_adapter::typed;
use crate::core_sync::{with_core, Footprint, Sources};
use crate::dual_window::tables;
use crate::generated::IdStr;

const GOODS: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Marks], &[Slice::Players]);
const BOARD: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board], &[Slice::Players, Slice::Board]);
const SHEET: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Marks], &[Slice::Players]);
const BOX: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board], &[Slice::Players]);

/// Whether the production hooks the typed supply writes need are all on the core table: the Crimson hero card
/// holds ships off the board (`held_on_card`) and the Last Bastion has docks the typed reads do not carry.
fn ported(state: &GameState) -> bool {
    !crate::production_view::unported_economy(state)
}

/// `supply::spend_goods`.
#[must_use]
pub fn spend_goods(state: &mut GameState, payer: &PlayerId, amount: i32) -> Option<bool> {
    if !ported(state) {
        return None;
    }
    let (mut cx, r) = tables(state)?;
    let who = r.seat(payer.as_str());
    let kernel = crate::production_view::kernel();
    with_core(state, &mut cx, Sources::default(), GOODS, |core, _| match who {
        Some(who) => rules::spend_goods(core, kernel, who, amount),
        // A payer outside the table owns nothing: only a free spend succeeds.
        None => amount <= 0,
    })
    .ok()
}

/// `supply::capture_from_board`.
#[must_use]
pub fn capture_from_board(state: &mut GameState, captor: &PlayerId, system: &SystemId, planet: Option<&PlanetId>, unit: &StrUnit) -> Option<bool> {
    if !ported(state) {
        return None;
    }
    let (mut cx, r) = tables(state)?;
    let sys = typed::<System>(system.as_str())?;
    let planet = match planet {
        Some(p) => Some(typed::<Planet>(p.as_str())?),
        None => None,
    };
    let kind = typed::<Unit>(unit.type_id.as_str())?;
    let (Some(captor), Some(owner)) = (r.seat(captor.as_str()), r.seat(unit.owner.as_str())) else {
        // A seat outside the table: the string refuses.
        return Some(false);
    };
    let piece = Piece::new(kind, owner).with_sustained(unit.sustained_damage).with_galvanized(unit.galvanized);
    with_core(state, &mut cx, Sources::default(), BOARD, |core, _| rules::capture_from_board(core, captor, sys, planet, piece)).ok()
}

/// `supply::capture_from_reinforcements`.
#[must_use]
pub fn capture_from_reinforcements(state: &mut GameState, sources: SourceSet, captor: &PlayerId, owner: &PlayerId, unit: &UnitTypeId) -> Option<bool> {
    if !ported(state) {
        return None;
    }
    let (mut cx, r) = tables(state)?;
    let kind = typed::<Unit>(unit.as_str())?;
    let (Some(captor), Some(owner)) = (r.seat(captor.as_str()), r.seat(owner.as_str())) else {
        return Some(false);
    };
    let scope = crate::core_adapter::scope(sources);
    with_core(state, &mut cx, Sources::default(), BOX, |core, _| rules::capture_from_reinforcements(core, scope, captor, owner, kind)).ok()
}

/// `supply::return_captured`: the unit that went back, text.
#[must_use]
pub fn return_captured(state: &mut GameState, captor: &PlayerId, owner: &PlayerId, base_type: &str) -> Option<Option<UnitTypeId>> {
    if !ported(state) {
        return None;
    }
    let (mut cx, r) = tables(state)?;
    let base = typed::<UnitBaseType>(base_type)?;
    let (Some(captor), Some(owner)) = (r.seat(captor.as_str()), r.seat(owner.as_str())) else {
        return Some(None);
    };
    let kernel = crate::production_view::kernel();
    with_core(state, &mut cx, Sources::default(), SHEET, |core, _| rules::return_captured(core, kernel, captor, owner, base))
        .ok()
        .map(|unit| unit.map(|u| UnitTypeId::new(u.as_str())))
}

/// `supply::release_all_captured`.
#[must_use]
pub fn release_all_captured(state: &mut GameState, captor: &PlayerId) -> Option<usize> {
    if !ported(state) {
        return None;
    }
    let (mut cx, r) = tables(state)?;
    let captor = r.seat(captor.as_str());
    with_core(state, &mut cx, Sources::default(), SHEET, |core, _| captor.map_or(0, |c| rules::release_all_captured(core, c))).ok()
}
