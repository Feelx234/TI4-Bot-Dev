//! The agenda effects that ask a question (package B9): Seed of an Empire (the speaker names one of the tied
//! seats, 8.18) and Homeland Defense Act, Against (each seat with several PDS picks the one to destroy).

use super::{Resolve, Stage};
use crate::choice::agenda::{Ask as AAsk, Opt as AOpt};
use crate::choice::generic::{Content, Rule};
use crate::choice::{Ask, Choice, Opt, Opts, Origin, Pick};
use crate::containers::SmallList;
use crate::generated::{Agenda, Planet, System, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx};
use crate::kernel::Next;
use crate::state::CoreState;

/// A PDS a seat holds: its system, its planet and its position in the planet's unit list.
pub type Structure = (System, Planet, u8);

/// `agenda_effects::structures_of(.., "pds")`: the seat's PDS in board order (system, then planet, then position).
#[must_use]
pub fn pds_of(core: &CoreState, scope: crate::tables::Scope, who: Seat) -> SmallList<Structure, 16> {
    let mut out: SmallList<Structure, 16> = SmallList::new();
    for planet in Planet::ALL {
        let Some(system) = crate::query::board::home_system(core, planet) else { continue };
        for (at, piece) in core.board().planet(planet).units.as_slice().iter().enumerate() {
            let data = piece.kind().data();
            if piece.owner() == who && data.base_type == UnitBaseType::Pds && scope.0 & crate::tables::source_bit(data.source) != 0 {
                out.push((system, planet, u8::try_from(at).unwrap_or(u8::MAX)));
            }
        }
    }
    out.as_mut_slice().sort_by_key(|(s, p, at)| (s.index(), p.index(), *at));
    out
}

impl Resolve {
    /// Seed of an Empire: the seat with the most (For) or fewest (Against) points gains one; a tie is the speaker's
    /// to break.
    pub(super) fn seed(&mut self, sx: &mut Sx<'_>, agenda: Agenda, high: bool) -> Next {
        if self.stage == Stage::Apply {
            let mut points: SmallList<(Seat, i16), 8> = SmallList::new();
            for seat in sx.state.seats().players() {
                if let Some(p) = sx.state.players().by_seat(*seat) {
                    points.push((*seat, p.econ.victory_points));
                }
            }
            points.as_mut_slice().sort_by_key(|(s, _)| s.0);
            let target = if high { points.as_slice().iter().map(|(_, n)| *n).max() } else { points.as_slice().iter().map(|(_, n)| *n).min() };
            let Some(target) = target else {
                self.stage = Stage::Done;
                return Next::Done;
            };
            self.tied = points.as_slice().iter().filter(|(_, n)| *n == target).map(|(s, _)| *s).collect();
            if let [only] = self.tied.as_slice() {
                Self::point(sx.state, agenda, *only, 1);
                self.stage = Stage::Done;
                return Next::Done;
            }
            self.stage = Stage::Asking;
        }
        match self.stage {
            Stage::Asking => Next::Continue(Ask::Agenda(AAsk::NameTied)),
            _ => Next::Done,
        }
    }

    pub(super) fn seed_question(&self, s: &CoreState) -> Option<Choice> {
        if self.stage != Stage::Asking {
            return None;
        }
        let mut opts = Opts::default();
        for seat in self.tied.as_slice() {
            opts.push(Opt::Agenda(AOpt::Elect(*seat)));
        }
        Some(super::super::question(s.meta().speaker, AAsk::NameTied, Origin::Rule(Rule::R8_18), opts))
    }

    pub(super) fn seed_answer(&mut self, sx: &mut Sx<'_>, agenda: Agenda, pick: Pick) -> Result<(), Illegal> {
        match pick {
            Pick::Opt(Opt::Agenda(AOpt::Elect(seat))) if self.stage == Stage::Asking && self.tied.as_slice().contains(&seat) => {
                Self::point(sx.state, agenda, seat, 1);
                self.stage = Stage::Done;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }

    /// Homeland Defense Act, Against: each seat in seating order destroys one of its PDS (asked when it holds
    /// several).
    pub(super) fn defense(&mut self, sx: &mut Sx<'_>) -> Next {
        let scope = self.facts.as_ref().map_or(crate::tables::Scope(0), |f| f.scope);
        let order: SmallList<Seat, 8> = SmallList::from_slice(sx.state.seats().seating_order());
        loop {
            let Some(seat) = order.as_slice().get(usize::from(self.cursor)).copied() else {
                self.stage = Stage::Done;
                return Next::Done;
            };
            let held = pds_of(sx.state, scope, seat);
            match held.as_slice() {
                [] => self.cursor += 1,
                [only] => {
                    Self::destroy_structure(sx.state, *only);
                    self.cursor += 1;
                }
                _ => {
                    self.stage = Stage::Asking;
                    return Next::Continue(Ask::Agenda(AAsk::Structure));
                }
            }
        }
    }

    fn destroy_structure(core: &mut CoreState, (_, planet, at): Structure) {
        let units = &mut core.board_mut().planet_mut(planet).units;
        if usize::from(at) < units.len() {
            units.remove(usize::from(at));
        }
    }

    pub(super) fn defense_question(&self, s: &CoreState) -> Option<Choice> {
        if self.stage != Stage::Asking {
            return None;
        }
        let scope = self.facts.as_ref().map_or(crate::tables::Scope(0), |f| f.scope);
        let seat = s.seats().seating_order().get(usize::from(self.cursor)).copied()?;
        let code = self.facts.as_ref()?.defense_act;
        let mut opts = Opts::default();
        for (index, (system, planet, _)) in pds_of(s, scope, seat).as_slice().iter().enumerate() {
            opts.push(Opt::Agenda(AOpt::Structure { index: u8::try_from(index).unwrap_or(u8::MAX), planet: *planet, system: *system }));
        }
        Some(super::super::question(seat, AAsk::Structure, Origin::Content(Content::Other(code)), opts))
    }

    pub(super) fn defense_answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let scope = self.facts.as_ref().map_or(crate::tables::Scope(0), |f| f.scope);
        let Some(seat) = sx.state.seats().seating_order().get(usize::from(self.cursor)).copied() else { return Err(Illegal) };
        match pick {
            Pick::Opt(Opt::Agenda(AOpt::Structure { index, planet, system })) if self.stage == Stage::Asking => {
                let held = pds_of(sx.state, scope, seat);
                let chosen = *held.as_slice().get(usize::from(index)).ok_or(Illegal)?;
                if chosen.0 != system || chosen.1 != planet {
                    return Err(Illegal);
                }
                Self::destroy_structure(sx.state, chosen);
                self.cursor += 1;
                self.stage = Stage::Apply;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}
