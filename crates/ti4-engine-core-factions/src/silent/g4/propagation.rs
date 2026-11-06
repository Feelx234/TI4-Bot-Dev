//! nekro: Propagation (`factions::nekro::propagation`): "When you would research a technology: gain 3 command tokens instead." The research window notes each replaced research
//! (`fx_strategy::note_propagation`); the turn flow announces one `PROPAGATION_RESEARCH` per pending research, and each announcement settles one: the owner places three tokens,
//! all three asked at once as the gain-pool question of the strategy area. Mandatory; registered for every seat as the string does (`ability:*:propagation:PROPAGATION_RESEARCH:after`), the condition names the Nekro.

use ti4_engine_core::choice::strategy::{gain_answer, gain_pool_choice, gain_pools};
use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::Faction;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{CoreState, EventKind, Key};

use crate::common::{mark, num};

/// Propagation (`ability:*:propagation:PROPAGATION_RESEARCH:after`).
pub const PROPAGATION: Slot = Slot(44_000);

/// The tokens one replaced research is worth.
const TOKENS: u8 = 3;

/// The researches of `who` already replaced and not yet announced.
fn pending(s: &CoreState, who: Seat) -> u32 {
    mark(s, MarkKey::NekroPropagation(who)).and_then(num).unwrap_or(0)
}

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    c.event.payload.seat(Key::Player) == Some(me) && c.state.players().by_seat(me).is_some_and(|p| p.faction == Faction::TheNekroVirus) && pending(c.state, me) > 0
}

fn propagate(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if eff.event.payload.seat(Key::Player) != Some(me) {
        return Ok(());
    }
    let owed = pending(eff.read().0, me);
    let key = MarkKey::NekroPropagation(me);
    let tail = eff.state().marks_mut().tail.edit();
    if owed <= 1 {
        tail.remove(&key);
    } else {
        tail.insert(key, MarkVal::Atom(Atom::Num(owed - 1)));
    }
    if let Some(split) = gain_answer(eff.ask(&gain_pool_choice(me, TOKENS)), TOKENS) {
        for pool in gain_pools(split) {
            fx::gain_token(eff.state(), me, pool, 1);
        }
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(PROPAGATION, AbilityKind::Ability, EventKind::PropagationResearch, Relation::After, propagate).when(condition));
}
