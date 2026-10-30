//! nekro: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: Antivirus, the placement half,
//! "At the start of a combat: place this card faceup in your play area" (a holder of the note of another faction; the
//! Nekro's own copy is not theirs to play). Technological Singularity is `singularity.rs`. Galactic Threat is `threat.rs` (package W9). Not ported: Propagation, the assimilation half.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::PromissoryNote;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{CoreState, EventKind, NoteKey};

/// Antivirus, space (`promissory:*:antivirus:SPACE_COMBAT_STARTED:after`).
pub const ANTIVIRUS_SPACE: Slot = Slot(2001);
/// Antivirus, ground (`promissory:*:antivirus:GROUND_COMBAT_STARTED:after`).
pub const ANTIVIRUS_GROUND: Slot = Slot(2002);

/// The Antivirus `holder` keeps in hand: another faction's, not yet placed (the first in note order).
fn in_hand(s: &CoreState, holder: Seat) -> Option<NoteKey> {
    let own = s.players().by_seat(holder)?.faction;
    s.cards()
        .promissory
        .iter()
        .find(|(k, h)| **h == holder && k.note == PromissoryNote::Antivirus && !s.cards().promissory_faceup.contains_key(*k) && k.faction != own)
        .map(|(k, _)| *k)
}

fn condition(c: &CondCx<'_>) -> bool {
    in_hand(c.state, c.def.owner).is_some()
}

fn place(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if let Some(note) = in_hand(eff.read().0, me) {
        eff.state().cards_mut().promissory_faceup.edit().insert(note, ());
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    for (slot, event) in [(ANTIVIRUS_SPACE, EventKind::SpaceCombatStarted), (ANTIVIRUS_GROUND, EventKind::GroundCombatStarted)] {
        t.add(Spec::new(slot, AbilityKind::Promissory, event, Relation::After, place).when(condition).optional());
    }
    // The string order of the Nekro's abilities: the Antivirus placements, then the Singularity marks (one bucket:
    // SPACE_COMBAT_STARTED / GROUND_COMBAT_STARTED, after).
    super::threat::register_abilities(t);
    super::singularity::register_abilities(t);
}
