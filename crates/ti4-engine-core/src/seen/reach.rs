//! Movement facts: `Observed::{can_reach, movable_into, rift_exits_into}` and `loadable`.
//!
//! Wrappers of `query::reach` (the rules of one mover, built once per seat and position) and `query::transit`.
//! Everything is memoised per position, as `core_adapter::SeatCache` does for the string `Observed`. Without a map
//! slice (a decision asked without a galaxy) the answers are the string side's: no route, nothing movable, no rift.
//!
//! Not here (bail `D3-bail-03`): `ship_routes_into` (the bot never calls it: `pick_route` stays 0).

use std::rc::Rc;

use super::Seen;
use crate::generated::{System, Unit};
use crate::ids::{Seat, SystemIx};
use crate::movement::CoreRules;
use crate::query::reach::{self, Movable, SeatFleet, SeatReach};
use crate::query::transit::{self, Cand};
use crate::state::Slice;

/// The movement state of a position.
#[derive(Default)]
pub struct MoveCache {
    reach: [[Option<Option<Rc<SeatReach>>>; 2]; 16],
    rules: Vec<(u8, bool, u16, Rc<CoreRules>)>,
    fleets: [Option<Rc<SeatFleet>>; 16],
    movable: Vec<(u8, u16, Rc<[Movable]>)>,
}

impl Seen<'_> {
    fn map_loaded(&self) -> bool {
        self.st().loaded.has(Slice::Map) && self.st().galaxy().is_some()
    }

    fn seat_reach(&self, seat: Seat, effects: bool) -> Option<Rc<SeatReach>> {
        let slot = usize::from(seat.0);
        let flag = usize::from(effects);
        if let Some(known) = &self.memo().borrow().moves.reach[slot][flag] {
            return known.clone();
        }
        let built = SeatReach::new(self.st(), self.kernel(), seat, effects).map(Rc::new);
        self.memo().borrow_mut().moves.reach[slot][flag] = Some(built.clone());
        built
    }

    fn rules_for(&self, seat: Seat, effects: bool, destination: SystemIx) -> Option<Rc<CoreRules>> {
        let key = (seat.0, effects, destination.index() as u16);
        if let Some((_, _, _, r)) = self.memo().borrow().moves.rules.iter().find(|(s, e, d, _)| (*s, *e, *d) == key) {
            return Some(Rc::clone(r));
        }
        let rules = Rc::new(self.seat_reach(seat, effects)?.rules_for(self.st(), self.kernel(), destination));
        self.memo().borrow_mut().moves.rules.push((key.0, key.1, key.2, Rc::clone(&rules)));
        Some(rules)
    }

    fn fleet(&self, seat: Seat) -> Rc<SeatFleet> {
        let slot = usize::from(seat.0);
        if let Some(f) = &self.memo().borrow().moves.fleets[slot] {
            return Rc::clone(f);
        }
        let f = Rc::new(SeatFleet::new(self.st(), self.kernel(), seat));
        self.memo().borrow_mut().moves.fleets[slot] = Some(Rc::clone(&f));
        f
    }

    /// Whether a ship with this printed movement can reach a destination, without the cards in hand
    /// (`Observed::can_reach`); false without a map.
    #[must_use]
    pub fn can_reach(&self, seat: Seat, origin: System, destination: System, move_value: i32) -> bool {
        if !self.map_loaded() {
            return false;
        }
        self.rules_for(seat, false, destination).is_some_and(|rules| rules.can_reach_ship(origin, move_value, None))
    }

    /// Every ship of the seat that could reach `destination` if it were activated (`Observed::movable_into`);
    /// empty without a map.
    #[must_use]
    pub fn movable_into(&self, seat: Seat, destination: System) -> Rc<[Movable]> {
        if !self.map_loaded() {
            return Rc::from(Vec::new());
        }
        let key = (seat.0, destination.index() as u16);
        if let Some((_, _, m)) = self.memo().borrow().moves.movable.iter().find(|(s, d, _)| (*s, *d) == key) {
            return Rc::clone(m);
        }
        let found: Rc<[Movable]> = match self.rules_for(seat, true, destination) {
            Some(rules) => self.fleet(seat).movable(&rules).into(),
            None => Rc::from(Vec::new()),
        };
        self.memo().borrow_mut().moves.movable.push((key.0, key.1, Rc::clone(&found)));
        found
    }

    /// `movable_into(seat, d).len()` for each destination `d` of `destinations`, computed at once: one clone of the rules
    /// retargeted per destination and one route search per group of equal ships (the MCTS features read the count for
    /// every system of the map). Not memoised; 0 for every destination without a map.
    #[must_use]
    pub fn movable_counts(&self, seat: Seat, destinations: &[System]) -> Vec<u16> {
        if !self.map_loaded() || destinations.is_empty() {
            return vec![0; destinations.len()];
        }
        let Some(reach) = self.seat_reach(seat, true) else { return vec![0; destinations.len()] };
        let fleet = self.fleet(seat);
        let mut rules = reach.template().clone();
        destinations
            .iter()
            .map(|d| {
                reach.retarget(self.st(), self.kernel(), &mut rules, *d);
                u16::try_from(fleet.movable_count(&rules)).unwrap_or(u16::MAX)
            })
            .collect()
    }

    /// Gravity rifts a ship of `seat` sailing from `origin` to `destination` would exit (each rolls a die)
    /// (`Observed::rift_exits_into`); 0 without a map.
    #[must_use]
    pub fn rift_exits_into(&self, seat: Seat, origin: System, kind: Unit, boosts: (bool, bool), destination: System) -> usize {
        if !self.map_loaded() {
            return 0;
        }
        // A type outside the scope has no kind row: no move, no rifts.
        let Some(value) = reach::effective_move_value(self.st(), self.kernel(), seat, origin, kind, boosts) else {
            return 0;
        };
        let Some(rules) = self.rules_for(seat, true, destination) else { return 0 };
        if rules.anomalies_ignored || rules.rifts_ignored {
            return 0;
        }
        rules.path_from_ship(origin, value, Some(kind)).map_or(0, |path| rules.rifts_exited(&path))
    }

    /// Every unit of the seat a ship leaving `origin` could carry (`Observed::loadable`).
    #[must_use]
    pub fn loadable(&self, seat: Seat, origin: System) -> Vec<Cand> {
        transit::loadable(self.st(), self.kernel(), seat, origin, false)
    }
}
