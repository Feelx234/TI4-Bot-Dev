//! The owner table (docs/ENGINE_CORE_STATE.md 1.3). `Seat` is an index into the game's owner table,
//! built by the adapter at creation/load from the players' ids plus every other owner that can
//! appear on the board (neutral units, the Styx holder, ...), sorted in text order, at most 16, so
//! `Seat` order is the `BTreeMap<PlayerId, _>` order. The core never sees the texts.

use crate::containers::SeatSet;
use crate::ids::Seat;

/// Marks "no player" in [`Seats::player_of`].
const NO_PLAYER: u8 = u8::MAX;

/// Most players one game has.
pub const MAX_PLAYERS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seats {
    n_players: u8,
    n_owners: u8,
    /// Player index (the position in `GameState.players`) -> owner seat.
    seat_of_player: [Seat; MAX_PLAYERS],
    /// Owner seat -> player index, or `NO_PLAYER` for an owner that is not a player.
    player_of_seat: [u8; 16],
    /// `GameState.seating_order`, as seats.
    seating_order: [Seat; MAX_PLAYERS],
}

impl Default for Seats {
    fn default() -> Self {
        Self {
            n_players: 0,
            n_owners: 0,
            seat_of_player: [Seat(0); MAX_PLAYERS],
            player_of_seat: [NO_PLAYER; 16],
            seating_order: [Seat(0); MAX_PLAYERS],
        }
    }
}

impl Seats {
    /// Build from the player seats in player-index order, the seating order, and the owner count.
    #[must_use]
    pub fn new(players: &[Seat], seating_order: &[Seat], n_owners: u8) -> Self {
        debug_assert!(players.len() <= MAX_PLAYERS && seating_order.len() == players.len());
        debug_assert!(n_owners <= 16);
        let mut s = Self { n_players: players.len() as u8, n_owners, ..Self::default() };
        for (i, seat) in players.iter().enumerate() {
            s.seat_of_player[i] = *seat;
            s.player_of_seat[usize::from(seat.0)] = i as u8;
        }
        for (i, seat) in seating_order.iter().enumerate() {
            s.seating_order[i] = *seat;
        }
        s
    }

    #[must_use]
    pub const fn n_players(&self) -> usize {
        self.n_players as usize
    }

    #[must_use]
    pub const fn n_owners(&self) -> usize {
        self.n_owners as usize
    }

    /// The seat of player index `i`.
    #[must_use]
    pub fn seat_of(&self, player_index: usize) -> Seat {
        debug_assert!(player_index < self.n_players());
        self.seat_of_player[player_index]
    }

    /// The player index of a seat, if the owner is a player.
    #[must_use]
    pub fn player_of(&self, seat: Seat) -> Option<usize> {
        match self.player_of_seat[usize::from(seat.0)] {
            NO_PLAYER => None,
            i => Some(usize::from(i)),
        }
    }

    /// The seating order (speaker-relative order is derived from it).
    #[must_use]
    pub fn seating_order(&self) -> &[Seat] {
        &self.seating_order[..self.n_players()]
    }

    /// Player seats in player-index order.
    #[must_use]
    pub fn players(&self) -> &[Seat] {
        &self.seat_of_player[..self.n_players()]
    }

    /// The set of player seats (not the other owners).
    #[must_use]
    pub fn player_set(&self) -> SeatSet {
        self.players().iter().copied().collect()
    }

    /// Every owner seat, players or not.
    pub fn owners(&self) -> impl Iterator<Item = Seat> {
        (0..self.n_owners).map(Seat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_both_ways() {
        let s = Seats::new(&[Seat(3), Seat(0), Seat(2)], &[Seat(2), Seat(3), Seat(0)], 5);
        assert_eq!(s.n_players(), 3);
        assert_eq!(s.seat_of(0), Seat(3));
        assert_eq!(s.player_of(Seat(0)), Some(1));
        assert_eq!(s.player_of(Seat(4)), None);
        assert_eq!(s.seating_order(), &[Seat(2), Seat(3), Seat(0)]);
        assert_eq!(s.owners().count(), 5);
        assert_eq!(s.player_set().len(), 3);
    }
}
