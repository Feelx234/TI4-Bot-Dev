//! Forced random positions for replays of a redone timeline (H7 turn redo).
//!
//! A game's dice and decks come from seeded streams that only advance when something draws.
//! After a turn is redone, the decisions that follow it are other seats' recorded answers, made
//! against the *original* dice. If the new turn drew a different number of dice, a plain replay
//! from the seed would hand those later decisions different faces. A history therefore carries
//! `rng_marks`: for some decision indexes, where every stream stood when the decision was
//! answered. Every replay path restores those positions before the decision's consequences draw.
//! The game seed is never touched; only stream positions move.

use std::collections::BTreeMap;
use std::sync::Arc;

use ti4_engine::game::Game;
use ti4_engine::rng::{RngPositions, RngSync};

/// Stream positions by decision index (the index of the decision being answered).
pub type RngMarks = BTreeMap<usize, RngPositions>;

/// Applies [`RngMarks`] during a replay. Cheap to clone; clones share the same side channel.
#[derive(Clone)]
pub struct RngForce {
    sync: Arc<RngSync>,
    marks: Arc<RngMarks>,
}

impl RngForce {
    /// `None` for an empty mark set, so histories without marks replay exactly as before.
    #[must_use]
    pub fn new(marks: &RngMarks) -> Option<Self> {
        (!marks.is_empty()).then(|| Self {
            sync: RngSync::new(),
            marks: Arc::new(marks.clone()),
        })
    }

    /// A force that always exists, so a run can capture positions even with nothing to force.
    #[must_use]
    pub fn always(marks: &RngMarks) -> Self {
        Self {
            sync: RngSync::new(),
            marks: Arc::new(marks.clone()),
        }
    }

    /// Connect the game's random source to this force.
    pub fn attach(&self, game: &mut Game<'_>) {
        game.set_rng_sync(Some(Arc::clone(&self.sync)));
    }

    /// Call when decision `index` is about to be answered from the record.
    pub fn before_answer(&self, index: usize) {
        if let Some(positions) = self.marks.get(&index) {
            self.sync.restore_at_next_draw(positions.clone());
        }
    }
}

impl RngForce {
    /// Record where the streams stand when decision `index` is answered (read at the next draw,
    /// after any restore for the same decision).
    pub fn capture(&self, index: usize) {
        self.sync.mark_at_next_draw(index);
    }

    /// Resolve captures that no draw has read yet against the game's final positions.
    pub fn finish(&self, game: &Game<'_>) {
        self.sync.resolve_pending(&game.rng_positions());
    }

    /// Everything captured so far, by decision index.
    #[must_use]
    pub fn captured(&self) -> RngMarks {
        self.sync.marks()
    }
}
