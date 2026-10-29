//! nekro: Technological Singularity, "Once per combat, after 1 of your opponent's units is destroyed: you may gain 1
//! technology that is owned by that player", and the bookkeeping of its combat (docs/ENGINE_REACTIONS.md, package B12).
//!
//! * The marks (`nekro:ts:{system}[|{planet}]`, `MarkKey::NekroTs`) open when a combat of the Nekro starts (the opponent
//!   and `open`), turn `used` with the ability and are removed when the combat ends: four mandatory abilities.
//! * The ability (space: a ship destroyed during the combat; ground: a ground force destroyed by the combat) is optional.
//!   It is eligible while the mark is open for the owner of the destroyed unit, no faceup Antivirus of the Nekro sits in
//!   that player's area and the opponent has something to take. Used, it asks `gain a technology, or assimilate instead`
//!   (the options of `nekro::assimilation_options`: gain a technology the Nekro lacks, or put the X or Y token of a Valefar
//!   Assimilator on a faction technology) and applies the answer.
//!
//! Not covered (the window bails, tests `bail_*`): the Z token of the Valefar Assimilator breakthrough, which lends a
//! flagship's text (the Nekro holds the breakthrough and an opponent could take from it), and a gained technology that
//! changes the form of the Nekro's units (a unit upgrade, or a technology some unit requires): the string swaps the units
//! on the board.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub, Take};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Breakthrough, Faction, Leader, Requirement, Technology, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{Applies, CondCx, Eff, Slot, Spec, Unknown};
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

fn opens(c: &CondCx<'_>) -> bool {
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

fn holds_z(s: &CoreState, nekro: Seat) -> bool {
    s.players().by_seat(nekro).is_some_and(|p| p.cards.breakthrough == Some(Breakthrough::ValefarAssimilatorZ))
}

/// `nekro::assimilation_options` without the Z token: what the Nekro can take of `source`'s technologies, in the string's
/// order (technology order, the gain before the tokens).
fn options(s: &CoreState, k: &Kernel, nekro: Seat, source: Seat) -> Vec<(Take, Technology)> {
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
    (!antivirus_bars(s, nekro, opponent) && !options(s, k, nekro, opponent).is_empty()).then_some((key, opponent))
}

fn singularity_when(c: &CondCx<'_>) -> bool {
    let slot = Slot(u16::try_from(c.def.arg).unwrap_or(0));
    let ground = slot == GROUND;
    let me = c.def.owner;
    // The ability the Z token changes (a flagship's text lent): the string offers the token as an option.
    let got = ready(c.state, c.kernel, &c.event.payload, &c.codes, me, ground);
    if got.is_some() && holds_z(c.state, me) {
        return c.unknown(Unknown::MissingFact);
    }
    got.is_some()
}

/// A technology whose gain changes the form of a unit the Nekro owns (`technology::apply_unit_upgrades`): the core does not
/// swap the units on the board.
fn changes_unit_forms(t: Technology) -> bool {
    ti4_engine_core::query::tech::is_unit_upgrade(t)
        || Unit::ALL.iter().any(|u| matches!(u.data().required_technology, Requirement::Known(r) if r == t))
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
        options(s, k, me, opponent)
    };
    if offered.is_empty() {
        return Ok(());
    }
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::Assimilate),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Other(codes.technological_singularity)),
            subtype: Subtype::Reaction(RSub::Assimilate),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: offered.iter().map(|(how, tech)| Opt::Reaction(ROpt::Take { how: *how, tech: *tech, source: opponent })).collect::<Opts>(),
    };
    let Pick::Opt(Opt::Reaction(ROpt::Take { how, tech, .. })) = eff.ask(&choice) else { return Ok(()) };
    match how {
        Take::Gain => {
            if changes_unit_forms(tech) {
                eff.unknown(Unknown::MissingFact);
                return Ok(());
            }
            ti4_engine_core::kernel::fx::grant_technology(eff.state(), me, tech);
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
    let nekro = Applies::Faction(Faction::TheNekroVirus);
    t.add(Spec::new(SPACE, AbilityKind::Ability, EventKind::ShipDestroyed, Relation::After, singularity).applies(nekro).when(singularity_when).optional());
    t.add(Spec::new(GROUND, AbilityKind::Ability, EventKind::GroundForceDestroyed, Relation::After, singularity).applies(nekro).when(singularity_when).optional());
    for (slot, event, effect) in [
        (OPEN_SPACE, EventKind::SpaceCombatStarted, open as fn(&mut Eff<'_, '_>) -> Result<(), TimingError>),
        (CLOSE_SPACE, EventKind::SpaceCombatEnded, close),
        (OPEN_GROUND, EventKind::GroundCombatStarted, open),
        (CLOSE_GROUND, EventKind::GroundCombatEnded, close),
    ] {
        t.add(Spec::new(slot, AbilityKind::Ability, event, Relation::After, effect).applies(nekro).when(opens));
    }
}
