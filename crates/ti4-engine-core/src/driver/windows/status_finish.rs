//! Steps 81.5 (second sentence) to 81.8 and the end of the status phase (package KS5, lane C): `Game::finish_status_phase` and
//! `status::resolve_after_token_gain`, as ONE job of the turn flow ([`TurnJob::StatusFinish`]).
//!
//! The order is the string's: every seat redistributes its command tokens in initiative order ([`Redistribute`]), the typed
//! `STRATEGY_CARDS_WOULD_RETURN` per seat holding cards (the reaction windows: Political Stability and the like), then 81.6-81.8 (exhausted
//! technologies, relics and legendary ability cards, leaders ready; the Crown of Emphidia pays; every planet readies; damaged units repair;
//! the strategy cards go back unless a seat keeps them), then `COMMAND_TOKENS_GAINED`, Genesis, `STATUS_PHASE_RESOLVED` and the typed
//! `STATUS_PHASE_ENDED`.

use crate::choice::strategy::{Ask as SAsk, Opt as SOpt, Subtype as SSub};
use crate::choice::generic::Rule;
use crate::choice::{Ask, Choice, Ctx, Opt, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::driver::turn::inline::{emit_typed, gap, halted, player_payload, run, sync_clock, Flow};
use crate::generated::{Faction, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::{fx, fx_strategy, Kernel, Next};
use crate::query::reach;
use crate::query::seats::initiative_order;
use crate::rules::strategy_primaries::{sheet_total, splits};
use crate::rules::{explore, leaders, strategy_flow};
use crate::state::{CoreState, EventKind, Payload, PlainArg, PlainKind, PlainRec, PlanetFlags};

/// `RULES_LETNEV_FLEET_POOL`: from here Letnev may empty its fleet pool.
const RULES_LETNEV_FLEET_POOL: u32 = 4;

// -- 81.5, second sentence: every seat rearranges its sheet ---------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RStage {
    Start,
    Asking,
    Done,
}

/// "Then, each player can redistribute each command token on their command sheet among their strategy, tactic, and fleet pools"
/// (`strategy_cards::redistribute_tokens` with subtype `status_redistribute_tokens`): one question over every legal final split, asked
/// only while the seat holds a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Redistribute {
    pub who: Seat,
    stage: RStage,
}

impl Redistribute {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, stage: RStage::Start }
    }

    fn floor(&self, core: &CoreState) -> u8 {
        let letnev = core.players().by_seat(self.who).is_some_and(|p| p.faction == Faction::TheBaronyOfLetnev);
        if letnev && core.meta().rules_at_least(RULES_LETNEV_FLEET_POOL) { 0 } else { 2 }
    }

    fn total(&self, core: &CoreState) -> u8 {
        u8::try_from(sheet_total(core, self.who)).unwrap_or(u8::MAX)
    }
}

impl Window for Redistribute {
    type Stage = RStage;

    fn stage(&self) -> RStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == RStage::Start {
            self.stage = if sheet_total(sx.state, self.who) > 0 { RStage::Asking } else { RStage::Done };
        }
        match self.stage {
            RStage::Asking => Next::Continue(Ask::Strategy(SAsk::StatusRedistribute { total: self.total(sx.state), floor: self.floor(sx.state) })),
            RStage::Start | RStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, core: &CoreState, _: &Kernel) -> Option<Choice> {
        if self.stage != RStage::Asking {
            return None;
        }
        let (total, floor) = (self.total(core), self.floor(core));
        Some(Choice {
            actor: self.who,
            ask: Ask::Strategy(SAsk::StatusRedistribute { total, floor }),
            ctx: Ctx {
                origin: Origin::Rule(Rule::R81_5),
                subtype: Subtype::Strategy(SSub::StatusRedistributeTokens),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: splits(total, floor).map(Opt::Strategy).collect(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Strategy(SOpt::Split { tactic, fleet, strategic })) = pick else { return Err(Illegal) };
        let offered = self.ask_of(sx.state, sx.kernel).ok_or(Illegal)?;
        if !offered.opts.as_slice().contains(&Opt::Strategy(SOpt::Split { tactic, fleet, strategic })) {
            return Err(Illegal);
        }
        if let Some(p) = sx.state.players_mut().by_seat_mut(self.who) {
            p.econ.tactic_tokens = i16::from(tactic);
            p.econ.fleet_tokens = i16::from(fleet);
            p.econ.strategic_tokens = i16::from(strategic);
        }
        self.stage = RStage::Done;
        Ok(())
    }
}

// -- 81.6 to 81.8 ----------------------------------------------------------------------------------------------

/// `status::resolve_after_token_gain`: ready, repair, return. Choice-free.
fn resolve_after_token_gain(core: &mut CoreState, artuno: crate::state::Code) {
    // 81.6: exhausted technologies, relics and legendary ability cards, leaders ready.
    for i in 0..core.players().len() {
        core.players_mut().edit(i).tech.exhausted = crate::state::TechSet::EMPTY;
    }
    explore::ready_relics_and_legendary(core);
    let seats: Vec<Seat> = core.players().iter().map(|p| p.seat).collect();
    for seat in &seats {
        // (Artuno the Betrayer pays out when it readies: `nomad_agents::agent_readied`, called by `leaders::ready_all` in the string.)
        for leader in leaders::ready_all(core, *seat).as_slice() {
            leaders::agent_readied(core, *seat, *leader, artuno);
        }
    }
    // The Crown of Emphidia pays out before the cards are readied.
    for seat in &seats {
        let _ = explore::crown_point(core, *seat);
    }
    // 81.6: every planet readies.
    let exhausted: Vec<_> = core.board().planets().filter(|(_, cell)| cell.flags.has(PlanetFlags::EXHAUSTED)).map(|(p, _)| p).collect();
    for planet in exhausted {
        fx::ready_planet(core, planet);
    }
    // 81.7: damaged units repair, in space and on planets.
    let systems: Vec<_> = core.board().systems().filter(|(_, c)| c.units.as_slice().iter().any(|u| u.sustained())).map(|(s, _)| s).collect();
    for system in systems {
        let cell = core.board_mut().sys_mut(system);
        for unit in cell.units.as_mut_slice() {
            *unit = unit.with_sustained(false);
        }
    }
    let planets: Vec<_> = core.board().planets().filter(|(_, c)| c.units.as_slice().iter().any(|u| u.sustained())).map(|(p, _)| p).collect();
    for planet in planets {
        let cell = core.board_mut().planet_mut(planet);
        for unit in cell.units.as_mut_slice() {
            *unit = unit.with_sustained(false);
        }
    }
    // 81.8: the cards go back, except those a seat under Political Stability keeps (they ready); nobody has passed any more.
    for seat in &seats {
        let Some(p) = core.players().by_seat(*seat) else { continue };
        if p.econ.stability {
            let p = core.players_mut().by_seat_mut(*seat);
            if let Some(p) = p {
                p.strat_exhausted.clear();
                p.econ.passed = false;
            }
            continue;
        }
        strategy_flow::clear(core, *seat);
        if let Some(p) = core.players_mut().by_seat_mut(*seat) {
            p.econ.passed = false;
        }
    }
}

/// Genesis, the Sol flagship: "At the end of the status phase, place 1 infantry from your reinforcements in this system's space area."
/// One per flagship, of the type the owner builds, while the box has one left. Gap: the unit roster is missing.
fn genesis(sx: &mut Sx<'_>) -> Flow {
    let flagships: Vec<_> = sx
        .state
        .board()
        .systems()
        .flat_map(|(system, cell)| cell.units.as_slice().iter().filter(|u| u.kind() == Unit::Genesis).map(move |u| (u.owner(), system)).collect::<Vec<_>>())
        .collect();
    if flagships.is_empty() {
        return Flow::Done;
    }
    let Some(roster) = sx.table.facts.plays.as_ref().and_then(|p| p.roster) else {
        gap(sx, TurnGap::NoRoster);
        return Flow::Halt;
    };
    let scope = reach::scope_of(sx.kernel);
    for (owner, system) in flagships {
        if fx_strategy::place_for(sx.state, sx.kernel, scope, roster, owner, system, None, UnitBaseType::Infantry).is_some() {
            sx.plain(PlainRec::two(PlainKind::GenesisInfantry, PlainArg::Seat(owner), PlainArg::System(system)));
        }
    }
    Flow::Done
}

/// `Live::announce_staged_cards` (`reactions::announce_staged_card_events`) for a job: the card moves an effect staged as `CardsStaged` marks are
/// announced now, oldest first (the one body is [`crate::kernel::fx_cards::announce_staged_cards`], which every top-level typed event calls too).
/// `Flow::Halt` when a question found the tape dry or the resolver refused.
pub(crate) fn announce_staged_cards(sx: &mut Sx<'_>) -> Flow {
    if crate::kernel::fx_cards::announce_staged_cards(sx).is_err() {
        // A dry tape rolls the step back whatever the emission returned; a refusal on a wet tape is a defect.
        if !halted(sx) { // mutants:skip: the refusal of the resolver is a defect no position reaches
            gap(sx, TurnGap::EmissionRefused);
        }
        return Flow::Halt;
    }
    if halted(sx) { Flow::Halt } else { Flow::Done } // mutants:skip: a halted step is rolled back by the driver whatever the job does after
}

/// `Live::emit_typed`: the event through its windows, then the staged card moves (what [`emit_typed`] does for every typed event).
fn emit_and_announce(sx: &mut Sx<'_>, kind: EventKind, payload: Payload) -> Flow {
    emit_typed(sx, kind, payload)
}

/// The index of the text `artuno` in `Codes::not_from_supply`.
const ARTUNO: usize = 4;

/// The job.
pub fn run_finish(sx: &mut Sx<'_>) -> Flow {
    // 81.5: the rearrangement of everything already held, in initiative order (before 81.8 destroys it).
    let order: Vec<Seat> = initiative_order(sx.state).iter().copied().collect();
    for seat in order {
        if run(sx, &mut Redistribute::new(seat)) == Flow::Halt {
            return Flow::Halt;
        }
    }
    // "When you would return strategy cards during the status phase": per seat holding cards, before 81.8 returns them.
    let holders: Vec<Seat> = sx.state.players().iter().filter(|p| !p.strat.is_empty()).map(|p| p.seat).collect();
    sync_clock(sx);
    for seat in holders {
        if emit_and_announce(sx, EventKind::StrategyCardsWouldReturn, player_payload(seat)) == Flow::Halt {
            return Flow::Halt;
        }
    }
    if halted(sx) {
        return Flow::Halt;
    }
    resolve_after_token_gain(sx.state, sx.table.codes.not_from_supply[ARTUNO]);
    sx.plain(PlainRec::bare(PlainKind::CommandTokensGained));
    if genesis(sx) == Flow::Halt {
        return Flow::Halt;
    }
    sx.plain(PlainRec::bare(PlainKind::StatusPhaseResolved));
    // "At the end of the status phase" (Bioplasmosis): nothing opens unless a faction module listens.
    emit_and_announce(sx, EventKind::StatusPhaseEnded, Payload::new())
}
