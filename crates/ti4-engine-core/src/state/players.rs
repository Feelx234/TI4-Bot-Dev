//! `Players`: up to 8 `Cow<Player>`. A clone is one refcount bump per seat; a write copies one player.

use super::player::Player;
use super::seats::MAX_PLAYERS;
use crate::containers::Cow;
use crate::ids::Seat;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Players {
    n: u8,
    list: [Cow<Player>; MAX_PLAYERS],
}

impl Default for Players {
    fn default() -> Self {
        let shared = Cow::new(Player::default());
        Self { n: 0, list: std::array::from_fn(|_| shared.clone()) }
    }
}

impl Players {
    /// Players in `GameState.players` order.
    #[must_use]
    pub fn from_vec(players: Vec<Player>) -> Self {
        debug_assert!(players.len() <= MAX_PLAYERS);
        let mut p = Self::default();
        p.n = players.len() as u8;
        for (i, player) in players.into_iter().enumerate() {
            p.list[i] = Cow::new(player);
        }
        p
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.n as usize
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// The player at index `i` (the position in `GameState.players`).
    #[must_use]
    pub fn get(&self, i: usize) -> &Player {
        debug_assert!(i < self.len());
        &self.list[i]
    }

    /// Mutable access; copies the player first if a checkpoint shares it.
    pub fn edit(&mut self, i: usize) -> &mut Player {
        debug_assert!(i < self.len());
        self.list[i].edit()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Player> {
        self.list[..self.len()].iter().map(|c| &**c)
    }

    /// The player whose seat is `seat` (a linear scan of at most 8).
    #[must_use]
    pub fn by_seat(&self, seat: Seat) -> Option<&Player> {
        self.iter().find(|p| p.seat == seat)
    }

    pub fn by_seat_mut(&mut self, seat: Seat) -> Option<&mut Player> {
        let i = self.iter().position(|p| p.seat == seat)?;
        Some(self.edit(i))
    }

    /// How many players are shared with `other` (checkpoint tests).
    #[must_use]
    pub fn shared_with(&self, other: &Self) -> usize {
        (0..self.len().min(other.len())).filter(|i| Cow::ptr_eq(&self.list[*i], &other.list[*i])).count()
    }
}
