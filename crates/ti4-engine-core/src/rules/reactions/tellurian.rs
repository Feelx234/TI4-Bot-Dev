//! Tellurian, the Titans agent (`titans_leaders::tellurian`): "Before a hit would be assigned: you may exhaust this
//! card to cancel that hit." Registered for every seat on the four windows that precede the assignment of hits
//! (space combat, space cannon, ground, anti-fighter barrage); the effect exhausts the card and grants one
//! cancellation to the holder's round pool (`card_timers.cancel_hits_round`), which the next hit assigned spends.

use crate::choice::reaction::{AbilityKind, Relation};
use crate::generated::Leader;
use crate::ids::Seat;
use crate::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use crate::kernel::resolver::TimingError;
use crate::state::{CoreState, EventKind, Key, LeaderStatus};

/// `leader:*:titansagent:{HITS_TO_ASSIGN, SPACE_CANNON_HITS, GROUND_HITS_TO_ASSIGN, ANTI_FIGHTER_BARRAGE_HITS}:when`.
pub const HITS: Slot = Slot(11);
pub const CANNON_HITS: Slot = Slot(12);
pub const GROUND_HITS: Slot = Slot(13);
pub const BARRAGE_HITS: Slot = Slot(14);

const AGENT: Leader = Leader::Tellurian;

fn ready(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&AGENT) == Some(&LeaderStatus::Readied))
}

fn condition(c: &CondCx<'_>) -> bool {
    let p = &c.event.payload;
    if p.seat(Key::Player) != Some(c.def.owner) || !p.int(Key::Hits).is_none_or(|h| h > 0) || !ready(c.state, c.def.owner) {
        return false;
    }
    true
}

fn cancel(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if !ready(eff.state(), me) {
        return Ok(());
    }
    let round = eff.state().meta().combat_round_seq;
    // `leaders::exhaust`: the status, and the staged announcement a watching Nomad (Temporal Command Suite) is told by.
    let (state, kernel) = eff.state_and_kernel();
    crate::kernel::fx_strategy::exhaust_agent(state, kernel, me, AGENT);
    if let Some(p) = eff.state().players_mut().by_seat_mut(me) {
        let running = match p.card_timers.cancel_hits_round {
            Some((held, had)) if held == round => had,
            _ => 0,
        };
        p.card_timers.cancel_hits_round = Some((round, running + 1));
    }
    Ok(())
}

pub fn register(t: &mut AbilityTable) {
    for (slot, event) in [
        (HITS, EventKind::HitsToAssign),
        (CANNON_HITS, EventKind::SpaceCannonHits),
        (GROUND_HITS, EventKind::GroundHitsToAssign),
        (BARRAGE_HITS, EventKind::AntiFighterBarrageHits),
    ] {
        t.add(Spec::new(slot, AbilityKind::Leader, event, Relation::When, cancel).optional().when(condition));
    }
}
