//! saar: Ragh's Call, the promissory note of the Saar (`factions::saar::ragh_call`): "After you commit units to land on a planet: remove all Saar ground forces from that
//! planet and place them on another planet the Saar player controls. Then return this card to the Saar player."
//! Optional (the holder's own use / decline is the "may"); the Saar player picks the planet, asked before any unit moves. Registered for every seat
//! (`promissory:*:ragh:UNITS_COMMITTED:after`); the condition names the holder of the Saar's note and a planet to go to.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Planet, PromissoryNote, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, EventKind, Key, NoteKey, Payload, Piece};

/// Ragh's Call (`promissory:*:ragh:UNITS_COMMITTED:after`).
pub const RAGH: Slot = Slot(42_001);

const NOTE: NoteKey = NoteKey { note: PromissoryNote::RaghsCall, faction: Faction::TheClanOfSaar };

/// `ragh_targets`: the Saar seat, its ground forces on the planet and the other planets it controls (the string's order), when `holder` may play the note.
type Targets = (Seat, Vec<Piece>, Vec<(System, Planet)>);

fn targets(s: &CoreState, holder: Seat, planet: Planet) -> Option<Targets> {
    if s.cards().promissory.get(&NOTE) != Some(&holder) || s.players().by_seat(holder).is_some_and(|p| p.faction == Faction::TheClanOfSaar) {
        return None;
    }
    let saar = s.players().iter().find(|p| p.faction == Faction::TheClanOfSaar)?.seat;
    if saar == holder {
        return None;
    }
    let forces: Vec<Piece> = board::planet_units(s, planet).iter().copied().filter(|p| p.owner() == saar && p.kind().data().ground_force).collect();
    if forces.is_empty() {
        return None;
    }
    let destinations: Vec<(System, Planet)> = board::controlled_planets(s, saar).as_slice().iter().copied().filter(|(_, to)| *to != planet).collect();
    if destinations.is_empty() {
        return None;
    }
    Some((saar, forces, destinations))
}

/// The planet the holder committed to (`gained_by`: the event names the holder).
fn committed(p: &Payload, holder: Seat) -> Option<(System, Planet)> {
    (p.seat(Key::Player) == Some(holder)).then_some(())?;
    Some((p.system(Key::System)?, p.planet(Key::Planet)?))
}

fn condition(c: &CondCx<'_>) -> bool {
    committed(&c.event.payload, c.def.owner).is_some_and(|(_, planet)| targets(c.state, c.def.owner, planet).is_some())
}

fn call(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some((_, planet)) = committed(&eff.event.payload, me) else { return Ok(()) };
    let Some((saar, _, destinations)) = targets(eff.read().0, me, planet) else { return Ok(()) };
    let choice = Choice {
        actor: saar,
        ask: Ask::Reaction(RAsk::G(GAsk::T(TAsk::Ragh { planet }))),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Promissory(PromissoryNote::RaghsCall)),
            subtype: Subtype::Reaction(RSub::G(GSub::T(TSub::Ragh))),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: destinations.iter().map(|(to_system, to_planet)| Opt::Reaction(ROpt::G(GOpt::T(TOpt::Site { system: *to_system, planet: *to_planet })))).collect::<Opts>(),
    };
    let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::T(TOpt::Site { system: to_system, planet: to_planet })))) = eff.ask(&choice) else { return Ok(()) };
    if !destinations.contains(&(to_system, to_planet)) {
        return Ok(());
    }
    let s = eff.state();
    let moved: Vec<Piece> = board::planet_units(s, planet).iter().copied().filter(|p| p.owner() == saar && p.kind().data().ground_force).collect();
    let from = &mut s.board_mut().planet_mut(planet).units;
    for piece in &moved {
        if let Some(at) = from.as_slice().iter().position(|u| u == piece) {
            from.remove(at);
        }
    }
    let to = s.board_mut().planet_mut(to_planet);
    for piece in moved {
        to.units.push(piece);
    }
    to.flags.set(ti4_engine_core::state::PlanetFlags::UNITS_ENTRY, true);
    s.board_mut().sys_mut(to_system).flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
    fx::give_back_note(s, NOTE);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(RAGH, AbilityKind::Promissory, EventKind::UnitsCommitted, Relation::After, call).optional().when(condition));
}
