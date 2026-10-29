//! The relic gain shared by the cards area and the agenda effects (packages B5, B9c-2; string `relics::gain`).

use crate::generated::Relic;
use crate::ids::Seat;
use crate::kernel::fx;
use crate::keys::LedgerReason;
use crate::state::{CoreState, RelicRec};

/// `relics::SHARD`.
pub const SHARD: Relic = Relic::ShardOfTheThrone;

/// `relics::log_gain`: the public record of a relic gained (`points`: the victory points it was worth at once).
pub fn log_relic(core: &mut CoreState, who: Seat, relic: Relic, points: i32) {
    let round = core.meta().round;
    core.ledger_mut().relics.push(RelicRec { round, player: who, relic, victory_points: points });
}

/// `relics::gain` (73.2): the top relic to the seat; the Shard scores at once (capped at the target).
/// `None` for an empty deck (73.2a) or a seat that is not there.
pub fn relic_gain(core: &mut CoreState, who: Seat) -> Option<Relic> {
    core.players().by_seat(who)?;
    let top = core.decks_mut().relic.draw()?;
    core.players_mut().by_seat_mut(who)?.cards.relics.push(top);
    let mut points = 0;
    if top == SHARD {
        fx::adjust_victory_points(core, who, 1, LedgerReason::Relic(top));
        points = 1;
    }
    log_relic(core, who, top, points);
    Some(top)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Faction;
    use crate::state::{Player, Players, Seats};

    fn game() -> CoreState {
        let mut s = CoreState::all_loaded();
        let list = [Seat(0), Seat(1)];
        s.seats = Seats::new(&list, &list, 2);
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::ALL[0]), Player::new(Seat(1), Faction::ALL[1])]);
        s
    }

    #[test]
    fn the_top_relic_goes_to_the_seat_and_the_shard_scores_at_once() {
        let mut s = game();
        let other = Relic::ALL.iter().copied().find(|r| *r != SHARD).unwrap();
        s.decks_mut().relic.set(vec![other, SHARD]);
        assert_eq!(relic_gain(&mut s, Seat(1)), Some(other));
        assert_eq!(s.players().by_seat(Seat(1)).unwrap().cards.relics.as_slice(), [other]);
        assert_eq!(s.ledger().relics.last().unwrap().victory_points, 0);
        assert_eq!(relic_gain(&mut s, Seat(1)), Some(SHARD));
        assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.victory_points, 1);
        assert_eq!(s.ledger().relics.last().unwrap().victory_points, 1);
        assert_eq!(s.ledger().vp.len(), 1);
        // An empty deck yields nothing (73.2a); a seat that is not there neither.
        assert_eq!(relic_gain(&mut s, Seat(1)), None);
        s.decks_mut().relic.set(vec![other]);
        assert_eq!(relic_gain(&mut s, Seat(7)), None);
        assert_eq!(s.decks().relic.cards().len(), 1, "the card stays on the deck");
    }
}
