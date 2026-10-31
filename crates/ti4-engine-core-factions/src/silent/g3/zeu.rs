//! naalu: Z'eu, the Thunder's Edge agent (`factions::naalu::agent`): "After a player places a command token in a system: you may exhaust this card to remove that token from the board" (the placements the
//! engine announces: the activation token, a typed `COMMAND_TOKEN_PLACED`). Optional; the seat must hold the agent readied, and the system must still hold a token of the player the
//! event names. Registered for every seat (`leader:*:naaluagent-te:COMMAND_TOKEN_PLACED:after`).

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::{Leader, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::{fx, fx_strategy};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{CoreState, EventKind, Key, LeaderStatus, Payload};

/// Z'eu (`leader:*:naaluagent-te:COMMAND_TOKEN_PLACED:after`).
pub const ZEU: Slot = Slot(42_009);

/// `agent_ready`: the card is readied and the system still holds a token of the event's player.
fn ready(s: &CoreState, owner: Seat, p: &Payload) -> Option<(Seat, System)> {
    if s.players().by_seat(owner)?.leaders.status.get(&Leader::ZeuTe) != Some(&LeaderStatus::Readied) {
        return None;
    }
    let (player, system) = (p.seat(Key::Player)?, p.system(Key::System)?);
    s.board().sys(system).cmd.contains(player).then_some((player, system))
}

fn condition(c: &CondCx<'_>) -> bool {
    ready(c.state, c.def.owner, &c.event.payload).is_some()
}

fn remove_token(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some((player, system)) = ready(eff.read().0, me, &eff.event.payload) else { return Ok(()) };
    let (s, k) = eff.state_and_kernel();
    if fx_strategy::exhaust_agent(s, k, me, Leader::ZeuTe) {
        fx::remove_command_token(s, player, system);
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(ZEU, AbilityKind::Leader, EventKind::CommandTokenPlaced, Relation::After, remove_token).optional().when(condition));
}
