//! argent: Strike Wing (`factions::argent::{strike_wing, extra_die}`), the three abilities that read the STAGED roll of a unit ability
//! (`transient.combat.reroll_staging`: one entry per rolling unit type, with the unit typed in `RerollEntry::kind`, the hit number and the faces;
//! the hosts of the roll mirror it before they emit `UNIT_ABILITY_ROLLED`: the barrage, the space cannon offense and the bombardment).
//!
//! * Strike Wing Alpha II (`technology:*:swa2:UNIT_ABILITY_ROLLED:after`): "When this unit uses ANTI-FIGHTER BARRAGE, each result of 9 or 10 also
//!   destroys 1 of your opponent's infantry in the space area of the active system." Mandatory; the owner picks the type when the opponent has
//!   several. The victim is removed without an announcement (the string `remove`s it).
//! * Strike Wing Ambuscade (`promissory:*:ambuscade:UNIT_ABILITY_ROLLED:when`) and Trrakan Aun Zulok (`leader:*:argentcommander:...:when`): "When 1 or
//!   more of your units make a roll for a unit ability: choose 1 of those units to roll 1 additional die." Optional. The die is appended to the
//!   staged entry before the hits are read back, so it counts as if the unit had rolled it with the rest. The note goes home afterwards.
//!
//! All three are registered for every seat; the conditions limit them to the holder.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, DieSource, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, PromissoryNote, System, Technology, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::{fx, Kernel};
use ti4_engine_core::query::leaders;
use ti4_engine_core::rules::combat::cannon_guns::opponent_with_ships;
use ti4_engine_core::rules::combat::Rd;
use ti4_engine_core::state::{CoreState, EventKind, Key, NoteKey, RerollSet, RollPurpose};

/// `technology:*:swa2:UNIT_ABILITY_ROLLED:after`.
pub const SWA2: Slot = Slot(46_001);
/// `promissory:*:ambuscade:UNIT_ABILITY_ROLLED:when`.
pub const AMBUSCADE: Slot = Slot(46_002);
/// `leader:*:argentcommander:UNIT_ABILITY_ROLLED:when`.
pub const COMMANDER: Slot = Slot(46_003);

/// The ten-sided die.
const SIDES: u32 = 10;
/// A result of at least this destroys an infantry (Strike Wing Alpha II).
const DESTROYS_ON: u8 = 9;

const AMBUSCADE_NOTE: NoteKey = NoteKey { note: PromissoryNote::StrikeWingAmbuscade, faction: Faction::TheArgentFlight };

fn staged(s: &CoreState, who: Seat) -> Option<&RerollSet> {
    s.transient().combat.reroll_staging.iter().find(|(seat, _)| *seat == who).map(|(_, set)| set)
}

fn own_roll(c: &CondCx<'_>) -> bool {
    c.event.payload.seat(Key::Player) == Some(c.def.owner)
}

// -- Strike Wing Alpha II -------------------------------------------------------------------------

/// `strike_wing_hits`: the system, the opponent and the results of 9 or 10 on this seat's staged Alpha II dice. The staged roll of an Alpha II is
/// always its barrage (no other roll of a unit ability is staged with that type), so the kind of the set is not read.
fn swa2_hits(s: &CoreState, k: &Kernel, who: Seat) -> Option<(System, Seat, usize)> {
    if !s.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(Technology::StrikeWingAlphaII)) {
        return None;
    }
    let set = staged(s, who)?;
    let hits: usize = set.rolls.iter().filter(|e| e.kind == Some(Unit::StrikeWingAlphaII)).map(|e| e.faces.iter().filter(|f| **f >= DESTROYS_ON).count()).sum();
    if hits == 0 {
        return None;
    }
    let opponent = opponent_with_ships(Rd::new(s, k), who, set.system)?;
    Some((set.system, opponent, hits))
}

/// `strike_wing_victims`: the opponent's infantry in the space area, when the barrage earned a kill.
fn swa2_victims(s: &CoreState, k: &Kernel, who: Seat) -> Vec<Unit> {
    let Some((system, opponent, _)) = swa2_hits(s, k, who) else { return Vec::new() };
    s.board().sys(system).units.iter().filter(|p| p.owner() == opponent && p.kind().data().base_type == UnitBaseType::Infantry).map(|p| p.kind()).collect()
}

fn swa2_when(c: &CondCx<'_>) -> bool {
    own_roll(c) && !swa2_victims(c.state, c.kernel, c.def.owner).is_empty()
}

fn swa2(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some((system, opponent, hits)) = ({
        let (s, k) = eff.read();
        swa2_hits(s, k, me)
    }) else {
        return Ok(());
    };
    for _ in 0..hits {
        let victims = {
            let (s, k) = eff.read();
            swa2_victims(s, k, me)
        };
        // The distinct types, in the order they stand in the space area.
        let mut kinds: Vec<Unit> = Vec::new();
        for unit in victims {
            if !kinds.contains(&unit) {
                kinds.push(unit);
            }
        }
        let victim = match kinds.as_slice() {
            [] => return Ok(()),
            [only] => *only,
            many => {
                let choice = Choice {
                    actor: me,
                    ask: Ask::Reaction(RAsk::G(GAsk::StrikeWing)),
                    ctx: Ctx {
                        origin: Origin::FactionAbility(Content::Technology(Technology::StrikeWingAlphaII)),
                        subtype: Subtype::Reaction(RSub::G(GSub::StrikeWing)),
                        optional: false,
                        target: None,
                        outstanding: SmallList::new(),
                        space_battle: false,
                        invasion_seq: None,
                    },
                    opts: many.iter().map(|unit| Opt::Reaction(ROpt::G(GOpt::Infantry { unit: *unit, victim: opponent }))).collect::<Opts>(),
                };
                match eff.ask(&choice) {
                    Pick::Opt(Opt::Reaction(ROpt::G(GOpt::Infantry { unit, .. }))) if many.contains(&unit) => unit,
                    _ => return Ok(()),
                }
            }
        };
        fx::remove_unit(eff.state(), opponent, system, None, victim);
    }
    Ok(())
}

// -- the additional die ---------------------------------------------------------------------------

fn holds(s: &CoreState, who: Seat, source: DieSource) -> bool {
    match source {
        // The note is played by the seat that holds it, never by its owner.
        DieSource::Ambuscade => s.cards().promissory.get(&AMBUSCADE_NOTE) == Some(&who) && s.players().by_seat(who).is_some_and(|p| p.faction != Faction::TheArgentFlight),
        DieSource::Commander => leaders::has_commander_ability_in(s, who, Leader::TrrakanAunZulok),
    }
}

/// `rolling_units`: the staged entries of this seat's unit-ability roll that can take a die, one per (type, planet), as indices. The relic's
/// dice (no unit type) and entries without a hit number are skipped (the string's `unit_types.is_empty()`: here the typed `kind`).
fn rolling_units(s: &CoreState, who: Seat) -> Vec<u8> {
    let Some(set) = staged(s, who) else { return Vec::new() };
    let mut seen: Vec<(Unit, Option<ti4_engine_core::generated::Planet>)> = Vec::new();
    let mut found = Vec::new();
    for (index, entry) in set.rolls.iter().enumerate() {
        let Some(unit) = entry.kind else { continue };
        if entry.hits_on.is_none() {
            continue;
        }
        let key = (unit, entry.planet);
        if !seen.contains(&key) {
            seen.push(key);
            found.push(u8::try_from(index).unwrap_or(u8::MAX));
        }
    }
    found
}

fn extra_when(c: &CondCx<'_>, source: DieSource) -> bool {
    own_roll(c) && holds(c.state, c.def.owner, source) && !rolling_units(c.state, c.def.owner).is_empty()
}

fn ambuscade_when(c: &CondCx<'_>) -> bool {
    extra_when(c, DieSource::Ambuscade)
}

fn commander_when(c: &CondCx<'_>) -> bool {
    extra_when(c, DieSource::Commander)
}

fn extra_die(eff: &mut Eff<'_, '_>, source: DieSource, slot: Slot) -> Result<(), TimingError> {
    let me = eff.owner();
    if !holds(eff.read().0, me, source) {
        return Ok(());
    }
    let candidates = rolling_units(eff.read().0, me);
    let index = match candidates.as_slice() {
        [] => return Ok(()),
        [only] => *only,
        many => {
            let opts: Option<Opts> = {
                let set = staged(eff.read().0, me);
                set.and_then(|set| many.iter().map(|i| set.rolls.get(usize::from(*i)).and_then(|e| e.kind).map(|unit| Opt::Reaction(ROpt::G(GOpt::ExtraDie { entry: *i, unit })))).collect::<Option<Opts>>())
            };
            // (An entry with no typed unit cannot be named: the card's own staging always types it.)
            let Some(opts) = opts else { return Ok(()) };
            let origin = match source {
                DieSource::Ambuscade => Content::Promissory(PromissoryNote::StrikeWingAmbuscade),
                DieSource::Commander => Content::Leader(Leader::TrrakanAunZulok),
            };
            let choice = Choice {
                actor: me,
                ask: Ask::Reaction(RAsk::G(GAsk::ExtraDie(source))),
                ctx: Ctx {
                    origin: Origin::FactionAbility(origin),
                    subtype: Subtype::Reaction(RSub::G(GSub::ExtraDie)),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts,
            };
            match eff.ask(&choice) {
                Pick::Opt(Opt::Reaction(ROpt::G(GOpt::ExtraDie { entry, .. }))) if many.contains(&entry) => entry,
                _ => return Ok(()),
            }
        }
    };
    let hits_on = staged(eff.read().0, me).and_then(|set| set.rolls.get(usize::from(index))).and_then(|e| e.hits_on);
    let faces = eff.roll(me, 1, hits_on, RollPurpose::Ability(slot.0), SIDES);
    let list = eff.state().transient_mut().combat.reroll_staging.edit();
    if let Some(entry) = list.iter_mut().find(|(seat, _)| *seat == me).and_then(|(_, set)| set.rolls.get_mut(usize::from(index))) {
        entry.faces.extend(faces);
    }
    if source == DieSource::Ambuscade {
        fx::give_back_note(eff.state(), AMBUSCADE_NOTE);
    }
    Ok(())
}

fn ambuscade(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    extra_die(eff, DieSource::Ambuscade, AMBUSCADE)
}

fn commander(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    extra_die(eff, DieSource::Commander, COMMANDER)
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(SWA2, AbilityKind::Technology, EventKind::UnitAbilityRolled, Relation::After, swa2).when(swa2_when));
    t.add(Spec::new(AMBUSCADE, AbilityKind::Promissory, EventKind::UnitAbilityRolled, Relation::When, ambuscade).optional().when(ambuscade_when));
    t.add(Spec::new(COMMANDER, AbilityKind::Leader, EventKind::UnitAbilityRolled, Relation::When, commander).optional().when(commander_when));
}
