use super::cow::Cow;

/// A deck as a shared order plus a head index. A draw is `head += 1` (no copy); edits that change the
/// order (put on the bottom, insert, reshuffle) copy the vec once if it is shared.
#[derive(Debug, Clone)]
pub struct Deck<T: Copy> {
    order: Cow<Vec<T>>,
    head: u32,
}

impl<T: Copy> Default for Deck<T> {
    fn default() -> Self {
        Self { order: Cow::default(), head: 0 }
    }
}

impl<T: Copy> Deck<T> {
    #[must_use]
    pub fn new(cards: Vec<T>) -> Self {
        Self { order: Cow::new(cards), head: 0 }
    }

    /// The undrawn cards, top first.
    #[must_use]
    pub fn cards(&self) -> &[T] {
        &self.order[self.head as usize..]
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.order.len() - self.head as usize
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn top(&self) -> Option<T> {
        self.cards().first().copied()
    }

    /// Draw the top card.
    pub fn draw(&mut self) -> Option<T> {
        let card = self.top()?;
        self.head += 1;
        Some(card)
    }

    /// Put a card on the bottom.
    pub fn put_bottom(&mut self, card: T) {
        self.order.edit().push(card);
    }

    /// Put a card on top.
    pub fn put_top(&mut self, card: T) {
        if self.head > 0 {
            self.head -= 1;
            self.order.edit()[self.head as usize] = card;
        } else {
            self.order.edit().insert(0, card);
        }
    }

    /// Remove the card at `position` (0 = top) of the undrawn cards.
    pub fn remove(&mut self, position: usize) -> T {
        let at = self.head as usize + position;
        self.order.edit().remove(at)
    }

    /// Replace the undrawn cards (reshuffle, setup).
    pub fn set(&mut self, cards: Vec<T>) {
        self.order = Cow::new(cards);
        self.head = 0;
    }

    /// Whether both decks share their order allocation (a checkpoint nobody reshuffled).
    #[must_use]
    pub fn shares_order_with(&self, other: &Self) -> bool {
        Cow::ptr_eq(&self.order, &other.order)
    }
}

impl<T: Copy + PartialEq> PartialEq for Deck<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cards() == other.cards()
    }
}
impl<T: Copy + Eq> Eq for Deck<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draw_does_not_copy() {
        let mut d = Deck::new(vec![1u8, 2, 3, 4]);
        let c = d.clone();
        assert_eq!(d.draw(), Some(1));
        assert!(d.shares_order_with(&c));
        assert_eq!(d.cards(), &[2, 3, 4]);
        assert_eq!(c.cards(), &[1, 2, 3, 4]);
        d.put_bottom(9);
        assert!(!d.shares_order_with(&c));
        assert_eq!(d.cards(), &[2, 3, 4, 9]);
        d.put_top(7);
        assert_eq!(d.cards(), &[7, 2, 3, 4, 9]);
        assert_eq!(d.remove(1), 2);
        assert_eq!(d.cards(), &[7, 3, 4, 9]);
        assert_eq!(d.len(), 4);
    }
}
