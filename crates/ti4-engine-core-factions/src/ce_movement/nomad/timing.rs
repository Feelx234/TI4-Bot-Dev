//! nomad: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: The Cavalry, a promissory note of
//! the Nomad: played by its holder at the start of a space combat (the holder picks the non-fighter ship type that takes
//! the flagship's abilities; one type is not a question; the note stays faceup with them), and returned to the Nomad
//! when the combat ends. Future Sight (package W9): a trade good after an outcome the Nomad voted for or predicted plainly is
//! resolved. Artuno the Betrayer (package W9): the exhaust and the pile on the card; the payout when the card readies stays with the
//! string's `leaders::ready` callers (`nomad_agents::agent_readied`). Not ported: Mercer, The Thundarian, Temporal Command Suite.

use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, System, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::read::Rd;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::reach;
use ti4_engine_core::rules::combat;
use ti4_engine_core::generated::Leader;
use ti4_engine_core::keys::{Outcome, Prediction};
use ti4_engine_core::state::LeaderStatus;
use ti4_engine_core::state::{CoreState, EventKind, Key, TransientFlags, Val};

use super::query::{cavalry_ship_type, CAVALRY};

/// The Cavalry, played (`promissory:*:cavalry:SPACE_COMBAT_STARTED:after`).
pub const CAVALRY_PLAY: Slot = Slot(10_001);
/// The Cavalry, returned (`promissory:*:cavalry:SPACE_COMBAT_ENDED:after`).
pub const CAVALRY_RETURN: Slot = Slot(10_002);

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
pub const FUTURE_SIGHT: Slot = Slot(10_003);

/// The outcome an `AGENDA_RESOLVED` event names (`player`: the elected seat, else the outcome itself).
fn resolved_outcome(c: &CondCx<'_>) -> Option<Outcome> {
    match c.event.payload.get(Key::Player)? {
        Val::Seat(seat) => Some(Outcome::Seat(seat)),
        Val::Outcome(outcome) => Some(outcome),
        _ => None,
    }
}

/// "During the Agenda phase, after an outcome that you voted for or predicted is resolved": the vote is mirrored on the state
/// while the event is announced, a prediction is held (a bare one: a rider's tagged prediction is another text in the string and
/// does not match), and an agenda a Deadly Plot discarded has no resolved outcome.
fn future_sight_condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    // The string registers the ability for every seat; the condition names the Nomad.
    if c.state.players().by_seat(me).is_none_or(|p| p.faction != Faction::TheNomad) {
        return false;
    }
    let Some(outcome) = resolved_outcome(c) else { return false };
    let agenda = c.state.agenda();
    !c.state.transient().misc.flags.has(TransientFlags::AGENDA_DISCARDED)
        && (agenda.votes.get(&me) == Some(&outcome) || agenda.predictions.get(&me) == Some(&Prediction::Plain(outcome)))
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
pub const ARTUNO: Slot = Slot(10_004);

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
    let total = pile(eff.state(), me) + amount;
    eff.state().marks_mut().tail.edit().insert(MarkKey::NomadArtuno(me), MarkVal::Atom(Atom::Num(total)));
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
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
