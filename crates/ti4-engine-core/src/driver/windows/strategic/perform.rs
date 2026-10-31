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

use crate::driver::turn::inline::{gap, halted, Flow};
use crate::generated::{Faction, StrategyCard, StrategyCardKind, System};
use crate::ids::Seat;
use crate::kernel::children::ErasedChild;
use crate::kernel::fx;
use crate::kernel::fx_perform::{self, Began, Finished};
use crate::kernel::stage::{Ran, Sx};
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::Next;
use crate::state::{EventKind, Key, Payload, Val};

use super::facts;

/// Run the ability of `card` for `who` (the secondary, or the primary when `primary`) to its end, putting its questions to the tape.
pub fn run(sx: &mut Sx<'_>, who: Seat, card: StrategyCard, primary: bool) -> Flow {
    let Some(card_facts) = facts::perform_facts(sx, who, card) else {
        gap(sx, TurnGap::NoRoster);
        return Flow::Halt;
    };
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
            let finished = loop {
                if halted(sx) {
                    return Flow::Halt;
                }
                match child.step(sx) {
                    None => {
                        gap(sx, TurnGap::ReplayDiverged);
                        return Flow::Halt;
                    }
                    Some(Ran::Done(report)) => break fx_perform::finish(report, sx.state, card_facts.carried.trade_source),
                    Some(Ran::Asks(_)) => {
                        let Some(choice) = child.ask_of(sx.state, sx.kernel) else {
                            gap(sx, TurnGap::ReplayDiverged);
                            return Flow::Halt;
                        };
                        let pick = sx.ask(&choice);
                        if halted(sx) {
                            return Flow::Halt;
                        }
                        if child.answer(sx, pick).is_err() {
                            gap(sx, TurnGap::ReplayDiverged);
                            return Flow::Halt;
                        }
                    }
                    Some(Ran::Pass(next)) => {
                        for n in next.flatten() {
                            match n {
                                Next::Emit(rec) => {
                                    if sx.emit(rec).is_err() {
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
            announce(sx, who, finished)
        }
    }
}

/// What the finished ability announces (the string stages it after the window).
fn announce(sx: &mut Sx<'_>, _who: Seat, done: Finished) -> Flow {
    if done.failed {
        gap(sx, TurnGap::StrategyAbility);
        return Flow::Halt;
    }
    if let Some((system, seats)) = done.tokens {
        place_tokens(sx, system, seats.as_slice());
    }
    if done.made.as_ref().is_some_and(|made| !made.as_slice().is_empty()) {
        gap(sx, TurnGap::ProductionAnnounce);
        return Flow::Halt;
    }
    Flow::Done
}

/// `stage_command_token_placed(.., Reinforcements)` for each seat that got a token of the Diplomacy primary, in seating order.
fn place_tokens(sx: &mut Sx<'_>, system: System, seats: &[Seat]) {
    let pool = sx.table.codes.reinforcements;
    for seat in seats {
        let payload = Payload::new().with(Key::Player, Val::Seat(*seat)).with(Key::System, Val::System(system)).with(Key::Pool, Val::Code(pool));
        fx::stage(sx.state, fx::event(EventKind::CommandTokenPlaced, payload));
    }
}

/// `Game::resolve_faction_strategy`: what a faction does when a strategy card finishes for it (`faction_abilities::strategy_resolved`): the hooks of
/// the modules, then Xxcha's Peace Accords after Diplomacy.
pub fn resolved(sx: &mut Sx<'_>, who: Seat, kind: StrategyCardKind) -> Flow {
    let kernel = sx.kernel;
    {
        let mut cx = sx.cx();
        kernel.hooks.general.strategy_resolved(&mut cx, who, Some(kind));
    }
    if kind == StrategyCardKind::Diplomacy && sx.state.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheXxchaKingdom) {
        gap(sx, TurnGap::PeaceAccords);
        return Flow::Halt;
    }
    Flow::Done
}
