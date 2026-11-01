//! nekro: The Alastor, the flagship (`factions::nekro_units::alastor`): "At the start of a space combat, you may choose any number of ground forces in this system to participate in the
//! combat as if they were ships." Mandatory window whose every question may be answered "none": per planet and unit type (in the string's key order: the planet's id, then the unit's)
//! the owner says how many of his ground forces there fight as ships. The choice is the mark `NekroAlastor(activation, system, owner, planet, unit)` = the count; the combat reads it
//! (`nekro_units_alastor_participants`) and clears it at its end. Registered for every seat (`unit:*:nekro_flagship:SPACE_COMBAT_STARTED:after`).

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Planet, System, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, DecimalTextOrd, MarkKey, MarkVal};
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, EventKind, Key, Payload};

use super::code;

/// The Alastor (`unit:*:nekro_flagship:SPACE_COMBAT_STARTED:after`).
pub const ALASTOR: Slot = Slot(42_007);

/// `took_part`: the seat is a side of the combat the event names.
fn took_part(p: &Payload, who: Seat) -> bool {
    p.seat(Key::Attacker) == Some(who) || p.seat(Key::Defender) == Some(who)
}

/// `ground_forces_on_planets`: `who`'s ground forces on the planets of `system`, a count per planet and unit type, in the string's key order (the ids as text).
fn groups(s: &CoreState, who: Seat, system: System) -> Vec<(Planet, Unit, u32)> {
    let mut found: Vec<(Planet, Unit, u32)> = Vec::new();
    for planet in board::planets_of(s, system) {
        for piece in board::planet_units(s, planet) {
            if piece.owner() == who && piece.kind().data().ground_force {
                match found.iter_mut().find(|(p, u, _)| *p == planet && *u == piece.kind()) {
                    Some(entry) => entry.2 += 1,
                    None => found.push((planet, piece.kind(), 1)),
                }
            }
        }
    }
    // The generated enums are numbered in the order of their corpus ids (`UNIT_IDS`, the planets' ids): the derived order is the string's.
    found.sort_by_key(|(planet, unit, _)| (*planet, *unit));
    found
}

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    let Some(system) = c.event.payload.system(Key::System) else { return false }; // mutants:skip (no system in the event: `choose` ends at once too)
    took_part(&c.event.payload, me) && board::space_units(c.state, system).iter().any(|u| u.owner() == me && u.kind() == Unit::TheAlastor) && !groups(c.state, me, system).is_empty()
}

fn choose(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    let found = groups(eff.read().0, me, system);
    let origin = Origin::FactionAbility(Content::Other(eff.codes().silent.g3[code::NEKRO_FLAGSHIP]));
    let mut chosen: Vec<(Planet, Unit, u32)> = Vec::new();
    for (planet, unit, count) in found {
        let mut opts: Opts = (1..=count).map(|n| Opt::Reaction(ROpt::G(GOpt::T(TOpt::Count(n))))).collect();
        opts.push(Opt::Decline);
        let choice = Choice {
            actor: me,
            ask: Ask::Reaction(RAsk::G(GAsk::T(TAsk::Alastor { planet, unit }))),
            ctx: Ctx { origin, subtype: Subtype::Reaction(RSub::G(GSub::T(TSub::Alastor))), optional: false, target: None, outstanding: SmallList::new(), space_battle: false, invasion_seq: None }, // mutants:skip (the adapter puts the combat and the invasion into every nested context; the other fields are killed by the lockstep, this line is one struct literal)
            opts,
        };
        let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::T(TOpt::Count(n))))) = eff.ask(&choice) else { continue };
        if !(1..=count).contains(&n) {
            continue;
        }
        chosen.push((planet, unit, n));
    }
    let seq = DecimalTextOrd(eff.state().meta().activation_seq);
    for (planet, unit, n) in chosen {
        eff.state().marks_mut().tail.edit().insert(MarkKey::NekroAlastor(seq, system, me, planet, unit), MarkVal::Atom(Atom::Num(n)));
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(ALASTOR, AbilityKind::Unit, EventKind::SpaceCombatStarted, Relation::After, choose).when(condition));
}
