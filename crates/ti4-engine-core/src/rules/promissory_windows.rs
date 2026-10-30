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
