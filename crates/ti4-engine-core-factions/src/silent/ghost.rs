//! ghost: Sai Seravus, the commander of the Ghosts of Creuss (`factions::ghost::{commander_moved, commander_finished}`). "After your ships move: for each ship that has a
//! capacity value and moved through 1 or more wormholes, you may place 1 fighter from your reinforcements with that ship if you have unused capacity in the active
//! system." Each arrival through a wormhole is counted on a mark of the activation (`private:#ghost:commander:{seat}:{activation}`); the fighters are placed once the movement step
//! is over. Both halves are registered for every seat, the conditions limit them to the holder of the commander.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Leader, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::{fx, Kernel};
use ti4_engine_core::keys::{Atom, DecimalTextOrd, MarkKey, MarkVal};
use ti4_engine_core::query::{fleet, leaders};
use ti4_engine_core::state::{CoreState, EventKind, Key};

use crate::cd_registry::yin::timing::placed_form_overridden;
use crate::common::{mark, num, scope};

/// `leader:*:ghostcommander:SHIP_MOVED:after`.
pub const MOVED: Slot = Slot(42_030);
/// `leader:*:ghostcommander:MOVEMENT_FINISHED:after`.
pub const FINISHED: Slot = Slot(42_031);

fn key(s: &CoreState, owner: Seat) -> MarkKey {
    MarkKey::PrivateGhostCommander(owner, DecimalTextOrd(s.meta().activation_seq))
}

fn count(s: &CoreState, owner: Seat) -> u32 {
    mark(s, key(s, owner)).and_then(num).unwrap_or(0)
}

fn has_commander(s: &CoreState, who: Seat) -> bool {
    leaders::has_commander_ability_in(s, who, Leader::SaiSeravus)
}

/// `eligible`: a ship with a capacity value of the seat arrived in the active system through 1 or more wormholes.
fn eligible(s: &CoreState, event: &ti4_engine_core::state::EventRec, who: Seat) -> bool {
    let p = &event.payload;
    p.seat(Key::Player) == Some(who)
        && p.system(Key::System) == s.meta().active_system
        && has_commander(s, who)
        && p.int(Key::Wormholes).is_some_and(|hops| hops > 0)
        && p.unit(Key::Unit).is_some_and(|unit| unit.data().ship && unit.data().capacity.is_some_and(|c| c > 0))
}

fn moved_when(c: &CondCx<'_>) -> bool {
    eligible(c.state, c.event, c.def.owner)
}

fn moved(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let (s, _) = eff.read();
    if !eligible(s, eff.event, me) {
        return Ok(());
    }
    let (k, n) = (key(s, me), count(s, me));
    eff.state().marks_mut().tail.edit().insert(k, MarkVal::Atom(Atom::Num(n.saturating_add(1))));
    Ok(())
}

// mutants:skip (an eligible finish with no count only clears nothing)
fn finished_when(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.event.payload.seat(Key::Player) == Some(me) && has_commander(c.state, me) && count(c.state, me) > 0
}

/// `commander_fighters`: the mark's count, capped by the free capacity of the system and the fighters in the box.
fn available(s: &CoreState, k: &Kernel, who: Seat, system: ti4_engine_core::generated::System) -> usize {
    if !has_commander(s, who) {
        return 0;
    }
    // (The capacity of a place does not depend on the fleet limit or on how a fighter weighs: neither is read.)
    let never = |_: ti4_engine_core::generated::Unit| false; // mutants:skip
    let free = fleet::standing(s, scope(k), who, system, 0, None, &never).capacity_free(); // mutants:skip
    let plastic = fleet::remaining_all(s, scope(k), who, ti4_engine_core::generated::Unit::FighterI);
    (count(s, who) as usize).min(usize::try_from(free).unwrap_or(0)).min(usize::try_from(plastic).unwrap_or(0)) // mutants:skip (neither is negative)
}

fn finished(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    let k = key(eff.read().0, me);
    let n = {
        let (s, kernel) = eff.read();
        available(s, kernel, me, system)
    };
    if n == 0 {
        eff.state().marks_mut().tail.edit().remove(&k);
        return Ok(());
    }
    let mut opts: Opts = (1..=n).map(|count| Opt::Reaction(ROpt::G(GOpt::Fighters { count: u8::try_from(count).unwrap_or(u8::MAX), system }))).collect();
    opts.push(Opt::Decline);
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::G(GAsk::CommanderFighters)),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Leader(Leader::SaiSeravus)),
            subtype: Subtype::Reaction(RSub::G(GSub::CommanderFighters)),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts,
    };
    if let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::Fighters { count, .. }))) = eff.ask(&choice) {
        let (s, kernel) = eff.state_and_kernel();
        let form = placed_form_overridden(s, kernel, scope(kernel), me, UnitBaseType::Fighter);
        let _ = fx::place_counted(s, kernel, scope(kernel), me, system, None, form, usize::from(count));
    }
    eff.state().marks_mut().tail.edit().remove(&k);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(MOVED, AbilityKind::Leader, EventKind::ShipMoved, Relation::After, moved).when(moved_when));
    t.add(Spec::new(FINISHED, AbilityKind::Leader, EventKind::MovementFinished, Relation::After, finished).when(finished_when));
}
