//! `combat.rs` of the legacy crate, as far as the product surface uses it: who the space cannon offense fires at, the guns that roll in it
//! (the core's `rules::combat::cannon_guns` over the kernel with every faction module), and the hits a card lets a seat cancel.
//! A position the core cannot hold has no guns.

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_content::units::UnitType;
use ti4_engine_core::generated::Unit as CoreUnit;
use ti4_engine_core::rules::combat::Rd;
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;
use ti4_model::units::Unit;

use super::view::Loaded;
use crate::generated::IdStr;
use crate::ties::core_adapter::{typed, typed_map};

/// The first other player with ships in `system`: whom the active player's space cannon offense fires at.
#[must_use]
pub fn opponent_with_ships(state: &GameState, content: &ContentStore, sources: SourceSet, active: &PlayerId, system: &SystemId) -> Option<PlayerId> {
    let types = ti4_content::units::catalogue(content, sources);
    state.system_state(system).units.iter().find(|unit| &unit.owner != active && types.get(unit.type_id.as_str()).is_some_and(UnitType::is_ship)).map(|unit| unit.owner.clone())
}

/// Hits a card has let this seat cancel in the current combat round (Shields Holding), scoped to the round the card names.
#[must_use]
pub fn cancellable_hits(state: &GameState, player: &PlayerId) -> usize {
    state.player(player).and_then(|seat| seat.cancel_hits_round).filter(|(round, _)| *round == state.combat_round_seq).map_or(0, |(_, hits)| hits)
}

/// The guns that roll in the space cannon offense of a system, when a player activates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CannonGuns {
    /// The units, in the order in which they roll.
    pub units: Vec<Unit>,
    /// SPACE CANNON of an attachment: the owner, the planet, the value and the number of dice.
    pub attachments: Vec<(PlayerId, PlanetId, u32, usize)>,
}

/// Whether a card lets the unit fire its SPACE CANNON into adjacent systems (PDS II, the Indomitus mech and flagship, Hel-Titan II).
fn reaches_adjacent(kind: UnitType<'_>) -> bool {
    let Some(ability) = kind.record().text("ability") else {
        return false;
    };
    let ability = ability.to_ascii_lowercase();
    ability.contains("space cannon against ships that are") && ability.contains("adjacent")
}

/// Who may fire in the space cannon offense of `system` when `active` activates it. It rolls nothing and changes nothing. Empty when the
/// core cannot hold the position.
#[must_use]
pub fn space_cannon_guns(state: &GameState, content: &ContentStore, sources: SourceSet, system: &SystemId, active: &PlayerId, galaxy: Option<&Galaxy>) -> CannonGuns {
    space_cannon_guns_in(state, content, sources, system, active, galaxy).unwrap_or(CannonGuns { units: Vec::new(), attachments: Vec::new() })
}

fn space_cannon_guns_in(state: &GameState, content: &ContentStore, sources: SourceSet, system: &SystemId, active: &PlayerId, galaxy: Option<&Galaxy>) -> Option<CannonGuns> {
    let view = Loaded::of(state, None, sources, SliceSet::of_without(SliceSet::ALL, Slice::Map))?;
    let (seat, sys) = (view.seat(active)?, typed::<ti4_engine_core::generated::System>(system.as_str())?);
    let types = ti4_content::units::catalogue(content, sources);
    let reaching: Vec<CoreUnit> = types.iter().filter(|(_, kind)| reaches_adjacent(**kind)).filter_map(|(id, _)| typed::<CoreUnit>(id)).collect();
    let adjacency = match galaxy {
        Some(g) => Some(typed_map(g)?.adjacency),
        None => None,
    };
    let rd = Rd::new(view.core(), view.kernel()).with_blanked(view.blanked());
    let units = ti4_engine_core::rules::combat::cannon_guns::unit_guns(rd, sys, seat, adjacency.as_deref(), &reaching)
        .into_iter()
        .map(|piece| {
            let mut unit = Unit::new(UnitTypeId::new(piece.kind().as_str()), view.player(piece.owner())?);
            unit.sustained_damage = piece.sustained();
            unit.galvanized = piece.galvanized();
            Some(unit)
        })
        .collect::<Option<Vec<_>>>()?;
    let attachments = ti4_engine_core::rules::combat::cannon_guns::attachment_guns(rd, sys, seat)
        .into_iter()
        .map(|(owner, planet, value, count)| Some((view.player(owner)?, PlanetId::new(planet.as_str()), value, count)))
        .collect::<Option<Vec<_>>>()?;
    Some(CannonGuns { units, attachments })
}
