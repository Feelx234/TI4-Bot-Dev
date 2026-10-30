//! The placement of units from the reinforcements (`action_cards::place_units_choosing`, the planets a seat controls) and the
//! promissory notes that use it, as [`Window`]s (package W14, lent-bodies lane 5): Military Support.
//!
//! The window reads the position and asks the one question; the adapter vouches for what the core does not hold (the form the seat
//! places with its upgrades and faction overrides, a faction rule that forbids placing it, the sources in play). Everything the
//! answer changes happens in `answer`: the units placed and, for a note, what the note does after them.

use crate::choice::generic::Content;
use crate::choice::promissory::{Ask as PAsk, Opt as POpt, Subtype as PSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Planet, PromissoryNote, System, Unit};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next, fx};
use crate::query::{board, fleet, laws, reach};
use crate::state::{Code, CoreState, EventKind, EventRec, Key, NoteKey, Payload, Val};
use crate::tables::Scope;

/// What the window does after the placement (or when there is nothing to place).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    /// Nothing more.
    Nothing,
    /// Military Support: "Remove 1 token from the Sol player's strategy pool, if able, and return it to their reinforcements.
    /// Then, ... return this card to the Sol player." The strategy token of `owner` is spent and announced (`reason` is the interned
    /// `military_support`), and the note of `owner`'s faction goes home.
    MilitarySupport { owner: Seat, reason: Code },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PStage {
    Start,
    Asking,
    Done,
}

/// "place up to `count` `unit` on any planet `who` controls" (optionally within one system), the units of the placer's
/// reinforcements: `optional` adds the decline of a "may". All the units go to the one planet chosen; one planet that the
/// box lets them all go to is no decision when the placement is not optional. A planet adds no fleet or capacity load, so the
/// pool and capacity limits never cut a placement on a planet. Nothing is asked and nothing is placed when `unit` is `None` (the
/// seat places nothing, or a rule forbids it), when the box holds none, or when no planet qualifies; the `then` clause happens
/// either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceUnits {
    pub who: Seat,
    unit: Option<Unit>,
    count: u8,
    optional: bool,
    within: Option<System>,
    scope: Scope,
    ability: Code,
    then: Then,
    stage: PStage,
    /// How many units arrived.
    pub placed: u8,
}

impl PlaceUnits {
    /// `unit`: the form the seat places (`placed_unit_id`), `None` when it places nothing or a faction rule forbids placing it
    /// (`effect_placement_forbidden`). `ability` is the interned text the question names as its source.
    #[must_use]
    #[allow(clippy::too_many_arguments, reason = "one placement question: who, what, how many, where, optional, which card, what follows")]
    pub const fn new(who: Seat, unit: Option<Unit>, count: u8, within: Option<System>, optional: bool, scope: Scope, ability: Code, then: Then) -> Self {
        Self { who, unit, count, optional, within, scope, ability, then, stage: PStage::Start, placed: 0 }
    }

    /// How many units the box lets the placer put on the board now (`supply::allowed`).
    fn wanted(&self, s: &CoreState) -> u8 {
        let Some(unit) = self.unit else { return 0 };
        let left = fleet::remaining_all(s, self.scope, self.who, unit);
        u8::try_from(left.clamp(0, i64::from(self.count))).unwrap_or(0)
    }

    /// The planets the placer controls, in the order of the string's `placement_spots` (system id, then planet id), not demilitarized.
    fn spots(&self, s: &CoreState) -> SmallList<(System, Planet), 24> {
        let blanked = reach::blanked(s);
        board::controlled_planets(s, self.who)
            .as_slice()
            .iter()
            .copied()
            .filter(|(system, planet)| self.within.is_none_or(|only| only == *system) && !laws::planet_is_demilitarized(s, blanked, *planet))
            .collect()
    }

    fn ctx(&self) -> Ctx {
        Ctx {
            origin: Origin::FactionAbility(Content::Other(self.ability)),
            subtype: Subtype::Promissory(PSub::PlaceUnitsFromReinforcements),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        }
    }

    fn follow(&self, s: &mut CoreState) {
        match self.then {
            Then::Nothing => {}
            Then::MilitarySupport { owner, reason } => {
                if fx::spend_strategic_token(s, owner) {
                    // `supply::note_strategy_token_spent(.., "military_support")`.
                    fx::stage(
                        s,
                        EventRec {
                            id: 0,
                            kind: EventKind::StrategyTokenSpent,
                            payload: Payload::new().with(Key::Player, Val::Seat(owner)).with(Key::Reason, Val::Code(reason)),
                            cancelled: false,
                        },
                    );
                }
                if let Some(faction) = s.players().by_seat(owner).map(|p| p.faction) {
                    fx::give_back_note(s, NoteKey { note: PromissoryNote::MilitarySupport, faction });
                }
            }
        }
    }
}

impl Window for PlaceUnits {
    type Stage = PStage;

    fn stage(&self) -> PStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == PStage::Start {
            let wanted = self.wanted(sx.state);
            let spots = self.spots(sx.state);
            if wanted == 0 || spots.as_slice().is_empty() {
                self.follow(sx.state);
                self.stage = PStage::Done;
            } else if spots.as_slice().len() == 1 && !self.optional {
                let (system, planet) = spots.as_slice()[0];
                self.put(sx.state, system, planet, wanted);
                self.follow(sx.state);
                self.stage = PStage::Done;
            } else {
                self.stage = PStage::Asking;
            }
        }
        match (self.stage, self.unit) {
            (PStage::Asking, Some(unit)) => Next::Continue(Ask::Promissory(PAsk::PlaceUnits { unit, fit: self.wanted(sx.state) })),
            _ => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        let unit = self.unit.filter(|_| self.stage == PStage::Asking)?;
        let mut opts: Opts = self.spots(s).as_slice().iter().map(|(system, planet)| Opt::Promissory(POpt::Spot(*system, *planet))).collect();
        if self.optional {
            opts.push(Opt::Decline);
        }
        Some(Choice { actor: self.who, ask: Ask::Promissory(PAsk::PlaceUnits { unit, fit: self.wanted(s) }), ctx: self.ctx(), opts })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != PStage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) if self.optional => {}
            Pick::Opt(Opt::Promissory(POpt::Spot(system, planet))) if self.spots(sx.state).as_slice().contains(&(system, planet)) => {
                let fit = self.wanted(sx.state);
                self.put(sx.state, system, planet, fit);
            }
            _ => return Err(Illegal),
        }
        self.follow(sx.state);
        self.stage = PStage::Done;
        Ok(())
    }
}

impl PlaceUnits {
    fn put(&mut self, s: &mut CoreState, system: System, planet: Planet, count: u8) {
        let Some(unit) = self.unit else { return };
        for _ in 0..count {
            fx::place_unit(s, self.who, system, Some(planet), unit);
        }
        self.placed = count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, DriverError, Status};
    use crate::generated::{Attachment, Faction, UnitBaseType};
    use crate::kernel::Resolver;
    use crate::kernel::stage::AbilityTable;
    use crate::state::{Player, Players};

    fn table() -> CoreState {
        let mut s = CoreState::all_loaded();
        let list = (0..3u8).map(|i| Player::new(Seat(i), Faction::from_index(usize::from(i)).unwrap())).collect();
        s.players = Players::from_vec(list);
        s.seats = crate::state::Seats::new(&[Seat(0), Seat(1), Seat(2)], &[Seat(0), Seat(1), Seat(2)], 3);
        s
    }

    fn driver(w: PlaceUnits) -> Driver<PlaceUnits> {
        Driver::new(w, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1)
    }

    fn unit_of(base: UnitBaseType) -> Unit {
        Unit::ALL.iter().copied().find(|u| u.data().base_type == base && u.data().faction.is_none()).expect("a plain unit")
    }

    /// Three planets in three systems, controlled by seat 0 (the last by seat 1).
    fn planets() -> [(System, Planet); 3] {
        let mut found: Vec<(System, Planet)> = Planet::ALL.iter().copied().filter_map(|p| p.data().system.map(|s| (s, p))).step_by(40).take(3).collect();
        found.sort_unstable();
        [found[0], found[1], found[2]]
    }

    fn control(s: &mut CoreState, owned: usize) {
        for (i, (_, planet)) in planets().iter().enumerate() {
            s.board_mut().planet_mut(*planet).control = Some(if i < owned { Seat(0) } else { Seat(1) });
        }
    }

    fn count(s: &CoreState, planet: Planet, unit: Unit) -> usize {
        s.board().planet(planet).units.as_slice().iter().filter(|p| p.owner() == Seat(0) && p.kind() == unit).count()
    }

    fn window(unit: Unit, optional: bool, then: Then) -> PlaceUnits {
        PlaceUnits::new(Seat(0), Some(unit), 2, None, optional, Scope(u32::MAX), Code(0), then)
    }

    #[test]
    fn a_lone_planet_takes_the_units_without_a_question_unless_the_placement_is_optional() {
        let (k, infantry) = (Kernel::plain(), unit_of(UnitBaseType::Infantry));
        let mut s = table();
        control(&mut s, 1);
        let mut d = driver(window(infantry, false, Then::Nothing));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!((count(&s, planets()[0].1, infantry), d.window.placed), (2, 2));
        let mut s = table();
        control(&mut s, 1);
        let mut d = driver(window(infantry, true, Then::Nothing));
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("a \"may\" is asked with one spot") };
        assert_eq!(c.opts.as_slice(), &[Opt::Promissory(POpt::Spot(planets()[0].0, planets()[0].1)), Opt::Decline]);
        assert_eq!(count(&s, planets()[0].1, infantry), 0, "nothing before the answer");
    }

    #[test]
    fn several_planets_are_offered_in_system_order_and_only_a_listed_one_is_taken() {
        let (k, infantry) = (Kernel::plain(), unit_of(UnitBaseType::Infantry));
        let mut s = table();
        control(&mut s, 2);
        let mut d = driver(window(infantry, false, Then::Nothing));
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(c.ask, Ask::Promissory(PAsk::PlaceUnits { unit: infantry, fit: 2 }));
        assert_eq!(c.opts.as_slice(), &[Opt::Promissory(POpt::Spot(planets()[0].0, planets()[0].1)), Opt::Promissory(POpt::Spot(planets()[1].0, planets()[1].1))], "no decline when it is not a may");
        // The planet of the other seat, a decline of a placement that is not optional, and a planet in the wrong system.
        for bad in [Pick::Opt(Opt::Promissory(POpt::Spot(planets()[2].0, planets()[2].1))), Pick::Opt(Opt::Decline), Pick::Opt(Opt::Promissory(POpt::Spot(planets()[1].0, planets()[0].1)))] {
            assert_eq!(d.resume(&mut s, &k, bad), Err(DriverError::Illegal), "{bad:?}");
        }
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(c.opts.as_slice()[1])).unwrap(), Status::Done);
        assert_eq!((count(&s, planets()[0].1, infantry), count(&s, planets()[1].1, infantry)), (0, 2));
    }

    #[test]
    fn a_decline_places_nothing_and_a_system_limit_hides_the_other_systems() {
        let (k, infantry) = (Kernel::plain(), unit_of(UnitBaseType::Infantry));
        let mut s = table();
        control(&mut s, 2);
        let mut d = driver(window(infantry, true, Then::Nothing));
        d.run(&mut s, &k).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
        assert_eq!((count(&s, planets()[0].1, infantry), d.window.placed), (0, 0));
        // Within one system: only its planet, and (not optional) no question at all.
        let mut s = table();
        control(&mut s, 2);
        let mut within = PlaceUnits::new(Seat(0), Some(infantry), 2, Some(planets()[1].0), false, Scope(u32::MAX), Code(0), Then::Nothing);
        within.placed = 0;
        let mut d = driver(within);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!((count(&s, planets()[0].1, infantry), count(&s, planets()[1].1, infantry)), (0, 2));
    }

    #[test]
    fn nothing_is_asked_when_the_box_is_empty_when_the_unit_is_unavailable_or_the_planet_is_demilitarized() {
        let k = Kernel::plain();
        let mech = unit_of(UnitBaseType::Mech);
        // The box holds four mechs; all four stand on the board.
        let mut s = table();
        control(&mut s, 2);
        for _ in 0..4 {
            fx::place_unit(&mut s, Seat(0), planets()[0].0, Some(planets()[0].1), mech);
        }
        let mut d = driver(window(mech, true, Then::Nothing));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!(d.window.placed, 0);
        // One mech left in the box: one is placed of the two asked for, and the question says so.
        let mut s = table();
        control(&mut s, 2);
        for _ in 0..3 {
            fx::place_unit(&mut s, Seat(0), planets()[0].0, Some(planets()[0].1), mech);
        }
        let mut d = driver(window(mech, true, Then::Nothing));
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(c.ask, Ask::Promissory(PAsk::PlaceUnits { unit: mech, fit: 1 }));
        d.resume(&mut s, &k, Pick::Opt(c.opts.as_slice()[1])).unwrap();
        assert_eq!((count(&s, planets()[1].1, mech), d.window.placed), (1, 1));
        // No unit form (the seat places nothing, or a faction rule forbids it).
        let mut s = table();
        control(&mut s, 2);
        let mut d = driver(PlaceUnits::new(Seat(0), None, 2, None, true, Scope(u32::MAX), Code(0), Then::Nothing));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        // A demilitarized planet is not offered (the exploration attachment, at the rules version that has it).
        let infantry = unit_of(UnitBaseType::Infantry);
        let mut s = table();
        control(&mut s, 2);
        s.meta_mut().rules_version = crate::query::laws::RULES_DMZ_EXPLORATION;
        s.board_mut().planet_mut(planets()[0].1).attach.push(Attachment::DemilitarizedZone);
        let mut d = driver(window(infantry, false, Then::Nothing));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done, "one planet is left: no question");
        assert_eq!((count(&s, planets()[0].1, infantry), count(&s, planets()[1].1, infantry)), (0, 2));
    }

    #[test]
    fn military_support_spends_the_token_of_its_owner_announces_it_and_sends_the_note_home_whatever_the_answer() {
        let (k, infantry) = (Kernel::plain(), unit_of(UnitBaseType::Infantry));
        let note = NoteKey { note: PromissoryNote::MilitarySupport, faction: Faction::from_index(1).unwrap() };
        for decline in [false, true] {
            let mut s = table();
            control(&mut s, 2);
            s.players_mut().by_seat_mut(Seat(1)).unwrap().econ.strategic_tokens = 2;
            s.cards_mut().promissory.edit().insert(note, Seat(0));
            let then = Then::MilitarySupport { owner: Seat(1), reason: Code(0) };
            let mut d = driver(PlaceUnits::new(Seat(0), Some(infantry), 2, None, true, Scope(u32::MAX), Code(0), then));
            let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
            assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 2, "nothing before the answer");
            let answer = if decline { Opt::Decline } else { c.opts.as_slice()[0] };
            assert_eq!(d.resume(&mut s, &k, Pick::Opt(answer)).unwrap(), Status::Done);
            assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.strategic_tokens, 1, "the owner pays, not the holder");
            assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 0);
            assert_eq!(s.cards().promissory.get(&note), Some(&Seat(1)), "the note is home");
            assert_eq!(s.pending().staged.len(), 1, "the spent token is announced");
            assert_eq!(count(&s, planets()[0].1, infantry), if decline { 0 } else { 2 });
        }
        // Nothing to place and no token: the note still goes home and nothing is announced.
        let mut s = table();
        s.cards_mut().promissory.edit().insert(note, Seat(0));
        let mut d = driver(PlaceUnits::new(Seat(0), Some(infantry), 2, None, true, Scope(u32::MAX), Code(0), Then::MilitarySupport { owner: Seat(1), reason: Code(0) }));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!(s.cards().promissory.get(&note), Some(&Seat(1)));
        assert!(s.pending().staged.is_empty());
    }
}
