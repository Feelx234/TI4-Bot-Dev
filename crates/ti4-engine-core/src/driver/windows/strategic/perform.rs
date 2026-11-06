//! A strategy card's ability performed inside one step of the strategic action (`strategy_cards::{primary, secondary, follow}` and
//! `Game::resolve_faction_strategy`), by the card's own window as a child ([`crate::kernel::fx_perform::begin`]).
//!
//! Every question the child asks goes to the TAPE (`Sx::ask`): when the tape is dry the driver rolls the whole step back and reports the
//! question, and `driver::live` stores the answers given so far (`Pending::nested`). So a caller that returns between two questions, a clone
//! of the position mid-ability and a fresh driver all continue identically. The window must not open other windows.
//!
//! What the child leaves for the play to announce ([`Finished`]) is staged here the way the string stages it: the command tokens a Diplomacy
//! primary placed (`COMMAND_TOKEN_PLACED`, from the reinforcements), the trade goods of the Trade primary (by `fx_perform::finish`). The units a
//! use of PRODUCTION made are announced by the production lane (`UNITS_PRODUCED`); until that lane's window carries the list the core stops
//! with the counted gap [`TurnGap::ProductionAnnounce`].

use crate::driver::turn::inline::{gap, gap_flow, halted, run as run_window, Flow};
use crate::generated::{StrategyCard, StrategyCardKind, System};
use crate::ids::Seat;
use crate::kernel::children::ErasedChild;
use crate::kernel::fx;
use crate::kernel::fx_perform::{self, Began, Finished};
use crate::kernel::stage::{Ran, Sx};
use crate::choice::Pick;
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::Next;
use crate::state::{EventKind, Key, Payload, Val};

use super::facts;

/// Run the ability of `card` for `who` (the secondary, or the primary when `primary`) to its end, putting its questions to the tape. `free`
/// is where the system of Thunder's Edge Warfare's free tactical action goes (`Ability::FreeTactical`).
pub fn run(sx: &mut Sx<'_>, who: Seat, card: StrategyCard, primary: bool, free: &mut Option<System>) -> Flow {
    run_from(sx, who, card, primary, free, Caller::Inline)
}

/// Who runs a perform, and so what the operation does after it (stepmachine-1: a converted caller lets the child loop park at a question).
#[derive(Clone, Copy)]
pub(crate) enum Caller<'a> {
    /// Not converted: a question replays the operation from its start.
    Inline,
    /// `SecondaryWin::answer` of this window (as it stands when it performs): after the perform, `resolved` for the follower; the op is the
    /// window's answer.
    Secondary(&'a super::SecondaryWin),
    /// `begin::run_begin`, the step of a turn-flow job: after the perform, `begin::after_primary`; the op is the job's step.
    Primary,
}

/// [`Caller`] as data, kept in a [`Parked`].
#[derive(Debug, Clone)]
pub enum Frame {
    Secondary(Box<super::SecondaryWin>),
    Primary,
}

/// The child loop of a perform stopped at its child's question (stepmachine-1): the locals of [`drive`] and who called it. With the
/// position and the resolver of that moment (`driver::Park`) this is the whole operation up to the question.
#[derive(Debug, Clone)]
pub struct Parked {
    frame: Frame,
    p: Perform,
}

/// The locals of the child loop.
#[derive(Debug, Clone)]
struct Perform {
    who: Seat,
    card: StrategyCard,
    primary: bool,
    trade_primary: bool,
    trade_source: crate::state::Code,
    child: ErasedChild,
}

pub(crate) fn run_from(sx: &mut Sx<'_>, who: Seat, card: StrategyCard, primary: bool, free: &mut Option<System>, caller: Caller<'_>) -> Flow {
    let card_facts = facts::perform_facts(sx, who, card);
    match fx_perform::begin(sx, who, card, primary, card_facts) {
        Began::Done => Flow::Done,
        Began::Unsupported => {
            gap(sx, TurnGap::StrategyAbility);
            Flow::Halt
        }
        Began::Run(spawn) => {
            let Some(window) = sx.table.links.start(&spawn) else {
                gap(sx, TurnGap::NoLink);
                return Flow::Halt;
            };
            let mut child = ErasedChild::idle();
            child.start(window);
            // The Trade primary: the string stages the three trade goods when it gains them, BEFORE the replenishes its window announces; the
            // window reports the goods for the parent to stage once it is done (the action cards' way), which would order the events after the
            // replenishes. The strategic action stages them first and drops the report.
            let trade_primary = primary && card.data().kind == StrategyCardKind::Trade;
            if trade_primary {
                fx::stage(sx.state, fx::trade_goods_gained(who, crate::rules::strategy_windows::TRADE_PRIMARY_GOODS, card_facts.carried.trade_source));
            }
            let mut p = Perform { who, card, primary, trade_primary, trade_source: card_facts.carried.trade_source, child };
            drive(sx, &mut p, free, caller)
        }
    }
}

/// The child loop: the child stepped to its end, its questions put to the tape, then what the finished ability announces.
fn drive(sx: &mut Sx<'_>, p: &mut Perform, free: &mut Option<System>, caller: Caller<'_>) -> Flow {
    let finished = loop {
        if halted(sx) {
            return Flow::Halt;
        }
        match p.child.step(sx) {
            None => {
                gap(sx, TurnGap::ReplayDiverged);
                return Flow::Halt;
            }
            Some(Ran::Done(report)) => {
                let report = if p.trade_primary && matches!(report, crate::kernel::children::Report::Goods(..)) { crate::kernel::children::Report::Nothing } else { report };
                break fx_perform::finish(report, sx.state, p.trade_source);
            }
            Some(Ran::Asks(_)) => {
                let Some(choice) = p.child.ask_of(sx.state, sx.kernel) else {
                    gap(sx, TurnGap::ReplayDiverged);
                    return Flow::Halt;
                };
                let dry = sx.tape.exhausted();
                let pick = sx.ask(&choice);
                if halted(sx) {
                    if dry {
                        park(sx, p, caller);
                    }
                    return Flow::Halt;
                }
                if p.child.answer(sx, pick).is_err() {
                    gap(sx, TurnGap::ReplayDiverged);
                    return Flow::Halt;
                }
            }
            Some(Ran::Pass(next)) => {
                for n in next.flatten() {
                    match n {
                        Next::Emit(rec) => {
                            if sx.emit(rec).is_err() { // mutants:skip: no window the abilities spawn returns `Next::Emit`
                                gap(sx, TurnGap::EmissionRefused);
                                return Flow::Halt;
                            }
                        }
                        // (`Next::Done` and `Chain` are flattened away; a continuation is the child's own ask.)
                        _ => {
                            gap(sx, TurnGap::OpenedWindow);
                            return Flow::Halt;
                        }
                    }
                }
            }
        }
    };
    *free = finished.free;
    announce(sx, p.who, p.card, p.primary, finished)
}

/// The child's question found the tape dry: leave the operation as it stands for the driver (`Tape::parked`), when the caller is converted and
/// the loop runs at the top of the operation (no emission open, nothing a hook emitted in flight).
fn park(sx: &mut Sx<'_>, p: &Perform, caller: Caller<'_>) {
    if !crate::driver::parking() || sx.tape.needed().is_none() || sx.tape.unknown.get().is_some() || !sx.resolver.emission_stack().is_empty() || !sx.tape.emitted.is_empty() {
        return;
    }
    let frame = match caller {
        Caller::Inline => return,
        Caller::Secondary(w) => Frame::Secondary(Box::new(w.clone())),
        Caller::Primary => Frame::Primary,
    };
    let park = crate::driver::Park {
        state: sx.state.clone(),
        resolver: sx.resolver.clone(),
        next_id: *sx.next_id,
        emitted: sx.emitted.clone(),
        opened: Vec::new(),
        at: crate::driver::Resume::Perform(Parked { frame, p: p.clone() }),
    };
    sx.tape.parked = crate::state::Memo::of(park);
}

impl Parked {
    /// The card and whether its primary is performed (the counter key of `driver::park_detail`).
    #[must_use]
    pub const fn key(&self) -> (StrategyCard, bool) {
        (self.p.card, self.p.primary)
    }

    /// Whether this continuation belongs to `op` of the driver's `window`: a follower's answer of the follower window, or the step of the
    /// strategic job (`TurnJob::Strategic`, or the action menu's answer `TurnJob::Action`).
    pub(crate) fn fits(&self, op: &crate::state::NestedOp, window: &dyn std::any::Any) -> bool {
        use crate::state::{NestedOp, TurnJob};
        match &self.frame {
            Frame::Secondary(_) => matches!(op, NestedOp::Answer(_)) && window.is::<super::SecondaryWin>(),
            Frame::Primary => {
                matches!(op, NestedOp::Step)
                    && window.downcast_ref::<crate::driver::turn::TurnFlowWin>().is_some_and(|w| matches!(w.job, TurnJob::Strategic(_) | TurnJob::Action(_)))
            }
        }
    }

    /// The operation from the question on: the child takes `pick`, the loop runs on, then what the caller does after the perform. `window` is
    /// the driver's (the follower window takes the state it had at the question). `None`: it is not the window the loop parked in.
    pub(crate) fn resume(&self, sx: &mut Sx<'_>, window: &mut dyn std::any::Any, pick: Pick) -> Option<crate::driver::Resumed> {
        if let Frame::Secondary(w) = &self.frame {
            *window.downcast_mut::<super::SecondaryWin>()? = (**w).clone();
        }
        let mut p = self.p.clone();
        let mut free = None;
        let caller = match &self.frame {
            Frame::Secondary(w) => Caller::Secondary(w),
            Frame::Primary => Caller::Primary,
        };
        let flow = if p.child.answer(sx, pick).is_err() {
            gap(sx, TurnGap::ReplayDiverged);
            Flow::Halt
        } else {
            drive(sx, &mut p, &mut free, caller)
        };
        Some(match &self.frame {
            Frame::Secondary(_) => {
                if flow == Flow::Done {
                    let _: Flow = resolved(sx, p.who, p.card.data().kind);
                }
                crate::driver::Resumed::Answered
            }
            Frame::Primary => {
                if flow == Flow::Done {
                    let _: Flow = super::begin::after_primary(sx, p.who, p.card, free);
                }
                crate::driver::Resumed::Stepped
            }
        })
    }
}

/// What the finished ability announces (the string stages it after the window).
fn announce(sx: &mut Sx<'_>, who: Seat, card: StrategyCard, primary: bool, done: Finished) -> Flow {
    if done.failed {
        gap(sx, TurnGap::StrategyAbility);
        return Flow::Halt;
    }
    if let Some((system, seats)) = done.tokens {
        place_tokens(sx, system, seats.as_slice());
    }
    if done.scored.is_some() {
        // What a public score sets off (51.7: a hero may unlock) and the winner (`imperial_primary_string`).
        let unlocking = crate::driver::turn::start::unlockable(sx, who, true);
        crate::rules::leaders::unlock(sx.state, who, unlocking.as_slice());
    }
    if primary && card.data().kind == StrategyCardKind::Imperial {
        fx::finish_if_won(sx.state);
    }
    if let (Some(made), Some(system)) = (done.made.as_ref().filter(|made| !made.as_slice().is_empty()), done.made_in.or_else(|| home_of(sx, who))) {
        announce_made(sx, who, system, made);
    }
    Flow::Done
}

/// The system a seat's Warfare secondary produced in: its home system.
fn home_of(sx: &Sx<'_>, who: Seat) -> Option<System> {
    sx.state.players().by_seat(who).and_then(|p| p.home_system)
}

/// The announcement the string stages at the end of a use of PRODUCTION that made something (`UNITS_PRODUCED`, no timing handle: staged for the
/// next step's reconcile).
pub(crate) fn announce_made(sx: &mut Sx<'_>, who: Seat, system: System, made: &crate::kernel::children::Made) {
    use crate::choice::production::Spot;
    use crate::state::List;
    let codes = sx.table.codes.production;
    let spots = made.as_slice().iter().map(|(unit, planet)| (*unit, planet.map_or(Spot::Space, Spot::Planet))).collect();
    let mut payload = Payload::new()
        .with(Key::Player, Val::Seat(who))
        .with(Key::System, Val::System(system))
        .with(Key::Source, Val::Code(codes.production))
        .with(Key::Count, Val::Int(i64::try_from(made.as_slice().len()).unwrap_or(i64::MAX)));
    payload.set_list(Key::Other(codes.units), List::Produced(spots));
    fx::stage(sx.state, fx::event(EventKind::UnitsProduced, payload));
}

/// `stage_command_token_placed(.., Reinforcements)` for each seat that got a token of the Diplomacy primary, in seating order.
pub(crate) fn place_tokens(sx: &mut Sx<'_>, system: System, seats: &[Seat]) {
    let pool = sx.table.codes.reinforcements;
    for seat in seats {
        let payload = Payload::new().with(Key::Player, Val::Seat(*seat)).with(Key::System, Val::System(system)).with(Key::Pool, Val::Code(pool));
        fx::stage(sx.state, fx::event(EventKind::CommandTokenPlaced, payload));
    }
}

/// `Game::resolve_faction_strategy`: what a faction does when a strategy card finishes for it (`faction_abilities::strategy_resolved`): the hooks of
/// the modules, then Xxcha's Peace Accords after Diplomacy: the annexation window, and the exploration of the planet taken (by its first trait,
/// announced to nobody: the string explores with no timing handle).
pub fn resolved(sx: &mut Sx<'_>, who: Seat, kind: StrategyCardKind) -> Flow {
    let kernel = sx.kernel;
    {
        let mut cx = sx.cx();
        kernel.hooks.general.strategy_resolved(&mut cx, who, Some(kind));
    }
    if kind != StrategyCardKind::Diplomacy {
        return Flow::Done;
    }
    peace_accords(sx, who)
}

/// Xxcha's Peace Accords (`faction_abilities::strategy_resolved`): the window, then what taking the planet sets off in the string's order:
/// Integrated Economy's production, the exploration of the planet by its first trait (announced to nobody: the string explores with no timing handle).
fn peace_accords(sx: &mut Sx<'_>, who: Seat) -> Flow {
    use crate::rules::explore_windows::{traits_now, Exploring};
    use crate::rules::peace_accords::PeaceAccords;
    use crate::state::EDeck;
    let mut window = PeaceAccords::new(who);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    let Some((system, planet)) = window.annexed else { return Flow::Done };
    if sx.state.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(crate::generated::Technology::IntegratedEconomy))
        && integrated_economy(sx, who, system, planet) == Flow::Halt
    {
        return Flow::Halt;
    }
    let Some(deck) = traits_now(sx.state, crate::query::reach::scope_of(sx.kernel), planet).as_slice().first().copied() else { return Flow::Done };
    let mut exploring = Exploring::planet(who, planet, EDeck::Fixed(deck), false); // mutants:skip: `true` announces PLANET_EXPLORED, which only Titans' Terragenesis hears (and its exploration is refused as unsupported)
    if run_window(sx, &mut exploring) == Flow::Halt {
        return Flow::Halt;
    }
    if exploring.unsupported().is_some() {
        return gap_flow(sx, TurnGap::PeaceAccords);
    }
    Flow::Done
}

/// Integrated Economy (`technology::control_gained`, `production::integrated_economy`): units up to the resource value of the planet gained, paid
/// for as usual, then the fleet supply of the system is enforced (it asks only when a form is over).
pub(crate) fn integrated_economy(sx: &mut Sx<'_>, who: Seat, system: System, planet: crate::generated::Planet) -> Flow {
    use crate::kernel::children::{Report, SeeingLimits, Spawn};
    use crate::query::economy::Spend;
    use crate::rules::production::{self, triggered::EconomyWindow, Cx as ProdCx};
    let (scope, blanked) = (crate::query::reach::scope_of(sx.kernel), crate::query::reach::blanked(sx.state));
    let budget = production::planet_value(&ProdCx::new(sx.state, sx.kernel, scope, blanked), who, planet, Spend::Resources);
    if budget <= 0 { // mutants:skip: a budget of nothing ends the window without a question and the enforcement asks only when a form is over
        return Flow::Done;
    }
    let Some(roster) = sx.table.facts.plays.as_ref().and_then(|p| p.roster) else { return gap_flow(sx, TurnGap::NoRoster) };
    let facts = (crate::driver::children::one_link().facts)(sx.state, sx.kernel, roster, who);
    let (Some((buildable, fleet_limit)), Ok(budget)) = (facts, i32::try_from(budget)) else { return gap_flow(sx, TurnGap::PeaceAccords) };
    let mut window = EconomyWindow::new(who, system, planet, budget, fleet_limit, buildable);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    let limits = crate::driver::windows::tactical::limits(sx.state, sx.kernel, who, system);
    let request = Spawn::Seeing { who, system, limits: SeeingLimits { enforce: limits.enforce, shown: limits.shown, halves: limits.halves } };
    if sx.table.links.spawn.is_none() {
        return gap_flow(sx, TurnGap::NoLink);
    }
    match run_spawn(sx, &request) {
        Some(Report::Removed(_) | Report::Nothing) => Flow::Done,
        _ if halted(sx) => Flow::Halt,
        _ => gap_flow(sx, TurnGap::ReplayDiverged),
    }
}

/// A child window of the table run to its end on the tape, its report. `None`: the step is doomed (a question found the tape dry, or a gap was
/// noted) or the child left the path.
fn run_spawn(sx: &mut Sx<'_>, request: &crate::kernel::children::Spawn) -> Option<crate::kernel::children::Report> {
    let window = sx.table.links.start(request)?;
    let mut child = ErasedChild::idle();
    child.start(window);
    loop {
        if halted(sx) {
            return None;
        }
        match child.step(sx)? {
            Ran::Done(report) => return Some(report),
            Ran::Asks(_) => {
                let choice = child.ask_of(sx.state, sx.kernel)?;
                let pick = sx.ask(&choice);
                if halted(sx) || child.answer(sx, pick).is_err() {
                    return None;
                }
            }
            Ran::Pass(_) => return None,
        }
    }
}
