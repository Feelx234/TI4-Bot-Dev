//! `fx_map`: the map edits a faction effect records (`MarkKey::MapEdit`: the Ghosts hero swaps two systems, the Muaat hero replaces one) and the
//! replay of them onto the core's own map (the string `movement::replay_map_edits` / `apply_map_edit`; lane M6d). A native game has no string
//! galaxy to catch up, so the core map is edited where the edit is recorded and, for a position that carries recorded edits its galaxy does not,
//! by [`replay_map_edits`].

use crate::generated::System;
use crate::ids::system_at;
use crate::keys::{Atom, MarkKey, MarkVal, Word};
use crate::state::CoreState;

/// One recorded edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapEdit {
    /// The two systems exchange hexes.
    Swap(System, System),
    /// The tile on the hex of `old` becomes `new`.
    Replace(System, System),
}

impl MapEdit {
    /// The mark value of the edit.
    #[must_use]
    pub fn value(self) -> MarkVal {
        let (word, a, b) = match self {
            Self::Swap(a, b) => (Word::Swap, a, b),
            Self::Replace(a, b) => (Word::Replace, a, b),
        };
        MarkVal::Rec(vec![MarkVal::Atom(Atom::Word(word)), MarkVal::Atom(Atom::System(a)), MarkVal::Atom(Atom::System(b))])
    }

    fn of(value: &MarkVal) -> Option<Self> {
        let MarkVal::Rec(fields) = value else { return None };
        match fields.as_slice() {
            [MarkVal::Atom(Atom::Word(Word::Swap)), MarkVal::Atom(Atom::System(a)), MarkVal::Atom(Atom::System(b))] => Some(Self::Swap(*a, *b)),
            [MarkVal::Atom(Atom::Word(Word::Replace)), MarkVal::Atom(Atom::System(a)), MarkVal::Atom(Atom::System(b))] => Some(Self::Replace(*a, *b)),
            _ => None,
        }
    }
}

/// The recorded edits, oldest first.
#[must_use]
pub fn recorded(core: &CoreState) -> Vec<MapEdit> {
    core.marks().tail.iter().filter_map(|(key, value)| if matches!(key, MarkKey::MapEdit(_)) { MapEdit::of(value) } else { None }).collect()
}

/// Record `edit` as the next `MapEdit` mark (`apply_map_edit`'s `map_edit|NNNN` row).
pub fn record(core: &mut CoreState, edit: MapEdit) {
    let seq = core.marks().tail.iter().filter(|(key, _)| matches!(key, MarkKey::MapEdit(_))).count();
    core.marks_mut().tail.edit().insert(MarkKey::MapEdit(u32::try_from(seq).unwrap_or(u32::MAX)), edit.value());
}

/// Apply `edit` to the core's map alone. `false`, nothing changed: no map, or an edit that does not fit it.
pub fn apply_to_map(core: &mut CoreState, edit: MapEdit) -> bool {
    let Some(galaxy) = core.galaxy() else { return false };
    let mut next = (**galaxy).clone();
    let done = match edit {
        MapEdit::Swap(a, b) => next.swap_systems(system_ix(a), system_ix(b)).is_ok(),
        MapEdit::Replace(old, new) => next.replace_system(system_ix(old), system_ix(new)).is_ok(),
    };
    if done {
        core.map_mut().set_galaxy(next);
    }
    done
}

fn system_ix(system: System) -> crate::ids::SystemIx {
    system_at(system as usize)
}

/// `movement::replay_map_edits`: bring the core's map up to the edits the position records. Idempotent: the map counts the edits it carries.
/// `false` when an edit no longer fits the map (the edits before it stay applied).
pub fn replay_map_edits(core: &mut CoreState) -> bool {
    let Some(carried) = core.galaxy().map(|g| g.edits_applied()) else { return recorded(core).is_empty() };
    for edit in recorded(core).into_iter().skip(carried) {
        if !apply_to_map(core, edit) {
            return false;
        }
    }
    true
}
