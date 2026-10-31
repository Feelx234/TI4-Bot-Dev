//! The hook chain of an activation (`Game::apply_tactical`, the activation arm after `SYSTEM_ACTIVATED`): what happens between the activation
//! of a system and the movement step, in the string's order.
//!
//! 1. The marks the string's stateful abilities write in the "after" window of `SYSTEM_ACTIVATED` and that the core faction crates have not
//!    ported (the Sardakk agent T'ro remembers the system for the end of the action; the Winnu note that a tactical action began).
//! 2. Mahact's commander: a system that held the seat's own token before the activation returns both tokens and ends the turn.
//! 3. Spatial Conduit Cylinders (use or decline; the map links), Scanlink Drone Network (a planet to explore, the exploration window).
//! 4. E-Res Siphons (automatic), Nullification Field (each holder in seat order; ends the turn), the Starlancer (each Mahact holder;
//!    ends the turn), Minister of Peace (the owner may discard the law; ends the turn).
//! 5. Ceasefire (automatic; it denies the movement step), the Dominus Orb (purge to move out of own tokens).
//!
//! The whole chain is ONE step: every question goes to the TAPE ([`Sx::ask`], the way the turn flow's jobs ask), so the driver rolls the step
//! back at a question, reports it, and replays the step with the answers when the caller returns (`Pending::nested`). Nothing of a half-run
//! chain is kept. A turn that ends (`After::EndTurn`) hands the position to the turn flow's `End` job.

use crate::choice::generic::Content;
use crate::choice::tactical::{Ask as TAsk, Hook, Opt as TOpt, Subtype as TSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::driver::turn::inline::{halted, run as run_window, Flow};
use crate::generated::{Agenda, Faction, Leader, System, Technology, Unit};
use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::kernel::{fx, fx_move, fx_strategy};
use crate::keys::{Atom, MarkKey, MarkVal, Word};
use crate::query::{board, laws, reach};
use crate::rules::agenda::offer_discard::OfferDiscard;
use crate::rules::explore_windows::{DominusOrb, Exploring};
use crate::state::{EDeck, ETarget, EventKind, EventRec, ExploreRun, Key, LeaderStatus, PlainArg, PlainKind, PlainRec, Val};

/// What the chain decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum After {
    /// On to the movement step.
    Move,
    /// A Ceasefire denies the movement: the step is skipped and the action closes (`finish_tactical`).
    Close,
    /// A hook ended the active seat's turn: the turn flow's `End` job runs next and the action leaves the stack.
    EndTurn,
    /// A question found the tape dry (the driver rolls the step back and asks it), or a gap was noted.
    Halt,
}

/// Run the chain for `seat`'s activation of `system`. `returns`: the system held the seat's own token before the activation.
pub(super) fn run(sx: &mut Sx<'_>, seat: Seat, system: System, returns: bool) -> After {
    marks(sx, seat, system);
    if returns {
        // "If you do, return both command tokens to your reinforcements and end your turn": the board holds one token per seat per system.
        sx.state.board_mut().sys_mut(system).cmd.remove(seat);
        return end_turn(sx, PlainRec::one(PlainKind::TurnEndedByMahactCommander, PlainArg::Seat(seat)));
    }
    if spatial_conduit(sx, seat, system) == Flow::Halt {
        return After::Halt;
    }
    if scanlink(sx, seat, system) == Flow::Halt {
        return After::Halt;
    }
    e_res_siphons(sx, seat, system);
    match nullification_field(sx, seat, system) {
        Err(Flow::Halt) => return After::Halt,
        Ok(Some(holder)) => return end_turn(sx, PlainRec::one(PlainKind::TurnEndedByNullificationField, PlainArg::Seat(holder))),
        _ => {}
    }
    match starlancer(sx, seat, system) {
        Err(Flow::Halt) => return After::Halt,
        Ok(Some(holder)) => return end_turn(sx, PlainRec::one(PlainKind::TurnEndedByStarlancer, PlainArg::Seat(holder))),
        _ => {}
    }
    match minister_of_peace(sx, seat, system) {
        Err(Flow::Halt) => return After::Halt,
        Ok(true) => return end_turn(sx, PlainRec::bare(PlainKind::TurnEndedByMinisterOfPeace)),
        _ => {}
    }
    if crate::query::promissory::denies_movement_into(sx.state, seat, system) && crate::rules::trade::use_ceasefire(sx.state, seat).unwrap_or(false) {
        sx.plain(PlainRec::bare(PlainKind::CeasefireUsed));
        return After::Close;
    }
    if run_window(sx, &mut DominusOrb::new(seat)) == Flow::Halt {
        return After::Halt;
    }
    After::Move
}

/// `self.tactical = None; active_system = None; pending = None; emit TURN_ENDED_BY_..; advance_turn()`: the action is over and the
/// turn passes on through the turn flow's `End` job (retention, Minister of War, the end-of-turn windows, the fleet sweep, `TURN_PASSED`).
fn end_turn(sx: &mut Sx<'_>, record: PlainRec) -> After {
    sx.state.meta_mut().active_system = None;
    sx.state.pending_mut().tag = None;
    fx_move::clear_conduit_links(sx.state);
    sx.plain(record);
    crate::driver::turn::open_job(sx.state, crate::state::TurnJob::End);
    After::EndTurn
}

/// The question of a hook: one offer, `Use` or decline. `None`: the tape is dry (the step is doomed).
fn offer(sx: &mut Sx<'_>, who: Seat, origin: Origin, ask: TAsk, subtype: TSub, options: Opts) -> Option<Pick> {
    let ctx = Ctx { origin, subtype: Subtype::Tactical(subtype), optional: false, target: None, outstanding: SmallList::new(), space_battle: false, invasion_seq: None };
    let choice = Choice { actor: who, ask: Ask::Tactical(ask), ctx, opts: options };
    let pick = sx.ask(&choice);
    (!halted(sx)).then_some(pick)
}

fn use_or_decline() -> Opts {
    [Opt::Tactical(TOpt::Use), Opt::Decline].into_iter().collect()
}

/// `technology::technology_text_ready`: the card that carries the technology's text is the seat's and is not exhausted.
fn ready(core: &crate::state::CoreState, who: Seat, tech: Technology) -> bool {
    fx_strategy::text_carrier(core, who, tech).is_some_and(|card| core.players().by_seat(who).is_some_and(|p| !p.tech.exhausted.contains(card)))
}

/// The "after you activate" marks of the Sardakk agent and of the Winnu.
fn marks(sx: &mut Sx<'_>, seat: Seat, system: System) {
    let sardakk: Vec<Seat> = sx
        .state
        .players()
        .iter()
        .filter(|p| p.faction == Faction::SardakkNorr)
        .filter(|p| matches!(p.leaders.status.get(&Leader::Tro), Some(LeaderStatus::Readied | LeaderStatus::Exhausted)))
        .map(|p| p.seat)
        .collect();
    if !sardakk.is_empty() {
        sx.state.marks_mut().tail.edit().insert(MarkKey::SardakkAgentTactical(seat), MarkVal::Atom(Atom::System(system)));
    }
    if sx.state.players().by_seat(seat).is_some_and(|p| p.faction == Faction::TheWinnu) {
        sx.state.marks_mut().tail.edit().insert(MarkKey::PrivateSeat(seat, Word::WinnuTactical), MarkVal::Atom(Atom::Num(1)));
    }
}

/// The other seats with a ship in `system`, in seat order (`faction_techs::others_with_ships`).
fn others_with_ships(core: &crate::state::CoreState, system: System, active: Seat) -> Vec<Seat> {
    core.players().iter().map(|p| p.seat).filter(|s| *s != active).filter(|s| board::space_units(core, system).iter().any(|u| u.owner() == *s && u.kind().data().ship)).collect()
}

/// Spatial Conduit Cylinders (`faction_techs::offer_spatial_conduit`).
fn spatial_conduit(sx: &mut Sx<'_>, seat: Seat, system: System) -> Flow {
    let s = &*sx.state;
    if !ready(s, seat, Technology::SpatialConduitCylinders) || !board::has_space_units_of(s, system, seat) {
        return Flow::Done;
    }
    // 22.3: nothing to connect unless the player has units somewhere else too.
    if !s.board().systems().any(|(other, cell)| other != system && cell.units.as_slice().iter().any(|u| u.owner() == seat)) {
        return Flow::Done;
    }
    let origin = Origin::Content(Content::Technology(Technology::SpatialConduitCylinders));
    let Some(pick) = offer(sx, seat, origin, TAsk::Hook { seat, hook: Hook::SpatialConduit, active: seat, system }, TSub::Hook(Hook::SpatialConduit), use_or_decline()) else { return Flow::Halt };
    if pick == Pick::Opt(Opt::Decline) {
        return Flow::Done;
    }
    let activation = sx.state.meta().activation_seq;
    fx_strategy::exhaust_technology_text(sx.state, seat, Technology::SpatialConduitCylinders);
    if let Some(p) = sx.state.players_mut().by_seat_mut(seat) {
        p.timers.spatial_conduit = crate::containers::Opt32::some(activation);
    }
    fx_move::set_conduit_links(sx.state, seat, system);
    sx.plain(PlainRec::two(PlainKind::SpatialConduit, PlainArg::Seat(seat), PlainArg::System(system)));
    Flow::Done
}

/// Scanlink Drone Network (`faction_techs::offer_scanlink`, then `exploration::explore_with`): "When you activate a system, you may explore 1
/// planet in that system which contains 1 or more of your units."
fn scanlink(sx: &mut Sx<'_>, seat: Seat, system: System) -> Flow {
    if !ready_owned(sx.state, seat, Technology::ScanlinkDroneNetwork) {
        return Flow::Done;
    }
    let scope = reach::scope_of(sx.kernel);
    let planets: Vec<crate::generated::Planet> = board::planets_of(sx.state, system)
        .into_iter()
        .filter(|p| board::planet_units(sx.state, *p).iter().any(|u| u.owner() == seat))
        .filter(|p| !crate::rules::explore_windows::traits_now(sx.state, scope, *p).is_empty())
        .collect();
    if planets.is_empty() {
        return Flow::Done;
    }
    let mut options: Opts = planets.iter().map(|p| Opt::Tactical(TOpt::Explore(*p))).collect();
    options.push(Opt::Decline);
    let origin = Origin::Content(Content::Technology(Technology::ScanlinkDroneNetwork));
    let Some(pick) = offer(sx, seat, origin, TAsk::Scanlink { seat, system }, TSub::ScanlinkExplore, options) else { return Flow::Halt };
    let Pick::Opt(Opt::Tactical(TOpt::Explore(planet))) = pick else { return Flow::Done };
    if !planets.contains(&planet) {
        return Flow::Done;
    }
    let mut window = Exploring::new(ExploreRun::new(seat, ETarget::Planet(planet), EDeck::Choose, true));
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    if window.explored() {
        sx.plain(PlainRec::one(PlainKind::ScanlinkExplored, PlainArg::Planet(planet)));
    }
    Flow::Done
}

/// The seat owns the technology itself (the string's Scanlink test is `technologies.contains("sdn")`, not the text carrier).
fn ready_owned(core: &crate::state::CoreState, who: Seat, tech: Technology) -> bool {
    core.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(tech))
}

/// E-Res Siphons (`faction_techs::e_res_siphons`): "After another player activates a system that contains 1 or more of your ships, gain 4 trade
/// goods." Not optional, not exhausted; the holders in seat order.
fn e_res_siphons(sx: &mut Sx<'_>, seat: Seat, system: System) {
    let holders: Vec<Seat> = others_with_ships(sx.state, system, seat).into_iter().filter(|s| sx.state.players().by_seat(*s).is_some_and(|p| p.tech.owned.contains(Technology::EResSiphons))).collect();
    let source = sx.table.codes.tactical.faction_technology;
    for holder in holders {
        fx::gain_trade_goods_staged(sx.state, holder, 4, source);
        sx.plain(PlainRec::one(PlainKind::EResSiphons, PlainArg::Seat(holder)));
    }
}

/// Nullification Field (`faction_techs::offer_nullification_field`): each eligible holder in seat order; the one that uses it ends the turn.
fn nullification_field(sx: &mut Sx<'_>, active: Seat, system: System) -> Result<Option<Seat>, Flow> {
    let holders: Vec<Seat> = others_with_ships(sx.state, system, active)
        .into_iter()
        .filter(|s| ready(sx.state, *s, Technology::NullificationField))
        .filter(|s| sx.state.players().by_seat(*s).is_some_and(|p| p.econ.strategic_tokens > 0))
        .collect();
    for holder in holders {
        let origin = Origin::Content(Content::Technology(Technology::NullificationField));
        let Some(pick) = offer(sx, holder, origin, TAsk::Hook { seat: holder, hook: Hook::NullificationField, active, system }, TSub::Hook(Hook::NullificationField), use_or_decline()) else { return Err(Flow::Halt) };
        if pick == Pick::Opt(Opt::Decline) {
            continue;
        }
        if !fx::spend_strategic_token(sx.state, holder) {
            continue;
        }
        fx_strategy::exhaust_technology_text(sx.state, holder, Technology::NullificationField);
        let reason = sx.table.codes.tactical.nullification_field;
        fx::stage(sx.state, EventRec { id: 0, kind: EventKind::StrategyTokenSpent, payload: crate::state::Payload::new().with(Key::Player, Val::Seat(holder)).with(Key::Reason, Val::Code(reason)), cancelled: false });
        return Ok(Some(holder));
    }
    Ok(None)
}

/// The owners of the other seats' command tokens in `mahact`'s fleet pool, one entry per token (`mahact::fleet_pool_owners`).
fn pool_owners(core: &crate::state::CoreState, mahact: Seat) -> Vec<Seat> {
    match core.marks().tail.get(&MarkKey::MahactFleet(mahact)) {
        Some(MarkVal::List(items)) => items.iter().filter_map(|v| if let MarkVal::Atom(Atom::Seat(s)) = v { Some(*s) } else { None }).collect(),
        _ => Vec::new(),
    }
}

/// `mahact::remove_token`: one of `owner`'s tokens leaves `mahact`'s fleet pool; `false`, changing nothing, when it holds none.
fn remove_pool_token(core: &mut crate::state::CoreState, mahact: Seat, owner: Seat) -> bool {
    let mut owners = pool_owners(core, mahact);
    let Some(at) = owners.iter().position(|s| *s == owner) else { return false };
    owners.remove(at);
    owners.sort_unstable();
    let marks = core.marks_mut().tail.edit();
    if owners.is_empty() {
        marks.remove(&MarkKey::MahactFleet(mahact));
    } else {
        marks.insert(MarkKey::MahactFleet(mahact), MarkVal::List(owners.into_iter().map(|s| MarkVal::Atom(Atom::Seat(s))).collect()));
    }
    true
}

/// Starlancer (`mahact_units::offer_starlancer`): "After a player whose command token is in your fleet pool activates this system, you may spend
/// their token from your fleet pool to end their turn; they gain that token." Each eligible Mahact holder in seating order; the activator gains
/// the token into a pool of its choice.
fn starlancer(sx: &mut Sx<'_>, active: Seat, system: System) -> Result<Option<Seat>, Flow> {
    let holders: Vec<Seat> = sx
        .state
        .seats()
        .seating_order()
        .iter()
        .copied()
        .filter(|h| *h != active)
        .filter(|h| {
            let s = &*sx.state;
            let mech_here = board::space_units(s, system).iter().chain(board::planets_of(s, system).into_iter().flat_map(|p| board::planet_units(s, p).iter())).any(|u| u.kind() == Unit::Starlancer && u.owner() == *h);
            mech_here && pool_owners(s, *h).contains(&active)
        })
        .collect();
    for holder in holders {
        let origin = Origin::FactionAbility(Content::Other(sx.table.codes.tactical.starlancer));
        let Some(pick) = offer(sx, holder, origin, TAsk::Hook { seat: holder, hook: Hook::Starlancer, active, system }, TSub::Hook(Hook::Starlancer), use_or_decline()) else { return Err(Flow::Halt) };
        if pick == Pick::Opt(Opt::Decline) {
            continue;
        }
        if !remove_pool_token(sx.state, holder, active) {
            continue;
        }
        // The activator gains that token "into a pool of their choice, like any command token gained".
        let choice = crate::choice::strategy::gain_pool_choice(active, 1);
        let pool = sx.ask(&choice);
        if halted(sx) {
            return Err(Flow::Halt);
        }
        let Pick::Opt(Opt::Strategy(crate::choice::strategy::Opt::Pool(pool))) = pool else { return Err(Flow::Halt) };
        fx::gain_token(sx.state, active, pool, 1);
        return Ok(Some(holder));
    }
    Ok(None)
}

/// `Game::minister_of_peace`: "After a player activates a system that contains 1 or more of a different player's units, the owner of this card
/// may discard this card -- immediately end the active player's turn." Whether the law was discarded.
fn minister_of_peace(sx: &mut Sx<'_>, active: Seat, system: System) -> Result<bool, Flow> {
    let contested = board::space_units(sx.state, system).iter().any(|u| u.owner() != active);
    if !contested {
        return Ok(false);
    }
    let Some(crate::state::Outcome::Seat(owner)) = laws::elected(sx.state, reach::blanked(sx.state), Agenda::MinisterOfPeace) else { return Ok(false) };
    if sx.state.players().by_seat(owner).is_none() {
        return Ok(false);
    }
    let codes = sx.table.codes.tactical;
    let mut window = OfferDiscard::new(Agenda::MinisterOfPeace, owner, codes.peace_prompt, codes.peace_source);
    if run_window(sx, &mut window) == Flow::Halt {
        return Err(Flow::Halt);
    }
    Ok(window.taken())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Faction;
    use crate::state::{Player, Players};

    fn three_seats() -> crate::state::CoreState {
        let mut s = crate::state::CoreState::all_loaded();
        s.players = Players::from_vec((0..3).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect());
        s
    }

    fn pool(s: &mut crate::state::CoreState, mahact: u8, owners: &[u8]) {
        let items = owners.iter().map(|o| MarkVal::Atom(Atom::Seat(Seat(*o)))).collect();
        s.marks_mut().tail.edit().insert(MarkKey::MahactFleet(Seat(mahact)), MarkVal::List(items));
    }

    #[test]
    fn a_token_leaves_the_fleet_pool_one_at_a_time_and_an_empty_pool_has_no_mark() {
        let mut s = three_seats();
        pool(&mut s, 0, &[2, 1, 2]);
        assert_eq!(pool_owners(&s, Seat(0)), vec![Seat(2), Seat(1), Seat(2)]);
        assert!(remove_pool_token(&mut s, Seat(0), Seat(2)));
        assert_eq!(pool_owners(&s, Seat(0)), vec![Seat(1), Seat(2)], "one token of seat 2 left, the rest sorted");
        assert!(!remove_pool_token(&mut s, Seat(0), Seat(0)), "the pool holds none of its own");
        assert!(remove_pool_token(&mut s, Seat(0), Seat(1)));
        assert!(remove_pool_token(&mut s, Seat(0), Seat(2)));
        assert!(!s.marks().tail.contains_key(&MarkKey::MahactFleet(Seat(0))), "the last token takes the mark with it");
        assert!(!remove_pool_token(&mut s, Seat(0), Seat(1)));
    }

    #[test]
    fn a_technology_text_is_ready_while_its_card_is_not_exhausted() {
        let mut s = three_seats();
        assert!(!ready(&s, Seat(0), Technology::NullificationField), "not owned");
        s.players.edit(0).tech.owned.insert(Technology::NullificationField);
        assert!(ready(&s, Seat(0), Technology::NullificationField));
        s.players.edit(0).tech.exhausted.insert(Technology::NullificationField);
        assert!(!ready(&s, Seat(0), Technology::NullificationField), "exhausted");
    }
}
