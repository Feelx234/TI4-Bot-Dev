//! naalu: the timing abilities of the activation window (lane M6c). Neuroglaive (`technology:*:ng:SYSTEM_ACTIVATED:after`): "after another
//! player activates a system that contains your ships, that player removes 1 token from their fleet pool and returns it to their
//! reinforcements." Mandatory, no question. (Not ported: Foresight, Gift of Prescience, Z'eu, the Oracle, Mindsieve.)

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::Technology;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::{board, reach};
use ti4_engine_core::state::{CoreState, EventKind, Key, Payload};

/// Neuroglaive.
pub const NEUROGLAIVE: Slot = Slot(23001);

/// `neuroglaive_ready`: `owner` has the technology and ships in the activated system, the activator is another seat with a token in its fleet pool.
fn ready(s: &CoreState, owner: Seat, p: &Payload) -> Option<Seat> {
    let (activator, system) = (p.seat(Key::Player)?, p.system(Key::System)?);
    (reach::technology_text(s, owner, Technology::Neuroglaive)
        && activator != owner
        && s.players().by_seat(activator).is_some_and(|other| other.econ.fleet_tokens > 0)
        && board::space_units(s, system).iter().any(|u| u.owner() == owner && u.kind().data().ship))
    .then_some(activator)
}

fn when(c: &CondCx<'_>) -> bool {
    ready(c.state, c.def.owner, &c.event.payload).is_some()
}

fn neuroglaive(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let owner = eff.owner();
    let p = eff.event.payload.clone();
    if let Some(activator) = ready(eff.state(), owner, &p)
        && let Some(player) = eff.state().players_mut().by_seat_mut(activator)
    {
        // The token returns to the reinforcements: those are what is on neither the sheet nor the board.
        player.econ.fleet_tokens -= 1;
    }
    Ok(())
}

/// The ability of the activation window (`activation::register`).
pub fn register_activation(t: &mut AbilityTable) {
    t.add(Spec::new(NEUROGLAIVE, AbilityKind::Technology, EventKind::SystemActivated, Relation::After, neuroglaive).when(when));
}
