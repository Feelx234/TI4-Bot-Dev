//! arborec: Stymie, the promissory note of the Arborec (`factions::arborec::stymie`): "After another player moves ships into a system that contains 1 or more of your units: you may
//! place 1 command token from that player's reinforcements in any non-home system. Then, return this card to the Arborec player." Optional; registered for every seat
//! (`promissory:*:stymie:SHIP_MOVED:after`). The holder picks the system (asked with several, the string's order: the ids of the map's systems and of the board's entries); the token
//! comes from the MOVER's reinforcements and the placement is announced as a staged `COMMAND_TOKEN_PLACED` (pool `reinforcements`) where the game has a faction module seated.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent4::{Ask as UAsk, Opt as UOpt, Subtype as USub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Planet, PromissoryNote, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::{board, economy};
use ti4_engine_core::state::{CoreState, EventKind, Key, NoteKey, Payload, SysFlags, Val};

use crate::common::scope;

/// Stymie (`promissory:*:stymie:SHIP_MOVED:after`).
pub const STYMIE: Slot = Slot(44_003);

const NOTE: NoteKey = NoteKey { note: PromissoryNote::Stymie, faction: Faction::TheArborec };

/// `galaxy::is_home_system`: a system printing a faction's homeworld in the sources of the game.
fn is_home_system(k: &Kernel, system: System) -> bool {
    let scope = scope(k);
    Planet::ALL.iter().any(|p| {
        let d = p.data();
        d.homeworld_of.is_some() && d.system == Some(system) && scope.0 & ti4_engine_core::tables::source_bit(d.source) != 0 // mutants:skip (a map holds only systems of the sources in play)
    })
}

/// `stymie_systems`: the systems of the map that hold things plus the board's entries, in id order, that are not home systems and take the mover's token.
fn systems(s: &CoreState, k: &Kernel, mover: Seat) -> Vec<System> {
    let galaxy = s.galaxy();
    System::ALL
        .iter()
        .copied()
        .filter(|system| {
            let mapped = galaxy.is_some_and(|g| g.coord_of(*system).is_some());
            let entry = s.board().systems().any(|(touched, cell)| touched == *system && cell.flags.has(SysFlags::TOUCHED));
            (mapped || entry) && !galaxy.is_some_and(|g| g.is_hyperlane(*system))
        })
        .filter(|system| !is_home_system(k, *system))
        .filter(|system| economy::can_place_command_token(s, mover, *system, fx::tokens_held_by_mahact(s, mover)))
        .collect()
}

/// `has_units_in`: a unit of the holder in the system, in space or on a planet.
fn has_units_in(s: &CoreState, holder: Seat, system: System) -> bool {
    board::space_units(s, system).iter().any(|u| u.owner() == holder) || board::planets_of(s, system).any(|p| board::planet_units(s, p).iter().any(|u| u.owner() == holder))
}

/// `stymie_ready`: the mover and the systems its token can go to, when `holder` may play the note on this move.
fn ready(s: &CoreState, k: &Kernel, holder: Seat, event: &ti4_engine_core::state::EventRec) -> Option<(Seat, Vec<System>)> {
    let mover = event.payload.seat(Key::Player)?;
    let system = event.payload.system(Key::System)?;
    if mover == holder
        || s.cards().promissory.get(&NOTE) != Some(&holder)
        || s.players().by_seat(holder).is_some_and(|p| p.faction == Faction::TheArborec)
        || !has_units_in(s, holder, system)
    {
        return None;
    }
    let systems = systems(s, k, mover);
    (!systems.is_empty()).then_some((mover, systems))
}

fn condition(c: &CondCx<'_>) -> bool {
    ready(c.state, c.kernel, c.def.owner, c.event).is_some()
}

fn stymie(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let ready = {
        let (s, k) = eff.read();
        ready(s, k, me, eff.event)
    };
    let Some((mover, systems)) = ready else { return Ok(()) };
    let target = match systems.as_slice() {
        [only] => *only,
        _ => {
            let choice = Choice {
                actor: me,
                ask: Ask::Reaction(RAsk::G(GAsk::U(UAsk::Stymie { mover }))),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Promissory(PromissoryNote::Stymie)),
                    subtype: Subtype::Reaction(RSub::G(GSub::U(USub::Stymie))),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: systems.iter().map(|system| Opt::Reaction(ROpt::G(GOpt::U(UOpt::System(*system))))).collect::<Opts>(),
            };
            let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::U(UOpt::System(system))))) = eff.ask(&choice) else { return Ok(()) };
            if !systems.contains(&system) {
                return Ok(());
            }
            system
        }
    };
    let pool = eff.codes().reinforcements;
    let staging = eff.adapter_facts().staging;
    let s = eff.state();
    if economy::can_place_command_token(s, mover, target, fx::tokens_held_by_mahact(s, mover)) {
        fx::place_command_token(s, mover, target);
        if staging {
            let payload = Payload::new().with(Key::Player, Val::Seat(mover)).with(Key::System, Val::System(target)).with(Key::Pool, Val::Code(pool));
            fx::stage(s, fx::event(EventKind::CommandTokenPlaced, payload));
        }
        fx::give_back_note(s, NOTE);
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(STYMIE, AbilityKind::Promissory, EventKind::ShipMoved, Relation::After, stymie).optional().when(condition));
}
