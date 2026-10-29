//! The bookkeeping of a scoring window (package A4b-2; string `objectives::ScoringWindow`): who is asked next,
//! what each player may still score in it (the per-window public and secret caps), and Plots Within Plots'
//! second step. [`ScoringBook`] is plain data: it reads the position only through [`Offers`], the lists of
//! scoreable objectives (`objectives::scoreable_on`, `secrets::scoreable_on` / `scoreable_event`,
//! `firmament::plot_options`), which the string engine computes and the read half of A4 compares.
//!
//! The window asks one player at a time in initiative order and keeps a player in place while the other
//! category might still have something on offer (61.6: at most one public and one secret objective in the
//! status window; one secret per player and event, or any number, in an event window). A decline ends the
//! player's whole turn. [`ScoringBook::asked`] is `next_askable`, [`ScoringBook::after`] the update
//! `ScoringWindow::resolve` makes to `pending` and the two sets after an answer.

use crate::choice::objectives::Offer;
use crate::containers::{SeatSet, SmallList};
use crate::generated::SecretObjective;
use crate::ids::Seat;
use crate::state::ObjectiveRef;

/// The lists the position gives for one player. The book asks only for the lists its caps leave open.
pub trait Offers {
    /// The revealed public objectives the seat could score now (status window only).
    fn publics(&mut self, seat: Seat) -> SmallList<ObjectiveRef, 24>;
    /// The secrets the seat could score in this window (status secrets, or the event's secrets).
    fn secrets(&mut self, seat: Seat) -> SmallList<SecretObjective, 8>;
    /// Plots Within Plots: the secrets another player scored that this seat may score as a plot, with that player.
    fn plots(&mut self, seat: Seat) -> SmallList<(SecretObjective, Seat), 8>;
    /// Whether the seat already used its one score of this window's event occurrence.
    fn scored_at_occurrence(&mut self, seat: Seat) -> bool;
}

/// The player the window asks next and what they may score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// How many entries of `pending` (from the next one) were skipped to reach the player.
    pub offset: usize,
    pub seat: Seat,
    pub offers: SmallList<Offer, 24>,
}

/// What an answer did, as far as the book is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The player declined: their whole turn is over.
    Declined,
    /// A public or secret objective was scored (`secret`: it left the hand, the secret cap is used).
    Scored { secret: bool },
    /// A plot was scored for `token`; `placed` when its card was placed at once (one card was left).
    PlotScored { token: Seat, placed: bool },
    /// The answer could not be applied (a stale window): the player's turn is over.
    Failed,
}

/// The book of one scoring window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoringBook {
    /// Seats still to be asked, the next one LAST (the string keeps it reversed so that `pop` takes the first).
    pending: SmallList<Seat, 8>,
    public_used: SeatSet,
    secret_used: SeatSet,
    /// Plots Within Plots: the Firmament that still has to place a plot card, and whose token goes on it.
    plot_card: Option<(Seat, Seat)>,
    /// The status window (publics and secrets in one list); an event window offers its secrets only.
    status: bool,
    /// The event window with the one-per-player cap, over an event occurrence.
    capped_by_occurrence: bool,
    /// `EventScoreLimit::OnePerPlayer`.
    one_per_player: bool,
}

impl ScoringBook {
    /// A book over `order` (initiative order, first player first).
    #[must_use]
    pub fn new(order: &[Seat], status: bool, one_per_player: bool, has_occurrence: bool) -> Self {
        Self {
            pending: order.iter().rev().copied().collect(),
            public_used: SeatSet::EMPTY,
            secret_used: SeatSet::EMPTY,
            plot_card: None,
            status,
            capped_by_occurrence: one_per_player && has_occurrence,
            one_per_player,
        }
    }

    /// The book as the string window holds it: `pending` is its vector in its stored (reversed) order.
    #[must_use]
    pub fn restore(
        pending_stored: &[Seat],
        public_used: SeatSet,
        secret_used: SeatSet,
        plot_card: Option<(Seat, Seat)>,
        status: bool,
        one_per_player: bool,
        has_occurrence: bool,
    ) -> Self {
        Self {
            pending: pending_stored.iter().copied().collect(),
            public_used,
            secret_used,
            plot_card,
            status,
            capped_by_occurrence: one_per_player && has_occurrence,
            one_per_player,
        }
    }

    /// `ScoringWindow::is_complete`.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.pending.is_empty() && self.plot_card.is_none()
    }

    /// The seats still pending, in the stored order (next LAST).
    #[must_use]
    pub fn pending(&self) -> &[Seat] {
        self.pending.as_slice()
    }

    #[must_use]
    pub const fn public_used(&self) -> SeatSet {
        self.public_used
    }

    #[must_use]
    pub const fn secret_used(&self) -> SeatSet {
        self.secret_used
    }

    /// The Firmament that has to place a plot card and the player whose token goes on it.
    #[must_use]
    pub const fn plot_card(&self) -> Option<(Seat, Seat)> {
        self.plot_card
    }

    /// `ScoringWindow::next_askable`: the first pending player with something to score. The public list is asked
    /// for only where the status window still allows the player a public score, the secret list unless the
    /// status secret cap or the one-per-event cap is used; plots are not capped.
    pub fn asked(&self, offers: &mut dyn Offers) -> Option<Asked> {
        for (offset, seat) in self.pending.as_slice().iter().rev().copied().enumerate() {
            let mut list: SmallList<Offer, 24> = SmallList::new();
            if self.status && !self.public_used.contains(seat) {
                list.extend(offers.publics(seat).iter().map(|o| Offer::Objective(*o)));
            }
            let secrets = if self.status && self.secret_used.contains(seat) {
                SmallList::new()
            } else {
                offers.secrets(seat)
            };
            let secrets = if self.capped_by_occurrence && offers.scored_at_occurrence(seat) { SmallList::new() } else { secrets };
            list.extend(secrets.iter().map(|s| Offer::Objective(ObjectiveRef::Secret(*s))));
            list.extend(offers.plots(seat).iter().map(|(secret, token)| Offer::Plot { secret: *secret, token: *token }));
            if !list.is_empty() {
                return Some(Asked { offset, seat, offers: list });
            }
        }
        None
    }

    /// Whether the player `asked` named is again the next one asked (and so keeps their place), after the
    /// answer changed the position `offers` reads.
    fn keeps_place(&self, asked: &Asked, offers: &mut dyn Offers) -> bool {
        self.asked(offers).is_some_and(|next| next.offset == asked.offset && next.seat == asked.seat)
    }

    fn truncate_keep(&mut self, asked: &Asked, stay: bool) {
        let keep = self.pending.len() - asked.offset - usize::from(!stay);
        self.pending.truncate(keep);
    }

    /// Plots Within Plots, second step done: the card was placed; the pending list is untouched.
    pub fn card_placed(&mut self) {
        self.plot_card = None;
    }

    /// `ScoringWindow::resolve`'s update of the book after an answer to `asked`; `offers` reads the position
    /// the answer left.
    pub fn after(&mut self, asked: &Asked, outcome: Outcome, offers: &mut dyn Offers) {
        match outcome {
            Outcome::Declined | Outcome::Failed => self.truncate_keep(asked, false),
            Outcome::PlotScored { token, placed } => {
                if !placed {
                    self.plot_card = Some((asked.seat, token));
                }
                // Neither per-window cap is used: the player stays while anything is still on offer to them.
                let stay = self.keeps_place(asked, offers);
                self.truncate_keep(asked, stay);
            }
            Outcome::Scored { secret } => {
                let stay = if self.status {
                    if secret {
                        self.secret_used.insert(asked.seat);
                    } else {
                        self.public_used.insert(asked.seat);
                    }
                    self.keeps_place(asked, offers)
                } else if self.one_per_player {
                    // Plots Within Plots is not subject to the one-per-player cap.
                    !offers.plots(asked.seat).is_empty()
                } else {
                    true
                };
                self.truncate_keep(asked, stay);
            }
        }
    }
}
