//! The agenda effects that place units or hand out a technology (package KV3, lane D): Swords to Plowshares (Against), Rearmament
//! Agreement (For), Colonial Redistribution and Research Grant Reallocation. The string bodies are `agenda_effects::{effect_plowshares,
//! effect_rearmament, effect_redistribution, effect_grant_reallocation}`.

use super::{Resolve, Stage};
use crate::choice::agenda::{Ask as AAsk, Opt as AOpt};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Opt, Opts, Origin, Pick};
use crate::generated::{Agenda, Faction, Planet, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx};
use crate::kernel::{fx, fx_strategy, Next};
use crate::query::board;
use crate::state::CoreState;

use super::super::table_effects::{clear_planet, everyone};

/// Whether the seat's Eidolon Maximum stands anywhere: the Naaz's printed Maximum forbids game effects to place its mechs (the module's
/// `effect_placement_forbidden` hook, read off the board here so that the window does not depend on which hook table it runs on).
fn maximum_on_board(core: &CoreState, seat: Seat) -> bool {
    let is = |p: &crate::state::Piece| p.owner() == seat && p.kind() == Unit::EidolonMaximum;
    core.board().systems().any(|(_, c)| c.units.as_slice().iter().any(is)) || core.board().planets().any(|(_, c)| c.units.as_slice().iter().any(is))
}

impl Resolve {
    /// `agenda_effects::place_infantry`: the seat's own form, refused when the reinforcements hold none (31.4). `false`: the facts the
    /// placement needs are missing (the effect is the string's).
    fn place_infantry(&mut self, sx: &mut Sx<'_>, who: Seat, system: System, planet: Planet) -> bool {
        let Some(roster) = self.facts.as_ref().and_then(|f| f.roster) else {
            self.unsupported = true;
            return false;
        };
        let scope = self.facts.as_ref().map_or(crate::tables::Scope(0), |f| f.scope);
        let Some(unit) = fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, who, UnitBaseType::Infantry) else { return true };
        if crate::query::fleet::remaining_all(sx.state, scope, who, unit) <= 0 {
            return true;
        }
        fx::place_unit(sx.state, who, system, Some(planet), unit);
        true
    }

    /// Swords to Plowshares, Against: everyone arms, one infantry per planet held.
    pub(super) fn arm(&mut self, sx: &mut Sx<'_>) -> Next {
        for seat in everyone(sx.state).as_slice() {
            for (system, planet) in board::controlled_planets(sx.state, *seat).as_slice() {
                if !self.place_infantry(sx, *seat, *system, *planet) {
                    self.stage = Stage::Done;
                    return Next::Done;
                }
            }
        }
        self.stage = Stage::Done;
        Next::Done
    }

    /// The seat's home planet, among the planets it controls (the seat's home first: Creuss's home is not the tile its faction record
    /// names).
    fn home_planet(core: &CoreState, seat: Seat) -> Option<(System, Planet)> {
        let p = core.players().by_seat(seat)?;
        let home = p.home_system.or(p.faction.data().home_system);
        board::controlled_planets(core, seat).as_slice().iter().find(|(system, _)| Some(*system) == home).copied()
    }

    /// Rearmament Agreement, For: a mech onto every seat's home planet. "Each player places 1 mech from their reinforcements": the
    /// seat's own form (`placed_unit`: Jol-Nar's J.N.S. 4.0 `jolnar_mech`, not the plain `mech`), refused when its reinforcements hold
    /// none (31.4) or a faction effect forbids the placement. The string places the PLAIN `mech` for every seat but the Naaz
    /// (`docs/STRING_ENGINE_BUGS.md` S9); the core does not copy that. No roster in the facts: the effect is the string's.
    pub(super) fn mechs(&mut self, sx: &mut Sx<'_>) -> Next {
        let scope = self.facts.as_ref().map_or(crate::tables::Scope(0), |f| f.scope);
        let Some(roster) = self.facts.as_ref().and_then(|f| f.roster) else {
            self.unsupported = true;
            self.stage = Stage::Done;
            return Next::Done;
        };
        for seat in everyone(sx.state).as_slice() {
            let Some((system, planet)) = Self::home_planet(sx.state, *seat) else { continue };
            // Naaz's printed Maximum forbids new mechs and its fourth Eidolon opens Synergy: its mech is its own form.
            let naaz = sx.state.players().by_seat(*seat).is_some_and(|p| p.faction == Faction::TheNaazRokhaAlliance);
            if !naaz {
                let _ = fx_strategy::place_for(sx.state, sx.kernel, scope, roster, *seat, system, Some(planet), UnitBaseType::Mech);
                continue;
            }
            let mech = Unit::Eidolon;
            let k = sx.kernel;
            if k.hooks.economy.effect_placement_forbidden(sx.state, k, *seat, mech) || maximum_on_board(sx.state, *seat) || crate::query::fleet::remaining_all(sx.state, scope, *seat, mech) <= 0 {
                continue;
            }
            fx::place_unit(sx.state, *seat, system, Some(planet), mech);
            fx::stage_naaz_mech_placed(sx.state, *seat, system, mech);
        }
        self.stage = Stage::Done;
        Next::Done
    }

    /// Colonial Redistribution: wipe the elected planet, then hand it to whoever is furthest behind (the controller chooses among the level).
    pub(super) fn settle(&mut self, sx: &mut Sx<'_>, planet: Planet) -> Next {
        if self.stage == Stage::Apply {
            let Some(f) = self.facts.clone() else {
                self.unsupported = true;
                self.stage = Stage::Done;
                return Next::Done;
            };
            let controller = board::control(sx.state, planet);
            let system = board::system_of(sx.state, f.scope, planet);
            clear_planet(sx.state, &f, planet, |_| true, None, Agenda::ColonialRedistribution);
            let (Some(_), Some(_)) = (controller, system) else {
                self.stage = Stage::Done;
                return Next::Done;
            };
            let fewest = sx.state.players().iter().map(|p| p.econ.victory_points).min().unwrap_or(0);
            self.tied = sx.state.players().iter().filter(|p| p.econ.victory_points == fewest).map(|p| p.seat).collect();
            match self.tied.as_slice() {
                [] => {
                    self.stage = Stage::Done;
                    return Next::Done;
                }
                [only] => {
                    let (only, system) = (*only, system.unwrap_or(System::ALL[0]));
                    self.place_infantry(sx, only, system, planet);
                    self.stage = Stage::Done;
                    return Next::Done;
                }
                _ => self.stage = Stage::Asking,
            }
        }
        if self.stage == Stage::Asking { Next::Continue(Ask::Agenda(AAsk::Settler)) } else { Next::Done }
    }

    pub(super) fn settle_question(&self, s: &CoreState, planet: Planet) -> Option<Choice> {
        if self.stage != Stage::Asking {
            return None;
        }
        let controller = board::control(s, planet)?;
        let source = self.facts.as_ref()?.redistribution;
        let mut opts = Opts::default();
        for seat in self.tied.as_slice() {
            opts.push(Opt::Agenda(AOpt::Elect(*seat)));
        }
        Some(super::super::question(controller, AAsk::Settler, Origin::Content(Content::Other(source)), opts))
    }

    pub(super) fn settle_answer(&mut self, sx: &mut Sx<'_>, planet: Planet, pick: Pick) -> Result<(), Illegal> {
        match pick {
            Pick::Opt(Opt::Agenda(AOpt::Elect(seat))) if self.stage == Stage::Asking && self.tied.as_slice().contains(&seat) => {
                let scope = self.facts.as_ref().map_or(crate::tables::Scope(0), |f| f.scope);
                let system = board::system_of(sx.state, scope, planet).ok_or(Illegal)?;
                self.place_infantry(sx, seat, system, planet);
                self.stage = Stage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

/// `Player::spend_token(Fleet)` `times` times (a pool never goes negative).
pub(crate) fn spend_fleet(core: &mut CoreState, who: Seat, times: usize) {
    if let Some(p) = core.players_mut().by_seat_mut(who) {
        for _ in 0..times {
            if p.econ.fleet_tokens <= 0 {
                break;
            }
            p.econ.fleet_tokens -= 1;
        }
    }
}
