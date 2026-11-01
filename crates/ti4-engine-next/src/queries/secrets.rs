//! The secret objectives' reads of the surface (`secrets.rs` of the legacy crate): [`Position`], [`progress`] and
//! [`ship_systems_beside_anomaly`]. The progress is the core's (`query::objectives::secret_progress`, over the same loaded view as the public
//! objectives); the system list is a plain read of the public state and the printed map. A position the core cannot hold (or a secret it
//! reports unsupported) answers `None`.

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::generated::SecretObjective;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, SecretObjectiveId, SystemId};
use ti4_model::state::GameState;

use super::objectives::{self, Imagined, ObjView, RequirementProgress};
use crate::generated::IdStr;

/// What a secret's requirement is evaluated against.
pub struct Position<'a> {
    pub state: &'a GameState,
    pub content: &'a ContentStore,
    pub sources: SourceSet,
    pub player: &'a PlayerId,
    /// The map, when the caller has one. `None` leaves board-shape requirements unmet.
    pub galaxy: Option<&'a Galaxy>,
    /// What an option would add on top of the real board; [`Imagined::NONE`] is the real board.
    pub imagined: Imagined<'a>,
}

/// Exact progress toward a secret objective's requirement, for display. `None` for an event secret (it has no position progress), for an id
/// that is no secret, and for a position the core cannot hold.
///
/// Each call loads the position into the typed core: a caller with several secrets of one position asks
/// [`objectives::held_secrets_of`] once instead.
#[must_use]
pub fn progress(alias: &SecretObjectiveId, position: &Position<'_>) -> Option<RequirementProgress> {
    let obj = <SecretObjective as IdStr>::from_id(alias.as_str())?;
    if !position.content.is_embedded() {
        return None;
    }
    let view = ObjView::build(position.state, position.sources, position.galaxy)?;
    objectives::secret_progress_in(&view, position.player, &position.imagined, obj)
}

/// Systems where the player has a ship, beside an anomaly: the display detail behind `CountFamily::ShipSystemsBesideAnomaly`; its length is
/// the count the requirement checks. `None` without a map.
#[must_use]
pub fn ship_systems_beside_anomaly(position: &Position<'_>) -> Option<Vec<SystemId>> {
    let galaxy = position.galaxy?;
    let types = ti4_content::units::catalogue(position.content, position.sources);
    let systems = ti4_content::galaxy::all_systems(position.content, position.sources);
    let mut found: Vec<SystemId> = position
        .state
        .board
        .iter()
        .filter(|(_, board)| board.units_of(position.player).into_iter().any(|unit| types.get(unit.type_id.as_str()).is_some_and(ti4_content::units::UnitType::is_ship)))
        .map(|(id, _)| id)
        .filter(|here| galaxy.adjacent(here.as_str()).into_iter().any(|other| systems.get(other).is_some_and(ti4_content::galaxy::System::is_anomaly)))
        .cloned()
        .collect();
    found.sort();
    Some(found)
}
