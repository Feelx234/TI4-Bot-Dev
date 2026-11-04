//! The movement facts of the host's plan view on the typed core (D2-8f): one seat's rules for moving into one system, with the cards in play
//! (what `movement::MovementRules::with_laws` + `action_cards::apply_movement_effects` build on the legacy side), and the reads the view makes
//! of them: the route of a ship, the rifts it leaves, the routes it may take, whether it may leave, and what its hold could take aboard
//! (`transit::CargoWindow`). Every answer comes from `query::reach`, `rules::tactical::routes` and `query::transit`; the legacy functions are
//! the oracle of `tests/movement_facts.rs`, never called here.

use std::rc::Rc;

use ti4_content::galaxy::Galaxy;
use ti4_engine_core::generated::{Planet, System};
use ti4_engine_core::movement::CoreRules;
use ti4_engine_core::query::transit::Hold;
use ti4_engine_core::rules::tactical::routes as core_routes;
use ti4_engine_core::state::Piece;
use ti4_engine_core::{Seat, SystemIx};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use super::adapter::{Position, SeatReach, unit_ix};
use super::tactical::MoveRoute;
use crate::generated::IdStr;
use crate::ties::core_adapter::{system_id, system_ix};

/// What a hold could pick up: where the unit is (the system it is picked up from, the planet it stands on), its type, and its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pool {
    pub system: String,
    pub planet: Option<String>,
    pub unit: String,
    pub damaged: bool,
    pub galvanized: bool,
}

/// One seat's movement rules for moving into one system, on a typed position.
pub struct MoveView {
    pos: Rc<Position>,
    seat: Seat,
    active: SystemIx,
    rules: Rc<CoreRules>,
}

impl MoveView {
    /// The rules of `player` for moving into `active`, cards in play included. `None` when the core cannot hold the position or an id.
    #[must_use]
    pub fn new(state: &GameState, sources: SourceSet, galaxy: &Galaxy, player: &PlayerId, active: &SystemId) -> Option<Self> {
        let mut pos = Position::new_owned(state, sources, galaxy)?;
        let to = system_ix(active.as_str())?;
        pos.set_active_system(to);
        let pos = Rc::new(pos);
        let reach = SeatReach::new(&pos, player, true)?;
        let rules = reach.rules_for(to);
        let seat = pos.seat(player)?;
        Some(Self { pos, seat, active: to, rules })
    }

    /// A ship that rolls for the rifts it leaves: it ignores neither anomalies nor rifts.
    #[must_use]
    pub fn rolls(&self) -> bool {
        !self.rules.anomalies_ignored && !self.rules.rifts_ignored
    }

    /// `MovementRules::path_from_ship`: the systems the ship enters, origin first and the active system last.
    #[must_use]
    pub fn path_from_ship(&self, origin: &str, move_value: i32, ship_type: Option<&str>) -> Option<Vec<String>> {
        let from = system_ix(origin)?;
        let path = self.rules.path_from_ship(from, move_value, ship_type.and_then(unit_ix))?;
        Some(path.into_iter().map(|s| system_id(s).to_owned()).collect())
    }

    /// `transit::rifts_exited`: the gravity rifts a route leaves (every system but the last) that this mover rolls for.
    #[must_use]
    pub fn rifts_exited(&self, path: &[String]) -> Vec<String> {
        let Some((_, before)) = path.split_last() else { return Vec::new() };
        before
            .iter()
            .filter(|id| system_ix(id).is_some_and(|s| self.rules.is_gravity_rift(s) && !self.rules.rift_roll_exempt.contains(s.index())))
            .cloned()
            .collect()
    }

    /// `MovementRules::may_depart`.
    #[must_use]
    pub fn may_depart(&self, origin: &str) -> bool {
        system_ix(origin).is_some_and(|s| self.rules.may_depart(s))
    }

    /// `MovementRules::nebula_caps`.
    #[must_use]
    pub fn nebula_caps(&self, origin: &str) -> bool {
        system_ix(origin).is_some_and(|s| self.rules.nebula_caps(s))
    }

    /// `tactical::ship_routes`: the ways the hull at `index` of the seat's ships in `origin` may take to the active system, the default first;
    /// `answers` of the routes are `option_id`, `option_id|r1`, ...
    #[must_use]
    pub fn routes(&self, origin: &str, index: usize, hull_type: &str, boosts: (bool, bool), option_id: &str) -> Vec<MoveRoute> {
        let (Some(from), Some(kind), Ok(index16)) = (system_ix(origin), unit_ix(hull_type), u16::try_from(index)) else { return Vec::new() };
        let found = core_routes::ship_routes(self.pos.core(), self.pos.kernel(), self.seat, from, index16, kind, boosts);
        let text = |systems: &[System]| systems.iter().map(|s| system_id(*s).to_owned()).collect::<Vec<_>>();
        found
            .iter()
            .enumerate()
            .map(|(number, route)| MoveRoute {
                path: text(&route.path),
                answer: if number == 0 { option_id.to_owned() } else { format!("{option_id}|r{number}") },
                default: route.default,
                gravity_drive: route.gd,
                ionian: route.ionian,
                rifts: text(&route.rifts),
                wormholes: route.wormholes,
                anomalies: text(&route.anomalies),
                pickups: text(&route.pickups),
            })
            .collect()
    }

    /// `CargoWindow::for_ship(..).pending_choice()` read as the pickups it lists: what the hold of a ship of `hull_type` leaving `origin` by
    /// `path` is offered, one entry per kind of unit (interchangeable units once). Empty when the hold offers nothing.
    #[must_use]
    pub fn pools(&self, origin: &str, hull_type: &str, path: &[String]) -> Vec<Pool> {
        let (Some(from), Some(kind)) = (system_ix(origin), unit_ix(hull_type)) else { return Vec::new() };
        let route: Option<Vec<System>> = path.iter().map(|s| system_ix(s)).collect();
        let Some(route) = route else { return Vec::new() };
        let hold = Hold::for_ship(self.pos.core(), self.pos.kernel(), self.seat, from, Piece::new(kind, self.seat), &route);
        let Some((options, _)) = hold.options() else { return Vec::new() };
        options
            .iter()
            .map(|o| Pool {
                system: system_id(o.cand.system).to_owned(),
                planet: o.cand.source.map(|p: Planet| <Planet as IdStr>::as_str(p).to_owned()),
                unit: <ti4_engine_core::generated::Unit as IdStr>::as_str(o.cand.piece.kind()).to_owned(),
                damaged: o.cand.piece.sustained(),
                galvanized: o.cand.piece.galvanized(),
            })
            .collect()
    }

    /// The active system the rules were built for.
    #[must_use]
    pub fn active(&self) -> &'static str {
        system_id(self.active)
    }
}
