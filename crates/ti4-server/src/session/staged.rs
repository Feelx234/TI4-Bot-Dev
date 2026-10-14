//! What a seat has staged and not sent: the staged movement, landings and production, the
//! private draft of a tactical action, and the draft of a secondary. The server keeps it for the
//! seat, so every tab and every device of the seat has the same staged work. The values are the
//! client's; the server stores and forwards them and does not read them. The wire types are in
//! `protocol::staged` (`ti4-host`); this is the state and its persistence.
//!
//! A slot is written by compare-and-set on its revision ([`SeatStaged::put`]). Revisions count
//! up for the whole seat, so the newest message of a seat is the one with the highest
//! `revision`.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ti4_model::id::PlayerId;

use crate::protocol::staged::SeatStagedMsg;
pub use crate::protocol::staged::{StagedSlot, StagedValue};

/// Every slot of one seat.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatStaged {
    /// The revision of the newest write of any slot (0: never written).
    pub revision: u64,
    #[serde(default)]
    pub slots: BTreeMap<StagedSlot, StagedValue>,
}

impl SeatStaged {
    /// The message that carries these slots to the connections of the seat.
    #[must_use]
    pub fn message(self, game_id: &str) -> SeatStagedMsg {
        SeatStagedMsg::new(game_id, self.revision, self.slots)
    }

    /// The revision of `slot`: 0 for a slot that was never written.
    #[must_use]
    pub fn slot_revision(&self, slot: StagedSlot) -> u64 {
        self.slots.get(&slot).map_or(0, |staged| staged.revision)
    }

    /// Write `value` into `slot` when `base_revision` is the revision of the slot. Returns
    /// whether it was written. A put of the value that the slot holds is taken and changes
    /// nothing.
    pub fn put(
        &mut self,
        slot: StagedSlot,
        base_revision: u64,
        value: Option<serde_json::Value>,
    ) -> Put {
        if base_revision != self.slot_revision(slot) {
            return Put::Stale;
        }
        let current = self
            .slots
            .get(&slot)
            .and_then(|staged| staged.value.as_ref());
        if current == value.as_ref() {
            return Put::Unchanged;
        }
        self.revision += 1;
        self.slots.insert(
            slot,
            StagedValue {
                revision: self.revision,
                value,
            },
        );
        Put::Written
    }
}

/// What became of a [`SeatStaged::put`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Put {
    Written,
    /// The slot holds that value already.
    Unchanged,
    /// The base revision is not the slot's: nothing was written.
    Stale,
}

/// The staged work of every seat of one game. A clone shares it: the sessions that replace each
/// other (undo, redo, a batch) have one book, so a write that reaches the old session while the
/// new one starts is not lost.
#[derive(Debug, Clone, Default)]
pub struct StagedBook(Arc<Mutex<BTreeMap<PlayerId, SeatStaged>>>);

impl StagedBook {
    #[must_use]
    pub fn new(seats: BTreeMap<PlayerId, SeatStaged>) -> Self {
        Self(Arc::new(Mutex::new(seats)))
    }

    /// The lock of the book. Taken before the lock of a session, never after it.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<PlayerId, SeatStaged>> {
        self.0.lock().expect("staged lock")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_put_is_a_compare_and_set_on_the_revision_of_its_slot() {
        let mut seat = SeatStaged::default();
        let value = |n: u64| Some(serde_json::json!({ "n": n }));
        assert_eq!(seat.put(StagedSlot::Plan, 0, value(1)), Put::Written);
        assert_eq!(seat.slot_revision(StagedSlot::Plan), 1);
        // Another slot has its own base, and the revisions count for the seat.
        assert_eq!(seat.put(StagedSlot::Movement, 0, value(2)), Put::Written);
        assert_eq!(seat.slot_revision(StagedSlot::Movement), 2);
        assert_eq!(seat.put(StagedSlot::Plan, 0, value(3)), Put::Stale);
        assert_eq!(seat.put(StagedSlot::Plan, 1, value(1)), Put::Unchanged);
        assert_eq!(seat.revision, 2);
        // A clear keeps the revision, so a late put on the old value is still stale.
        assert_eq!(seat.put(StagedSlot::Plan, 1, None), Put::Written);
        assert_eq!(seat.slot_revision(StagedSlot::Plan), 3);
        assert_eq!(seat.put(StagedSlot::Plan, 1, value(4)), Put::Stale);
        let text = serde_json::to_string(&seat).unwrap();
        assert!(text.contains("\"plan\""), "{text}");
        assert_eq!(serde_json::from_str::<SeatStaged>(&text).unwrap(), seat);
    }
}
