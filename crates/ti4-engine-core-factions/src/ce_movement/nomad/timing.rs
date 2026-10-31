//! nomad: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: The Cavalry, a promissory note of
//! the Nomad: played by its holder at the start of a space combat (the holder picks the non-fighter ship type that takes
//! the flagship's abilities; one type is not a question; the note stays faceup with them), and returned to the Nomad
//! when the combat ends. Future Sight (package W9): a trade good after an outcome the Nomad voted for or predicted plainly is
//! resolved. Artuno the Betrayer (package W9): the exhaust and the pile on the card; the payout when the card readies stays with the
//! string's `leaders::ready` callers (`nomad_agents::agent_readied`). The Thundarian (package W2): "after the Roll Dice step of combat: you may exhaust this card; if you do, return to the start of that step" (the replay mark the roll step reads). Not ported: Mercer, Temporal Command Suite.

use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, System, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::{fx, fx_strategy};
use ti4_engine_core::kernel::read::Rd;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::reach;
use ti4_engine_core::rules::combat;

use ti4_engine_core::keys::{Outcome, Prediction};
use ti4_engine_core::state::LeaderStatus;
use ti4_engine_core::state::{CoreState, EventKind, Key, TransientFlags, Val};

use super::query::{cavalry_ship_type, CAVALRY};

/// The Cavalry, played (`promissory:*:cavalry:SPACE_COMBAT_STARTED:after`).
pub const CAVALRY_PLAY: Slot = Slot(10_001);
/// The Cavalry, returned (`promissory:*:cavalry:SPACE_COMBAT_ENDED:after`).
pub const CAVALRY_RETURN: Slot = Slot(10_002);

/// Field Marshal Mercer (`leader:*:nomadagentmercer:TACTICAL_ACTION_ENDED:after`, lane M6d): at the end of a player's tactical action the player
/// removes up to 2 of their ground forces from the board and places them on a planet they control in the active system. Asked of the
/// ACTIVE player (the beneficiary), registered for every seat as the string does.
pub const MERCER: Slot = Slot(50_040);

/// Where one ground force stands: a system, and the planet (`None` in the space area).
type Place = (System, Option<ti4_engine_core::generated::Planet>);

/// `ground_forces`: the actor's ground forces on the board, in board order (a system's space area first, then its planets in id order). A
/// ground force that is also a structure (the Titans' PDS, Hel-Titan) is movable; a plain structure is not.
fn ground_forces(s: &CoreState, who: Seat) -> Vec<(Place, Unit, bool)> {
    use ti4_engine_core::query::{board, fleet};
    let mut found = Vec::new();
    for (system, cell) in s.board().systems() {
        for piece in cell.units.as_slice().iter().filter(|p| p.owner() == who && fleet::is_ground_force(p.kind())) {
            found.push(((system, None), piece.kind(), piece.sustained()));
        }
        let mut planets: Vec<_> = board::planets_of(s, system).collect();
        planets.sort_unstable();
        for planet in planets {
            for piece in board::planet_units(s, planet).iter().filter(|p| p.owner() == who && fleet::is_ground_force(p.kind())) {
                found.push(((system, Some(planet)), piece.kind(), piece.sustained()));
            }
        }
    }
    found
}

/// `landing_planets`: the planets of `system` the actor controls and may place units on, in id order.
fn landing_planets(s: &CoreState, who: Seat, system: System) -> Vec<ti4_engine_core::generated::Planet> {
    use ti4_engine_core::query::{board, laws};
    let blanked = reach::blanked(s);
    let mut planets: Vec<_> = board::planets_of(s, system).filter(|p| board::controls(s, who, *p) && !laws::planet_is_demilitarized(s, blanked, *p)).collect();
    planets.sort_unstable();
    planets
}

fn mercer_ready(s: &CoreState, owner: Seat) -> bool {
    s.players().by_seat(owner).is_some_and(|p| p.leaders.status.get(&Leader::FieldMarshalMercer) == Some(&LeaderStatus::Readied))
}

/// `mercer_window`: the actor and the active system of the end of a tactical action, when Mercer is readied for `owner`, a ground force can
/// move and a planet can take it.
fn mercer_window(s: &CoreState, owner: Seat, p: &ti4_engine_core::state::Payload) -> Option<(Seat, System)> {
    if !mercer_ready(s, owner) {
        return None;
    }
    let (actor, system) = (p.seat(Key::Player)?, p.system(Key::System)?);
    (!landing_planets(s, actor, system).is_empty() && !ground_forces(s, actor).is_empty()).then_some((actor, system))
}

fn mercer_when(c: &CondCx<'_>) -> bool {
    mercer_window(c.state, c.def.owner, &c.event.payload).is_some()
}

fn mercer_ctx(code: ti4_engine_core::state::Code, subtype: ti4_engine_core::choice::activation::Subtype) -> Ctx {
    Ctx {
        origin: Origin::FactionAbility(Content::Other(code)),
        subtype: Subtype::Reaction(RSub::Act(subtype)),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

/// `mercer_plan` and the effect: every question is asked before anything changes; the card is exhausted, then each unit is removed and put on
/// its planet fresh (undamaged).
fn mercer(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    use ti4_engine_core::choice::activation::{Ask as MAsk, Opt as MOpt, Subtype as MSub};
    let me = eff.owner();
    let found = {
        let (s, _) = eff.read();
        mercer_window(s, me, &eff.event.payload)
    };
    let Some((actor, system)) = found else { return Ok(()) };
    let code = eff.codes().activation.mercer;
    let mut pool = ground_forces(eff.read().0, actor);
    let landing = landing_planets(eff.read().0, actor, system);
    let mut taken: Vec<(Place, Unit, bool)> = Vec::new();
    for n in 1..=2_u8 {
        let mut opts: Opts = Opts::default();
        let mut seen: Vec<(Place, Unit, bool)> = Vec::new();
        for entry in &pool {
            if !seen.contains(entry) {
                seen.push(*entry);
                opts.push(Opt::Reaction(ROpt::Act(MOpt::Take { system: entry.0 .0, planet: entry.0 .1, unit: entry.1, damaged: entry.2 })));
            }
        }
        if seen.is_empty() {
            break;
        }
        opts.push(Opt::Decline);
        let choice = Choice { actor, ask: Ask::Reaction(RAsk::Act(MAsk::MercerTake { n })), ctx: mercer_ctx(code, MSub::MercerTake), opts };
        let Pick::Opt(Opt::Reaction(ROpt::Act(MOpt::Take { system: at, planet, unit, damaged }))) = eff.ask(&choice) else { break };
        let Some(position) = pool.iter().position(|e| e.0 == (at, planet) && e.1 == unit && e.2 == damaged) else { break };
        taken.push(pool.remove(position));
    }
    let mut plan: Vec<(Place, Unit, ti4_engine_core::generated::Planet)> = Vec::new();
    for (place, unit, damaged) in taken {
        let target = if let [only] = landing[..] {
            only
        } else {
            let opts: Opts = landing.iter().map(|p| Opt::Reaction(ROpt::Act(MOpt::Planet(*p)))).collect();
            let choice = Choice { actor, ask: Ask::Reaction(RAsk::Act(MAsk::MercerPlace { unit })), ctx: mercer_ctx(code, MSub::MercerPlace), opts };
            match eff.ask(&choice) {
                Pick::Opt(Opt::Reaction(ROpt::Act(MOpt::Planet(p)))) if landing.contains(&p) => p,
                _ => landing[0],
            }
        };
        let _ = damaged;
        plan.push((place, unit, target));
    }
    if plan.is_empty() {
        return Ok(());
    }
    let (state, kernel) = eff.state_and_kernel();
    if !fx_strategy::exhaust_agent(state, kernel, me, Leader::FieldMarshalMercer) {
        return Ok(());
    }
    let s = eff.state();
    for ((at, from), unit, target) in plan {
        let cell = match from {
            Some(planet) => &mut s.board_mut().planet_mut(planet).units,
            None => &mut s.board_mut().sys_mut(at).units,
        };
        if let Some(index) = cell.as_slice().iter().position(|p| p.owner() == actor && p.kind() == unit) {
            cell.remove(index);
        }
        fx::place_unit(s, actor, system, Some(target), unit);
    }
    Ok(())
}

/// The non-fighter ship types `who` has in the combat's system, in id order (the order of the enum is the order of the ids).
fn ship_types(s: &CoreState, k: &ti4_engine_core::kernel::Kernel, who: Seat, system: System) -> Vec<Unit> {
    let rd = Rd::new(s, k).with_blanked(reach::blanked(s));
    let mut kinds: Vec<Unit> = combat::ships_of(rd, who, system)
        .into_iter()
        .filter(|p| rd.data(p.kind()).is_some_and(|d| d.ship && d.base_type != UnitBaseType::Fighter))
        .map(|p| p.kind())
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    kinds
}

/// `cavalry_combat`: the system of a space combat `holder` may play the note at the start of.
fn playable_at(c: &CondCx<'_>) -> Option<System> {
    let (s, holder) = (c.state, c.def.owner);
    if s.cards().promissory.get(&CAVALRY) != Some(&holder) || s.cards().promissory_faceup.contains_key(&CAVALRY) {
        return None;
    }
    let nomad = |who: Seat| s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheNomad);
    if nomad(holder) {
        return None;
    }
    let p = &c.event.payload;
    let (attacker, defender) = (p.seat(Key::Attacker)?, p.seat(Key::Defender)?);
    let opponent = if holder == attacker {
        defender
    } else if holder == defender {
        attacker
    } else {
        return None;
    };
    if nomad(opponent) || s.players().by_seat(opponent).is_none() {
        return None;
    }
    let system = p.system(Key::System)?;
    (!ship_types(s, c.kernel, holder, system).is_empty()).then_some(system)
}

fn play_condition(c: &CondCx<'_>) -> bool {
    playable_at(c).is_some()
}

fn play(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let holder = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    let kinds = {
        let (s, k) = eff.read();
        ship_types(s, k, holder, system)
    };
    let Some(&first) = kinds.first() else { return Ok(()) };
    let kind = if kinds.len() == 1 {
        first
    } else {
        let source = eff.codes().cavalry;
        let choice = Choice {
            actor: holder,
            ask: Ask::Reaction(RAsk::Cavalry),
            ctx: Ctx {
                origin: Origin::FactionAbility(Content::Other(source)),
                subtype: Subtype::Reaction(RSub::Cavalry),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: kinds.iter().map(|u| Opt::Reaction(ROpt::Ship(*u))).collect::<Opts>(),
        };
        match eff.ask(&choice) {
            Pick::Opt(Opt::Reaction(ROpt::Ship(u))) if kinds.contains(&u) => u,
            _ => first,
        }
    };
    eff.state().cards_mut().promissory_faceup.edit().insert(CAVALRY, ());
    eff.state().marks_mut().tail.edit().insert(MarkKey::NomadCavalry(holder), MarkVal::Rec(vec![MarkVal::Atom(Atom::System(system)), MarkVal::Atom(Atom::Unit(kind))]));
    Ok(())
}

fn return_condition(c: &CondCx<'_>) -> bool {
    c.event.payload.system(Key::System).is_some_and(|system| cavalry_ship_type(c.state, c.def.owner, system).is_some())
}

fn give_back(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let holder = eff.owner();
    eff.state().marks_mut().tail.edit().remove(&MarkKey::NomadCavalry(holder));
    fx::give_back_note(eff.state(), CAVALRY);
    Ok(())
}

/// Future Sight (`ability:*:future_sight:AGENDA_RESOLVED:after`).
pub const FUTURE_SIGHT: Slot = Slot(10_005);

/// The outcome an `AGENDA_RESOLVED` event names (`player`: the elected seat, else the outcome itself).
fn resolved_outcome(p: &ti4_engine_core::state::Payload) -> Option<Outcome> {
    match p.get(Key::Player)? {
        Val::Seat(seat) => Some(Outcome::Seat(seat)),
        Val::Outcome(outcome) => Some(outcome),
        _ => None,
    }
}

/// "During the Agenda phase, after an outcome that you voted for or predicted is resolved": the vote is mirrored on the state
/// while the event is announced, a prediction is held (a bare one: a rider's tagged prediction is another text in the string and
/// does not match), and an agenda a Deadly Plot discarded has no resolved outcome.
fn future_sight_pays(s: &CoreState, me: Seat, resolved: &ti4_engine_core::state::Payload) -> bool {
    // The string registers the ability for every seat; the condition names the Nomad.
    if s.players().by_seat(me).is_none_or(|p| p.faction != Faction::TheNomad) {
        return false;
    }
    let Some(outcome) = resolved_outcome(resolved) else { return false };
    let agenda = s.agenda();
    !s.transient().misc.flags.has(TransientFlags::AGENDA_DISCARDED)
        && (agenda.votes.get(&me) == Some(&outcome) || agenda.predictions.get(&me) == Some(&Prediction::Plain(outcome)))
}

fn future_sight_condition(c: &CondCx<'_>) -> bool {
    future_sight_pays(c.state, c.def.owner, &c.event.payload)
}

fn future_sight(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let source = eff.codes().future_sight;
    let (_, announce) = fx::gain_trade_goods_announced(eff.state(), me, 1, source);
    if let Some(rec) = announce {
        eff.emit(rec)?;
    }
    Ok(())
}

/// Artuno the Betrayer (`leader:*:nomadagentartuno:TRADE_GOODS_GAINED:after`).
pub const ARTUNO: Slot = Slot(10_006);

/// The pile on Artuno's card (`nomad:artuno:{owner}`).
fn pile(s: &CoreState, owner: Seat) -> u32 {
    match s.marks().tail.get(&MarkKey::NomadArtuno(owner)) {
        Some(MarkVal::Atom(Atom::Num(n))) => *n,
        _ => 0,
    }
}

/// "When you gain trade goods from the supply": the amount gained by `owner` when the source is none of the gains that are not
/// from the supply (goods another player gave, took or paid, and Artuno's own payout).
fn supply_gain(c: &CondCx<'_>) -> Option<u32> {
    supply_gain_of(&c.event.payload, c.def.owner, &c.codes)
}

fn supply_gain_of(p: &ti4_engine_core::state::Payload, owner: Seat, codes: &ti4_engine_core::kernel::ability::Codes) -> Option<u32> {
    let amount = u32::try_from(p.int(Key::Amount)?).ok().filter(|n| *n > 0)?;
    let from_supply = p.seat(Key::Player) == Some(owner) && p.code(Key::Source).is_none_or(|source| !codes.not_from_supply.contains(&source));
    from_supply.then_some(amount)
}

fn artuno_ready(s: &CoreState, owner: Seat) -> bool {
    s.players().by_seat(owner).is_some_and(|p| p.leaders.status.get(&Leader::ArtunoTheBetrayer) == Some(&LeaderStatus::Readied))
}

/// Put `amount` more trade goods on Artuno's card.
fn add_to_pile(s: &mut CoreState, owner: Seat, amount: u32) {
    let total = pile(s, owner) + amount;
    s.marks_mut().tail.edit().insert(MarkKey::NomadArtuno(owner), MarkVal::Atom(Atom::Num(total)));
}

fn artuno_condition(c: &CondCx<'_>) -> bool {
    supply_gain(c).is_some() && artuno_ready(c.state, c.def.owner)
}

/// "You may exhaust this card to place an equal number of trade goods on this card." The gain itself still happens.
fn artuno(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let amount = {
        let p = &eff.event.payload;
        let Some(amount) = p.int(Key::Amount).and_then(|n| u32::try_from(n).ok()) else { return Ok(()) };
        amount
    };
    if !ti4_engine_core::rules::leaders::exhaust(eff.state(), me, Leader::ArtunoTheBetrayer) {
        return Ok(());
    }
    add_to_pile(eff.state(), me, amount);
    Ok(())
}

/// The Thundarian at the end of the "Roll Dice" step of a space combat (`leader:*:nomadagentthundarian:SPACE_COMBAT_ROLL_STEP_ENDED:after`).
pub const THUNDARIAN_SPACE: Slot = Slot(10_003);
/// ... and of a ground combat (`GROUND_COMBAT_ROLL_STEP_ENDED`).
pub const THUNDARIAN_GROUND: Slot = Slot(10_004);

/// A readied The Thundarian in hand, and no replay already asked for in this step.
fn thundarian_condition(c: &CondCx<'_>) -> bool {
    c.state.players().by_seat(c.def.owner).is_some_and(|p| p.leaders.status.get(&Leader::TheThundarian) == Some(&LeaderStatus::Readied))
        && !c.state.marks().tail.contains_key(&MarkKey::PrivateCombatReplayRoll())
}

/// Exhaust the card and ask for the step to be played again (`combat::request_roll_replay`). A Nomad that hears every agent exhausted
/// (Temporal Command Suite) is told by the staged `AGENT_EXHAUSTED` of the exhaustion (`fx_strategy::exhaust_agent`).
fn thundarian(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let owner = eff.owner();
    let (state, kernel) = eff.state_and_kernel();
    let ready = state.players().by_seat(owner).is_some_and(|p| p.leaders.status.get(&Leader::TheThundarian) == Some(&LeaderStatus::Readied));
    if ready && fx_strategy::exhaust_agent(state, kernel, owner, Leader::TheThundarian) {
        eff.state().marks_mut().tail.edit().insert(MarkKey::PrivateCombatReplayRoll(), MarkVal::Atom(Atom::Absent));
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(MERCER, AbilityKind::Leader, EventKind::TacticalActionEnded, Relation::After, mercer).when(mercer_when).optional());
    t.add(Spec::new(THUNDARIAN_SPACE, AbilityKind::Leader, EventKind::SpaceCombatRollStepEnded, Relation::After, thundarian).when(thundarian_condition).optional());
    t.add(Spec::new(THUNDARIAN_GROUND, AbilityKind::Leader, EventKind::GroundCombatRollStepEnded, Relation::After, thundarian).when(thundarian_condition).optional());
    t.add(Spec::new(FUTURE_SIGHT, AbilityKind::Ability, EventKind::AgendaResolved, Relation::After, future_sight).when(future_sight_condition));
    t.add(Spec::new(ARTUNO, AbilityKind::Leader, EventKind::TradeGoodsGained, Relation::After, artuno).when(artuno_condition).optional());
    t.add(Spec::new(CAVALRY_PLAY, AbilityKind::Promissory, EventKind::SpaceCombatStarted, Relation::After, play).when(play_condition).optional());
    t.add(Spec::new(CAVALRY_RETURN, AbilityKind::Promissory, EventKind::SpaceCombatEnded, Relation::After, give_back).when(return_condition));
}

#[cfg(test)]
mod artuno_tests {
    use super::*;
    use ti4_engine_core::kernel::ability::Codes;
    use ti4_engine_core::state::{Code, Payload};

    fn gain(player: Seat, amount: i64, source: Option<Code>) -> Payload {
        let p = Payload::new().with(Key::Player, Val::Seat(player)).with(Key::Amount, Val::Int(amount));
        match source {
            Some(code) => p.with(Key::Source, Val::Code(code)),
            None => p,
        }
    }

    fn codes() -> Codes {
        Codes { not_from_supply: [Code(1), Code(2), Code(3), Code(4), Code(5)], ..Codes::default() }
    }

    fn nomad_table() -> (CoreState, ti4_engine_core::kernel::Kernel) {
        crate::common::testkit::table(&[Faction::TheNomad, Faction::TheEmiratesOfHacan])
    }

    #[test]
    fn future_sight_pays_a_vote_or_a_bare_prediction_for_the_resolved_outcome_only() {
        use ti4_engine_core::keys::PredictionTag;
        let (mut s, _) = nomad_table();
        let resolved = |o: Outcome| Payload::new().with(Key::Player, Val::Outcome(o));
        assert!(!future_sight_pays(&s, Seat(0), &resolved(Outcome::For)), "nothing voted or predicted");
        s.agenda_mut().votes.insert(Seat(0), Outcome::For);
        assert!(future_sight_pays(&s, Seat(0), &resolved(Outcome::For)));
        assert!(!future_sight_pays(&s, Seat(0), &resolved(Outcome::Against)), "another outcome");
        assert!(!future_sight_pays(&s, Seat(1), &resolved(Outcome::For)), "the other seat has no Future Sight (and no vote)");
        s.agenda_mut().votes.insert(Seat(1), Outcome::For);
        assert!(!future_sight_pays(&s, Seat(1), &resolved(Outcome::For)), "only a Nomad is paid, whoever voted");
        s.agenda_mut().votes.clear();
        s.agenda_mut().predictions.insert(Seat(0), Prediction::Tagged(Outcome::For, PredictionTag::TradeRider));
        assert!(!future_sight_pays(&s, Seat(0), &resolved(Outcome::For)), "a rider's prediction is another text");
        s.agenda_mut().predictions.insert(Seat(0), Prediction::Plain(Outcome::For));
        assert!(future_sight_pays(&s, Seat(0), &resolved(Outcome::For)), "a bare prediction");
        assert!(!future_sight_pays(&s, Seat(0), &resolved(Outcome::Against)));
        s.transient_mut().misc.flags.set(TransientFlags::AGENDA_DISCARDED);
        assert!(!future_sight_pays(&s, Seat(0), &resolved(Outcome::For)), "a discarded agenda resolves no outcome");
        s.transient_mut().misc.flags.clear(TransientFlags::AGENDA_DISCARDED);
        // An election names the elected seat as the outcome.
        s.agenda_mut().predictions.insert(Seat(0), Prediction::Plain(Outcome::Seat(Seat(1))));
        let elected = Payload::new().with(Key::Player, Val::Seat(Seat(1)));
        assert!(future_sight_pays(&s, Seat(0), &elected));
        assert!(!future_sight_pays(&s, Seat(0), &Payload::new()), "an event without an outcome");
    }

    #[test]
    fn artuno_needs_a_readied_card_and_its_pile_grows_with_each_copy() {
        let (mut s, _) = nomad_table();
        assert!(!artuno_ready(&s, Seat(0)), "no leader row at all");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::ArtunoTheBetrayer, LeaderStatus::Readied);
        assert!(artuno_ready(&s, Seat(0)));
        assert!(!artuno_ready(&s, Seat(1)), "the other seat holds no card");
        s.players_mut().by_seat_mut(Seat(0)).unwrap().leaders.status.insert(Leader::ArtunoTheBetrayer, LeaderStatus::Exhausted);
        assert!(!artuno_ready(&s, Seat(0)), "an exhausted card copies nothing");
        assert_eq!(pile(&s, Seat(0)), 0);
        add_to_pile(&mut s, Seat(0), 2);
        add_to_pile(&mut s, Seat(0), 3);
        assert_eq!(pile(&s, Seat(0)), 5, "the card keeps what earlier copies put on it");
        assert_eq!(pile(&s, Seat(1)), 0);
    }

    #[test]
    fn only_the_owners_gain_from_the_supply_is_copied() {
        let c = codes();
        assert_eq!(supply_gain_of(&gain(Seat(0), 2, Some(Code(9))), Seat(0), &c), Some(2), "a card's or a strategy card's gain");
        assert_eq!(supply_gain_of(&gain(Seat(0), 2, None), Seat(0), &c), Some(2), "no source named");
        for excluded in 1..=5 {
            assert_eq!(supply_gain_of(&gain(Seat(0), 2, Some(Code(excluded))), Seat(0), &c), None, "source {excluded}: a transaction, a pillage, extreme duress, a Trade Agreement or Artuno's own payout");
        }
        assert_eq!(supply_gain_of(&gain(Seat(1), 2, Some(Code(9))), Seat(0), &c), None, "another seat's gain");
        assert_eq!(supply_gain_of(&gain(Seat(0), 0, Some(Code(9))), Seat(0), &c), None, "nothing gained");
    }
}
