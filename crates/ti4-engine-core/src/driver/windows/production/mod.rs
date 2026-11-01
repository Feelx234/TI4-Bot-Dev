//! One use of PRODUCTION as ONE window the core drives and holds (lane M2; the string `AftermathWindow::enter_production`,
//! `ProductionWindow::{new, refresh, resolve, finish_use}` and `production::resolve_timed`). L5: it composes the production window
//! (`rules::production::window`), the Hacan hero's offer (`rules::production::triggered`), Sarween Tools / AI Development Algorithm
//! (`rules::technology_windows::ProductionUsed`) and Self-Assembly Routines (the table's link) in the order of the string, and keeps what the string
//! window keeps between two questions as the typed payload [`crate::state::ProductionUse`] on the `Pending` stack.
//!
//! # The three openers ([`UseMode`])
//!
//! | mode       | string                                   | before the window                                                                                   | after the last answer |
//! |------------|------------------------------------------|-----------------------------------------------------------------------------------------------------|-----------------------|
//! | `Tactical` | `enter_production` (the aftermath step)  | a capacity of 0 ends it at once; else the use counter, the hero offer, `production_used`, `PRODUCTION_USED`, `refresh` | the end effects       |
//! | `Home`     | `resolve_timed` (the Warfare secondary)  | the unspent discount is dropped (version 26+); the hero is offered when unlocked and the capacity is positive       | the end effects       |
//! | `Ability`  | `produce_by_ability`                     | none (the caller's facts are the window)                                                              | the end effects       |
//!
//! The effects at the end of a use (`finish_use`) run once, in the step that finds the window done after an answer (T-I2-1: a use that asked nothing has
//! none): the planet value a swap lasted for goes back (Hegemonic Trade Policy), the Prophecy of Ixth is discarded unless two fighters were made,
//! `UNITS_PRODUCED` is announced with its `source` and `units`, and Self-Assembly Routines offers a mech (not for an ability's units). The Thunder's
//! Edge breakthrough tokens of the string (Auto-Factories) are not part of the agreed product and are not ported here.
//!
//! The start and end effects ask their questions THROUGH THE TAPE (`Sx::ask`): the driver rolls the step back and replays it with the answers, so the
//! payload never has to hold a half-run child window; only the production window's own questions (what to build, how to pay, where to put it) are stages.

mod live;

use crate::choice::{Choice, Pick};
use crate::generated::{Leader, Technology, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::ability::ProductionGap as Gap;
use crate::kernel::children::{Report, Spawn};
use crate::kernel::seam::observe::{Obs, Phase};
use crate::kernel::stage::{Child, Held, Illegal, Ran, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::rules::production::home::home_facts;
use crate::rules::production::triggered::HeroOffer;
use crate::rules::technology_windows::ProductionUsed;
use crate::state::{CoreState, EventKind, Key, List, Payload, ProductionUse, UseMode, UsePhase, Val, Window as OpenWindow, WindowKind};

pub use live::{advance_production, ask_production, open_production, UseOpen};

/// `ti4_model::state::RULES_WINNU_AGENT_PRODUCTION`.
const RULES_WINNU_AGENT_PRODUCTION: u32 = 26;
/// `ti4_model::state::RULES_SELF_ASSEMBLY`.
const RULES_SELF_ASSEMBLY: u32 = 13;

/// The open use of PRODUCTION.
#[derive(Debug, Clone)]
pub struct ProductionMachine {
    /// The payload: the source of truth.
    pub p: ProductionUse,
}

impl ProductionMachine {
    #[must_use]
    pub const fn new(p: ProductionUse) -> Self {
        Self { p }
    }

    /// What the use made, in order, with the planet each unit stands on.
    #[must_use]
    pub fn made(&self) -> crate::kernel::children::Made {
        self.p.window.made()
    }

    fn bail(sx: &mut Sx<'_>, why: Gap) -> Next {
        if sx.tape.unknown.get().is_none() {
            sx.tape.unknown.set(Some(crate::kernel::ability::Unknown::UnportedProduction(why)));
        }
        Next::Done
    }

    fn hero_unlocked(core: &CoreState, who: Seat) -> bool {
        core.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&Leader::HarrughGefhara) == Some(&crate::state::LeaderStatus::Unlocked))
    }

    /// The start-of-use effects of the mode, then the window. `Err`: a counted gap.
    fn start(&mut self, sx: &mut Sx<'_>) -> Result<(), Gap> {
        let (who, system) = (self.p.who, self.p.system);
        match self.p.mode {
            UseMode::Ability => {
                self.p.phase = UsePhase::Using;
                return Ok(());
            }
            UseMode::Tactical | UseMode::Home => {}
        }
        let roster = sx.table.facts.roster.ok_or(Gap::NoRoster)?;
        let agent_rules = sx.state.meta().rules_at_least(RULES_WINNU_AGENT_PRODUCTION);
        let facts = home_facts(sx.state, sx.kernel, roster, who, system, agent_rules).ok_or(Gap::NoFacts)?;
        let mut window = crate::rules::production::window::ProductionWindow::use_of(&facts.produce, agent_rules);
        window.emit_timing = true;
        let capacity = facts.produce.limit;
        match self.p.mode {
            UseMode::Tactical => {
                // `enter_production`: no capacity, no effects, no use.
                if capacity <= 0 {
                    self.p.window = window;
                    self.p.phase = UsePhase::Done;
                    return Ok(());
                }
                // The use counter goes up BEFORE the hero is offered: the free marker names this production (T-I2-2).
                let meta = sx.state.meta_mut();
                meta.production_seq = meta.production_seq.saturating_add(1);
                run_hero(sx, who)?;
                let mut used = ProductionUsed::new(who);
                run_on_tape(sx, &mut used)?;
                let payload = Payload::new().with(Key::Player, Val::Seat(who)).with(Key::System, Val::System(system));
                if sx.emit(fx::event(EventKind::ProductionUsed, payload)).is_err() {
                    window.stage = crate::state::PStage::Done;
                    self.p.window = window;
                    self.p.phase = UsePhase::Done;
                    return Ok(());
                }
                window.refresh(sx.state, sx.kernel);
            }
            UseMode::Home => {
                if agent_rules {
                    // This use reads no discount an earlier tactical action left unspent.
                    sx.state.transient_mut().production.discount_remaining = 0;
                }
                if capacity > 0 && Self::hero_unlocked(sx.state, who) {
                    let meta = sx.state.meta_mut();
                    meta.production_seq = meta.production_seq.saturating_add(1);
                    if run_hero(sx, who)? {
                        window.refresh(sx.state, sx.kernel);
                    }
                }
            }
            UseMode::Ability => {}
        }
        self.p.window = window;
        self.p.phase = UsePhase::Using;
        Ok(())
    }

    /// `finish_use`: the effects at the end of a use, once.
    fn finish(&mut self, sx: &mut Sx<'_>) -> Result<(), Gap> {
        let (who, system) = (self.p.who, self.p.system);
        end_value_swap(sx.state);
        // The Prophecy of Ixth: using PRODUCTION discards the law unless two or more fighters were produced (read over the whole use).
        let produced = self.p.window.produced.clone();
        let fighters = produced.as_slice().iter().filter(|(unit, _)| unit.data().base_type == UnitBaseType::Fighter).count();
        let blanked = crate::query::reach::blanked(sx.state);
        if fighters < 2 && crate::query::laws::holds_ministry(sx.state, blanked, who, crate::generated::Agenda::ProphecyOfIxth) {
            sx.state.agenda_mut().laws.remove(&crate::generated::Agenda::ProphecyOfIxth);
        }
        // Yin's Spinner and the rest: announced last, once every end-of-use effect has landed, and only for a use that made something.
        if !produced.as_slice().is_empty() {
            let source = if self.p.window.ability { sx.table.codes.production.ability } else { sx.table.codes.production.production };
            let mut payload = Payload::new()
                .with(Key::Player, Val::Seat(who))
                .with(Key::System, Val::System(system))
                .with(Key::Source, Val::Code(source))
                .with(Key::Count, Val::Int(i64::try_from(produced.as_slice().len()).unwrap_or(i64::MAX)));
            payload.set_list(Key::Other(sx.table.codes.production.units), List::Produced(produced.clone()));
            // A cancelled or failed announcement cannot un-produce the units.
            let _ = sx.emit(fx::event(EventKind::UnitsProduced, payload));
        }
        // Self-Assembly Routines: "after 1 or more of your units use PRODUCTION". Not for units an ability produces.
        if !self.p.window.ability
            && !produced.as_slice().is_empty()
            && sx.state.meta().rules_at_least(RULES_SELF_ASSEMBLY)
            && crate::query::tech::owns(sx.state, who, Technology::SelfAssemblyRoutines)
        {
            let mut child = crate::kernel::children::ErasedChild::idle();
            child.start(sx.table.links.start(&Spawn::SelfAssembly { who, system }).ok_or(Gap::NoLink)?);
            run_erased_on_tape(sx, &mut child)?;
        }
        Ok(())
    }

    /// The use is over: what lasted for it goes (the string's `resolve_timed` closes the observers and the swap itself).
    fn close(&mut self, sx: &mut Sx<'_>) {
        if self.p.mode == UseMode::Home {
            sx.observe(&Obs::ProductionWindowClosed { who: self.p.who, system: self.p.system }, Phase::After);
            end_value_swap(sx.state);
        }
        self.p.phase = UsePhase::Done;
    }
}

/// The planet-value swap lasts one use of PRODUCTION (Hegemonic Trade Policy): `production::end_value_swap`.
pub(crate) fn end_value_swap(core: &mut CoreState) {
    core.transient_mut().production.value_swapped_planet = None;
    core.marks_mut().tail.edit().remove(&crate::state::MarkKey::ProductionValueSwapSeq());
}

/// The Hacan hero's offer on the tape; whether it was used.
fn run_hero(sx: &mut Sx<'_>, who: Seat) -> Result<bool, Gap> {
    let mut offer = HeroOffer::new(who);
    run_on_tape(sx, &mut offer)?;
    Ok(offer.used)
}

/// The most questions one effect of a use may put (the loops of the windows it runs are short).
const TAPE_LIMIT: u8 = 32;

/// Run `window` to its end, putting each question to the tape (`Sx::ask`): an answer the tape does not have yet rolls the whole step back and the
/// driver reports the question; the replay finds the answer.
fn run_on_tape<W: Window>(sx: &mut Sx<'_>, window: &mut W) -> Result<(), Gap> {
    let mut child = Child::idle();
    child.start(window.clone());
    for _ in 0..TAPE_LIMIT { // mutants:skip (a bound no window of the start or the end of a use reaches)
        match child.step(sx) {
            None => break,
            Some(Ran::Done(done)) => {
                // (`Child` hands the finished window back: what it did, `HeroOffer::used`, is read off it.)
                *window = done;
                break;
            }
            Some(Ran::Pass(_)) => return Err(Gap::ChildPassed),
            Some(Ran::Asks(_)) => {
                let choice = child.ask_of(sx.state, sx.kernel).ok_or(Gap::ChildStuck)?;
                let pick = sx.ask(&choice);
                child.answer(sx, pick).map_err(|_| Gap::ChildStuck)?;
            }
        }
    }
    Ok(())
}

/// [`run_on_tape`] for the child of another area ([`crate::kernel::children::ErasedChild`]).
fn run_erased_on_tape(sx: &mut Sx<'_>, child: &mut crate::kernel::children::ErasedChild) -> Result<Option<Report>, Gap> {
    for _ in 0..TAPE_LIMIT { // mutants:skip (as above)
        match child.step(sx) {
            None => return Ok(None),
            Some(Ran::Done(report)) => return Ok(Some(report)),
            Some(Ran::Pass(_)) => return Err(Gap::ChildPassed),
            Some(Ran::Asks(_)) => {
                let choice = child.ask_of(sx.state, sx.kernel).ok_or(Gap::ChildStuck)?;
                let pick = sx.ask(&choice);
                child.answer(sx, pick).map_err(|_| Gap::ChildStuck)?;
            }
        }
    }
    Err(Gap::ChildStuck)
}

impl Window for ProductionMachine {
    type Stage = UsePhase;

    fn stage(&self) -> UsePhase {
        self.p.phase
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        for _ in 0..8 { // mutants:skip (a bound: a use passes through four phases at most)
            match self.p.phase {
                UsePhase::Start => {
                    if let Err(why) = self.start(sx) {
                        return Self::bail(sx, why);
                    }
                }
                UsePhase::Using => match self.p.window.step(sx) {
                    Next::Continue(ask) => return Next::Continue(ask),
                    Next::Done => {
                        self.p.phase = if self.p.answered { UsePhase::Finishing } else { UsePhase::Done };
                        if !self.p.answered {
                            self.close(sx);
                        }
                    }
                    other => return other,
                },
                UsePhase::Finishing => {
                    if let Err(why) = self.finish(sx) {
                        return Self::bail(sx, why);
                    }
                    self.close(sx);
                }
                UsePhase::Done => return Next::Done,
            }
        }
        Self::bail(sx, Gap::NoProgress)
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.p.phase == UsePhase::Using { self.p.window.ask_of(s, k) } else { None }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.p.phase != UsePhase::Using {
            return Err(Illegal);
        }
        self.p.window.answer(sx, pick)?;
        self.p.answered = true;
        Ok(())
    }
}

impl Held for ProductionMachine {
    const KIND: WindowKind = WindowKind::Production;

    fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
        match core.pending().window(WindowKind::Production)? {
            OpenWindow::Production(p) => Some(Self::new((**p).clone())),
            _ => None,
        }
    }

    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::Production(Box::new(self.p.clone())));
    }
}
