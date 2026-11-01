//! naaz: Garv and Gunn (the agent), Ssruu's copy of it and Dart and Tai (the commander) (`factions::naaz::{agent, borrowed_agent, commander}`).
//! Both explore a planet as a child of the ability: the exploration is the window of the explore area (`Exploring`), stepped on the tape, with no announcement
//! (the string explores with no timing handle).
//!
//! * Garv and Gunn (TURN_PASSED, optional): "at the end of a player's turn: you may exhaust this card to explore 1 planet that player controls". The ENDING player is
//!   asked which planet, the agent's owner only accepts the window.
//! * Dart and Tai (PLANET_CONTROL_GAINED, optional): "after you gain control of a planet that was controlled by another player: explore that planet".

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, Planet};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Runner, Slot, Spec, Unknown, Unported};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::stage::{Child, Ran};
use ti4_engine_core::kernel::{fx_strategy, Kernel};
use ti4_engine_core::query::{board, leaders};
use ti4_engine_core::rules::explore_windows::{traits_now, Exploring};
use ti4_engine_core::state::{CoreState, EDeck, EventKind, Key, LeaderStatus};

use crate::common::scope;

/// `leader:*:naazagent:TURN_PASSED:after`.
pub const AGENT: Slot = Slot(42_070);
/// `leader:*:naazcommander:PLANET_CONTROL_GAINED:after`.
pub const COMMANDER: Slot = Slot(42_071);
/// Ssruu copying Garv and Gunn (`leader:{lender faction}:{lender}:yssarilagent:naazagent:TURN_PASSED:after`).
pub const AGENT_COPY: Slot = Slot(42_072);

/// The most steps one exploration takes on the tape (its questions are few and short).
const STEPS: u8 = 32;

/// `explorable`: the planet has a trait now whose deck still holds a card.
fn explorable(s: &CoreState, k: &Kernel, planet: Planet) -> bool {
    let traits = traits_now(s, scope(k), planet);
    !traits.is_empty() && traits.as_slice().iter().any(|deck| s.decks().explore_present & (1 << *deck as u8) != 0 && !s.decks().explore[*deck as usize].is_empty())
}

/// `explorable_planets`: the controlled planets of `actor` that can be explored, in the order of the controlled planets.
fn explorable_planets(s: &CoreState, k: &Kernel, actor: Seat) -> Vec<Planet> {
    board::controlled_planets(s, actor).as_slice().iter().map(|(_, planet)| *planet).filter(|planet| explorable(s, k, *planet)).collect()
}

fn status(s: &CoreState, who: Seat, card: Leader) -> Option<LeaderStatus> {
    leaders::status(s, who, card)
}

/// `explore_planet`: the exploration window of `planet` for `who`, run to its end on the tape.
fn explore_planet(eff: &mut Eff<'_, '_>, who: Seat, planet: Planet) {
    let unsupported = eff.inline().with_sx(|sx| {
        let mut child: Child<Exploring> = Child::idle();
        child.start(Exploring::planet(who, planet, EDeck::Choose, false));
        for _ in 0..STEPS { // mutants:skip (a bound)
            match child.step(sx) {
                None => return None,
                Some(Ran::Done(done)) => return done.run().why,
                Some(Ran::Pass(_)) => return None,
                Some(Ran::Asks(_)) => {
                    let Some(choice) = child.ask_of(sx.state, sx.kernel) else { return None };
                    let pick = sx.ask(&choice);
                    if child.answer(sx, pick).is_err() { // mutants:skip (the tape holds only answers the question offered)
                        return None;
                    }
                }
            }
        }
        None
    });
    if let Some(why) = unsupported {
        eff.unknown(Unknown::Unported(Unported::Exploration(why)));
    }
}

// -- Garv and Gunn ---------------------------------------------------------------------------------------------------------------

fn actor_of(event: &ti4_engine_core::state::EventRec) -> Option<Seat> {
    event.payload.seat(Key::Player)
}

fn agent_when(c: &CondCx<'_>) -> bool {
    status(c.state, c.def.owner, Leader::GarvAndGunn) == Some(LeaderStatus::Readied) && actor_of(c.event).is_some_and(|actor| !explorable_planets(c.state, c.kernel, actor).is_empty())
}

/// The question: the ending player picks the planet (`copy`: Ssruu's copy, which names the planet by its name).
fn ask_planet(eff: &mut Eff<'_, '_>, actor: Seat, planets: &[Planet], copy: bool) -> Option<Planet> {
    let choice = Choice {
        actor,
        ask: Ask::Reaction(RAsk::G(if copy { GAsk::NaazCopy } else { GAsk::NaazAgent })),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Leader(Leader::GarvAndGunn)),
            subtype: Subtype::Reaction(RSub::G(if copy { GSub::NaazCopy } else { GSub::NaazAgent })),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: planets.iter().map(|p| Opt::Reaction(ROpt::G(GOpt::Explore(*p)))).collect::<Opts>(),
    };
    match eff.ask(&choice) {
        Pick::Opt(Opt::Reaction(ROpt::G(GOpt::Explore(planet)))) if planets.contains(&planet) => Some(planet),
        _ => None,
    }
}

fn agent(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(actor) = actor_of(eff.event) else { return Ok(()) };
    if status(eff.read().0, me, Leader::GarvAndGunn) != Some(LeaderStatus::Readied) {
        return Ok(());
    }
    let planets = {
        let (s, k) = eff.read();
        explorable_planets(s, k, actor)
    };
    if planets.is_empty() {
        return Ok(());
    }
    let Some(planet) = ask_planet(eff, actor, &planets, false) else { return Ok(()) };
    {
        let (s, k) = eff.state_and_kernel();
        if status(s, me, Leader::GarvAndGunn) != Some(LeaderStatus::Readied) {
            return Ok(());
        }
        fx_strategy::exhaust_agent(s, k, me, Leader::GarvAndGunn);
    }
    explore_planet(eff, actor, planet);
    Ok(())
}

fn holds_card(s: &CoreState, who: Seat, card: Leader) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.contains_key(&card))
}

fn lends(s: &CoreState, borrower: Seat, lender: Seat) -> bool {
    holds_card(s, lender, Leader::GarvAndGunn) && holds_card(s, borrower, Leader::Ssruu)
}

fn copy_when(c: &CondCx<'_>) -> bool {
    crate::cd_registry::yssaril::ssruu::borrowable(c.state, c.def.owner, c.def.pair_source(), Leader::GarvAndGunn)
        && actor_of(c.event).is_some_and(|actor| !explorable_planets(c.state, c.kernel, actor).is_empty())
}

fn copy(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let (me, lender) = (eff.owner(), eff.def.pair_source());
    let Some(actor) = actor_of(eff.event) else { return Ok(()) };
    if !crate::cd_registry::yssaril::ssruu::borrowable(eff.read().0, me, lender, Leader::GarvAndGunn) {
        return Ok(());
    }
    let planets = {
        let (s, k) = eff.read();
        explorable_planets(s, k, actor)
    };
    if planets.is_empty() {
        return Ok(());
    }
    let Some(planet) = ask_planet(eff, actor, &planets, true) else { return Ok(()) };
    {
        let (s, k) = eff.state_and_kernel();
        if status(s, me, Leader::Ssruu) != Some(LeaderStatus::Readied) {
            return Ok(());
        }
        fx_strategy::exhaust_agent(s, k, me, Leader::Ssruu);
    }
    explore_planet(eff, actor, planet);
    Ok(())
}

// -- Dart and Tai ------------------------------------------------------------------------------------------------------------------

/// The planet the owner just took from another player, when it can be explored and is still the owner's.
fn commander_planet(s: &CoreState, k: &Kernel, owner: Seat, event: &ti4_engine_core::state::EventRec) -> Option<Planet> {
    let p = &event.payload;
    let planet = p.planet(Key::Planet)?;
    (p.seat(Key::Player) == Some(owner) && p.seat(Key::PreviousOwner).is_some() && leaders::has_commander_ability_in(s, owner, Leader::DartAndTai) && explorable(s, k, planet) && board::controls(s, owner, planet)).then_some(planet)
}

fn commander_when(c: &CondCx<'_>) -> bool {
    commander_planet(c.state, c.kernel, c.def.owner, c.event).is_some()
}

fn commander(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let (Some(player), Some(planet)) = (eff.event.payload.seat(Key::Player), eff.event.payload.planet(Key::Planet)) else { return Ok(()) };
    let _ = me;
    explore_planet(eff, player, planet);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    use EventKind as E;
    t.add(Spec::new(AGENT, AbilityKind::Leader, E::TurnPassed, Relation::After, agent).optional().when(agent_when));
    t.add(Spec::new(COMMANDER, AbilityKind::Leader, E::PlanetControlGained, Relation::After, commander).optional().when(commander_when));
    t.add(Spec::new(AGENT_COPY, AbilityKind::Leader, E::TurnPassed, Relation::After, copy).applies(Applies::Faction(Faction::TheNaazRokhaAlliance)).pair(lends).optional().when(copy_when));
}
