//! nekro: Technological Singularity, "Once per combat, after 1 of your opponent's units is destroyed: you may gain 1
//! technology that is owned by that player", and the bookkeeping of its combat (docs/ENGINE_REACTIONS.md, package B12).
//!
//! * The marks (`nekro:ts:{system}[|{planet}]`, `MarkKey::NekroTs`) open when a combat of the Nekro starts (the opponent
//!   and `open`), turn `used` with the ability and are removed when the combat ends: four mandatory abilities.
//! Registered for every seat, as the string does (each condition names the Nekro).
//!
//! * The ability (space: a ship destroyed during the combat; ground: a ground force destroyed by the combat) is optional.
//!   It is eligible while the mark is open for the owner of the destroyed unit, no faceup Antivirus of the Nekro sits in
//!   that player's area and the opponent has something to take. Used, it asks `gain a technology, or assimilate instead`
//!   (the options of `nekro::assimilation_options`: gain a technology the Nekro lacks, or put the X or Y token of a Valefar
//!   Assimilator on a faction technology) and applies the answer.
//!
//! A gained technology reaches the units on the board (`common::apply_unit_upgrades`, `technology::gain`).
//!
//! Not covered (the window bails, tests `bail_*`): the Z token of the Valefar Assimilator breakthrough, which lends a
//! flagship's text (the Nekro holds the breakthrough and an opponent could take from it), and a Mentak Nekro-less gain is
//! not possible but a Mentak seat's own Corsair form of a unit is the string's override.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub, Take};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Breakthrough, Faction, Leader, Technology};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{CondCx, Eff, Slot, Spec, Unknown};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};
use ti4_engine_core::state::{CoreState, EventKind, Key, NoteKey, Payload};
use ti4_engine_core::tables::Scope;
use ti4_engine_core::generated::PromissoryNote;

/// The mark opens (`ability:*:technological_singularity_open:SPACE_COMBAT_STARTED:after`).
pub const OPEN_SPACE: Slot = Slot(2003);
/// The mark closes (`..._close:SPACE_COMBAT_ENDED:after`).
pub const CLOSE_SPACE: Slot = Slot(2004);
/// The mark opens (`..._open:GROUND_COMBAT_STARTED:after`).
pub const OPEN_GROUND: Slot = Slot(2005);
/// The mark closes (`..._close:GROUND_COMBAT_ENDED:after`).
pub const CLOSE_GROUND: Slot = Slot(2006);
/// The ability, space (`ability:*:technological_singularity:SHIP_DESTROYED:after`).
pub const SPACE: Slot = Slot(2007);
/// The ability, ground (`ability:*:technological_singularity:GROUND_FORCE_DESTROYED:after`).
pub const GROUND: Slot = Slot(2008);

const ASSIMILATORS: [(Take, Technology); 2] = [(Take::TokenX, Technology::ValefarAssimilatorX), (Take::TokenY, Technology::ValefarAssimilatorY)];

fn is_ground(slot: Slot) -> bool {
    matches!(slot, s if s == OPEN_GROUND || s == CLOSE_GROUND || s == GROUND)
}

/// The combat a payload belongs to: a space combat by its system, a ground combat by its system and planet.
fn combat_key(p: &Payload, ground: bool) -> Option<MarkKey> {
    let system = p.system(Key::System)?;
    Some(MarkKey::NekroTs(system, if ground { Some(p.planet(Key::Planet)?) } else { None }))
}

/// The opponent the Nekro has in the combat the event names.
fn opponent_in(p: &Payload, nekro: Seat) -> Option<Seat> {
    let (attacker, defender) = (p.seat(Key::Attacker)?, p.seat(Key::Defender)?);
    if attacker == nekro {
        Some(defender)
    } else if defender == nekro {
        Some(attacker)
    } else {
        None
    }
}

fn mark(opponent: Seat, word: Word) -> MarkVal {
    MarkVal::Rec(vec![MarkVal::Atom(Atom::Seat(opponent)), MarkVal::Atom(Atom::Word(word))])
}

/// The opponent and status of the mark, if there is one.
fn read(s: &CoreState, key: MarkKey) -> Option<(Seat, Word)> {
    match s.marks().tail.get(&key) {
        Some(MarkVal::Rec(v)) => match v.as_slice() {
            [MarkVal::Atom(Atom::Seat(who)), MarkVal::Atom(Atom::Word(word))] => Some((*who, *word)),
            _ => None,
        },
        _ => None,
    }
}

/// The string registers all six abilities for every seat (the condition names the Nekro), so the core does too.
pub(super) fn is_nekro(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheNekroVirus)
}

fn opens(c: &CondCx<'_>) -> bool {
    if !is_nekro(c.state, c.def.owner) {
        return false;
    }
    let slot = Slot(u16::try_from(c.def.arg).unwrap_or(0));
    opponent_in(&c.event.payload, c.def.owner).is_some() && combat_key(&c.event.payload, is_ground(slot)).is_some()
}

fn open(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let slot = Slot(u16::try_from(eff.def.arg).unwrap_or(0));
    let (Some(key), Some(opponent)) = (combat_key(&eff.event.payload, is_ground(slot)), opponent_in(&eff.event.payload, me)) else { return Ok(()) };
    eff.state().marks_mut().tail.edit().insert(key, mark(opponent, Word::Open));
    Ok(())
}

fn close(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let slot = Slot(u16::try_from(eff.def.arg).unwrap_or(0));
    if let Some(key) = combat_key(&eff.event.payload, is_ground(slot)) {
        eff.state().marks_mut().tail.edit().remove(&key);
    }
    Ok(())
}

/// The Valefar Assimilator `card` of `who`: the technology it stands for while another player owns it
/// (`nekro::live_token`).
pub(super) fn live_token(s: &CoreState, who: Seat, card: Technology) -> Option<Technology> {
    let p = s.players().by_seat(who)?;
    if p.faction != Faction::TheNekroVirus || !p.tech.owned.contains(card) {
        return None;
    }
    let target = *p.tech.assimilated.get(&card)?;
    s.players().iter().any(|o| o.seat != who && o.tech.owned.contains(target)).then_some(target)
}

pub(super) fn holds_z(s: &CoreState, nekro: Seat) -> bool {
    s.players().by_seat(nekro).is_some_and(|p| p.cards.breakthrough == Some(Breakthrough::ValefarAssimilatorZ))
}

/// `nekro::assimilation_options` without the Z token: what the Nekro can take of `source`'s technologies, in the string's
/// order (technology order, the gain before the tokens).
pub(super) fn options(s: &CoreState, k: &Kernel, nekro: Seat, source: Seat) -> Vec<(Take, Technology)> {
    use ti4_engine_core::query::tech;
    let (Some(me), Some(them)) = (s.players().by_seat(nekro), s.players().by_seat(source)) else { return Vec::new() };
    if nekro == source {
        return Vec::new();
    }
    let active = tech::active(Scope(u32::from(k.sources.0)));
    let tokens_forbidden = ti4_engine_core::rules::leaders::has_commander_ability_with(s, source, Leader::NipAndTuck, || {
        k.hooks.direct.mahact_imperia_grants(s, source, Leader::NipAndTuck)
    });
    let targeted = |t: Technology| ASSIMILATORS.iter().any(|(_, card)| live_token(s, nekro, *card) == Some(t));
    let mut out = Vec::new();
    for t in them.tech.owned.iter() {
        if !active.contains(t) {
            continue;
        }
        let faction = tech::faction_of(t);
        if !me.tech.owned.contains(t) && faction.is_none_or(|tag| tech::tag_is(tag, me.faction)) {
            out.push((Take::Gain, t));
        }
        if faction.is_some() && !tokens_forbidden && !tech::is_unit_upgrade(t) && !targeted(t) && t != Technology::ExecutiveOrder {
            for (how, card) in ASSIMILATORS {
                if me.tech.owned.contains(card) && live_token(s, nekro, card).is_none() {
                    out.push((how, t));
                }
            }
        }
    }
    out
}

/// A faceup Antivirus of the Nekro in `opponent`'s area bars the ability against them.
fn antivirus_bars(s: &CoreState, nekro: Seat, opponent: Seat) -> bool {
    let Some(p) = s.players().by_seat(nekro) else { return false };
    let note = NoteKey { note: PromissoryNote::Antivirus, faction: p.faction };
    s.cards().promissory.get(&note) == Some(&opponent) && s.cards().promissory_faceup.contains_key(&note)
}

/// `nekro::singularity_ready`: the mark key and the opponent when the destruction in the event opens the ability.
fn ready(s: &CoreState, k: &Kernel, c: &Payload, codes: &ti4_engine_core::kernel::ability::Codes, nekro: Seat, ground: bool) -> Option<(MarkKey, Seat)> {
    ready_with(s, k, c, codes, nekro, ground, true)
}

/// [`ready`], optionally without the check that the opponent has something to take (the Z token is an option of its own).
fn ready_with(s: &CoreState, k: &Kernel, c: &Payload, codes: &ti4_engine_core::kernel::ability::Codes, nekro: Seat, ground: bool, needs_options: bool) -> Option<(MarkKey, Seat)> {
    if !is_nekro(s, nekro) {
        return None;
    }
    if ground {
        if c.code(Key::Cause) != Some(codes.ground_combat) {
            return None;
        }
    } else if c.flag(Key::DuringSpaceCombat) != Some(true) {
        return None;
    }
    let key = combat_key(c, ground)?;
    let (opponent, status) = read(s, key)?;
    if status != Word::Open || c.seat(Key::Player) != Some(opponent) {
        return None;
    }
    (!antivirus_bars(s, nekro, opponent) && (!needs_options || !options(s, k, nekro, opponent).is_empty())).then_some((key, opponent))
}

fn singularity_when(c: &CondCx<'_>) -> bool {
    let slot = Slot(u16::try_from(c.def.arg).unwrap_or(0));
    let ground = slot == GROUND;
    let me = c.def.owner;
    if !is_nekro(c.state, me) {
        return false;
    }
    // The ability the Z token changes (a flagship's text lent): the string offers the token as an option.
    // The token is an option even when the opponent has no technology to take, so the check of the options is the string's alone.
    if holds_z(c.state, me) {
        if ready_with(c.state, c.kernel, &c.event.payload, &c.codes, me, ground, false).is_some() {
            return c.unknown(Unknown::MissingFact);
        }
        return false;
    }
    ready(c.state, c.kernel, &c.event.payload, &c.codes, me, ground).is_some()
}

fn singularity(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let ground = Slot(u16::try_from(eff.def.arg).unwrap_or(0)) == GROUND;
    let codes = eff.codes();
    let (key, opponent) = {
        let (s, k) = eff.read();
        match ready(s, k, &eff.event.payload, &codes, me, ground) {
            Some(found) => found,
            None => return Ok(()),
        }
    };
    eff.state().marks_mut().tail.edit().insert(key, mark(opponent, Word::Used));
    let offered = {
        let (s, k) = eff.read();
        options(s, k, me, opponent).into_iter().map(|(how, tech)| (how, tech, opponent)).collect::<Vec<_>>()
    };
    take(eff, &offered, codes.technological_singularity)
}

/// "Gain a technology, or assimilate instead" (`nekro::take_from`): ask the Nekro to pick one of `offered` (what it can take of the
/// players the ability allows, in the order given) and apply it. The choice is mandatory once the ability is used; `source` is the
/// interned name of the ability (the question's origin). Nothing happens for an empty list.
pub(super) fn take(eff: &mut Eff<'_, '_>, offered: &[(Take, Technology, Seat)], source: ti4_engine_core::state::Code) -> Result<(), TimingError> {
    let me = eff.owner();
    if offered.is_empty() {
        return Ok(());
    }
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::Assimilate),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Other(source)),
            subtype: Subtype::Reaction(RSub::Assimilate),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: offered.iter().map(|(how, tech, from)| Opt::Reaction(ROpt::Take { how: *how, tech: *tech, source: *from })).collect::<Opts>(),
    };
    let Pick::Opt(Opt::Reaction(ROpt::Take { how, tech, .. })) = eff.ask(&choice) else { return Ok(()) };
    match how {
        Take::Gain => {
            // Mentak's Corsair form of a unit is the string's override of the form the units take.
            if eff.state().players().by_seat(me).is_some_and(|p| p.faction == Faction::TheMentakCoalition) {
                eff.unknown(Unknown::MissingFact);
                return Ok(());
            }
            ti4_engine_core::kernel::fx::grant_technology(eff.state(), me, tech);
            // `technology::gain`: a unit upgrade reaches the units already on the board.
            let scope = Scope(u32::from(eff.kernel().sources.0));
            crate::common::apply_unit_upgrades(eff.state(), scope, me);
        }
        Take::TokenX | Take::TokenY => {
            let card = if how == Take::TokenX { Technology::ValefarAssimilatorX } else { Technology::ValefarAssimilatorY };
            if let Some(p) = eff.state().players_mut().by_seat_mut(me) {
                p.tech.assimilated.insert(card, tech);
            }
        }
    }
    Ok(())
}

pub fn register_abilities(t: &mut ti4_engine_core::kernel::ability::AbilityTable) {
    t.add(Spec::new(SPACE, AbilityKind::Ability, EventKind::ShipDestroyed, Relation::After, singularity).when(singularity_when).optional());
    t.add(Spec::new(GROUND, AbilityKind::Ability, EventKind::GroundForceDestroyed, Relation::After, singularity).when(singularity_when).optional());
    for (slot, event, effect) in [
        (OPEN_SPACE, EventKind::SpaceCombatStarted, open as fn(&mut Eff<'_, '_>) -> Result<(), TimingError>),
        (CLOSE_SPACE, EventKind::SpaceCombatEnded, close),
        (OPEN_GROUND, EventKind::GroundCombatStarted, open),
        (CLOSE_GROUND, EventKind::GroundCombatEnded, close),
    ] {
        t.add(Spec::new(slot, AbilityKind::Ability, event, Relation::After, effect).when(opens));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::generated::System;
    use ti4_engine_core::kernel::ability::Codes;
    use ti4_engine_core::state::Code;

    const GROUND_COMBAT: Code = Code(7);
    const OTHER_CAUSE: Code = Code(9);

    fn codes() -> Codes {
        Codes { ground_combat: GROUND_COMBAT, ..Codes::default() }
    }

    /// A Nekro (seat 0) against a Hacan (seat 1) who owns Gravity Drive and a faction technology; the mark of the combat in
    /// `system` is open for seat 1.
    fn game(word: Word) -> (CoreState, Kernel, System) {
        let (mut s, k) = table(&[Faction::TheNekroVirus, Faction::TheEmiratesOfHacan]);
        let system = System::ALL[0];
        let them = s.players_mut().by_seat_mut(Seat(1)).unwrap();
        them.tech.owned.insert(Technology::GravityDrive);
        them.tech.owned.insert(Technology::QuantumDatahubNode);
        s.marks_mut().tail.edit().insert(MarkKey::NekroTs(system, None), mark(Seat(1), word));
        (s, k, system)
    }

    fn ship_destroyed(system: System, victim: Seat, in_combat: bool) -> Payload {
        use ti4_engine_core::state::Val;
        Payload::new().with(Key::System, Val::System(system)).with(Key::Player, Val::Seat(victim)).with(Key::DuringSpaceCombat, Val::Flag(in_combat))
    }

    #[test]
    fn bail_a_destruction_outside_the_combat_opens_nothing() {
        let (s, k, system) = game(Word::Open);
        assert!(ready(&s, &k, &ship_destroyed(system, Seat(1), true), &codes(), Seat(0), false).is_some());
        assert!(ready(&s, &k, &ship_destroyed(system, Seat(1), false), &codes(), Seat(0), false).is_none(), "a ship destroyed by a card is not in the combat");
    }

    #[test]
    fn only_a_nekro_seat_opens_it() {
        let (s, k, system) = game(Word::Open);
        assert!(ready(&s, &k, &ship_destroyed(system, Seat(1), true), &codes(), Seat(1), false).is_none(), "the string registers the ability for every seat; the condition names the Nekro");
    }

    #[test]
    fn only_the_opponents_unit_opens_it_and_only_while_the_mark_is_open() {
        let (s, k, system) = game(Word::Open);
        assert!(ready(&s, &k, &ship_destroyed(system, Seat(0), true), &codes(), Seat(0), false).is_none(), "the Nekro's own ship");
        let (s, k, system) = game(Word::Used);
        assert!(ready(&s, &k, &ship_destroyed(system, Seat(1), true), &codes(), Seat(0), false).is_none(), "once per combat");
    }

    #[test]
    fn a_ground_force_opens_it_only_when_the_combat_destroyed_it() {
        use ti4_engine_core::state::Val;
        let (mut s, k, system) = game(Word::Open);
        let planet = ti4_engine_core::generated::Planet::ALL[0];
        s.marks_mut().tail.edit().insert(MarkKey::NekroTs(system, Some(planet)), mark(Seat(1), Word::Open));
        let p = |cause| Payload::new().with(Key::System, Val::System(system)).with(Key::Planet, Val::Planet(planet)).with(Key::Player, Val::Seat(Seat(1))).with(Key::Cause, Val::Code(cause));
        assert!(ready(&s, &k, &p(GROUND_COMBAT), &codes(), Seat(0), true).is_some());
        assert!(ready(&s, &k, &p(OTHER_CAUSE), &codes(), Seat(0), true).is_none(), "a space cannon or a card is not the combat");
        let no_planet = Payload::new().with(Key::System, Val::System(system)).with(Key::Player, Val::Seat(Seat(1))).with(Key::Cause, Val::Code(GROUND_COMBAT));
        assert!(ready(&s, &k, &no_planet, &codes(), Seat(0), true).is_none(), "a ground combat is keyed by its planet");
    }

    #[test]
    fn a_faceup_antivirus_of_the_nekro_in_the_opponents_area_bars_it() {
        let (mut s, k, system) = game(Word::Open);
        let note = NoteKey { note: PromissoryNote::Antivirus, faction: Faction::TheNekroVirus };
        s.cards_mut().promissory.edit().insert(note, Seat(1));
        assert!(ready(&s, &k, &ship_destroyed(system, Seat(1), true), &codes(), Seat(0), false).is_some(), "in the opponent's hand: not placed");
        s.cards_mut().promissory_faceup.edit().insert(note, ());
        assert!(antivirus_bars(&s, Seat(0), Seat(1)));
        assert!(ready(&s, &k, &ship_destroyed(system, Seat(1), true), &codes(), Seat(0), false).is_none());
    }

    #[test]
    fn the_options_follow_the_strings_rules() {
        let (mut s, k, _) = game(Word::Open);
        // Gravity Drive is generic: gain only (a generic card carries no token). The faction technology (Hacan's) is not the
        // Nekro's to gain, and a token needs a Valefar Assimilator the Nekro owns.
        assert_eq!(options(&s, &k, Seat(0), Seat(1)), vec![(Take::Gain, Technology::GravityDrive)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::ValefarAssimilatorX);
        assert_eq!(options(&s, &k, Seat(0), Seat(1)), vec![(Take::Gain, Technology::GravityDrive), (Take::TokenX, Technology::QuantumDatahubNode)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.assimilated.insert(Technology::ValefarAssimilatorX, Technology::QuantumDatahubNode);
        assert_eq!(options(&s, &k, Seat(0), Seat(1)), vec![(Take::Gain, Technology::GravityDrive)], "the technology carries a token already");
        assert!(options(&s, &k, Seat(0), Seat(0)).is_empty(), "nothing of one's own");
    }

}
