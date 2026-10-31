//! nekro: Acidos, the commander (`factions::borrowed_commanders::nekro_commander`): "After you gain a technology: you may draw 1 action card." Optional; the seat has the commander's ability
//! (its own unlocked commander, a faceup Alliance, a Yin grant) and the deck is not empty. Registered for every seat (`leader:*:nekrocommander:TECHNOLOGY_GAINED:after`).
//! The draw is the plain one (`g3::draw::plain`): a draw that would ask (Scheming, a full hand) is a loud gap.

use ti4_engine_core::choice::reaction::{AbilityKind, Relation};
use ti4_engine_core::generated::Leader;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::state::{EventKind, Key};

use crate::cc_ground::borrowed::query::has_commander_ability;

/// Acidos (`leader:*:nekrocommander:TECHNOLOGY_GAINED:after`).
pub const ACIDOS: Slot = Slot(42_010);

fn condition(c: &CondCx<'_>) -> bool {
    c.event.payload.seat(Key::Player) == Some(c.def.owner) && !c.state.decks().action.is_empty() && has_commander_ability(c.state, c.kernel, c.def.owner, Leader::NekroAcidos)
}

fn draw(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let _ = super::draw::plain(eff, me, 1, ACIDOS);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(ACIDOS, AbilityKind::Leader, EventKind::TechnologyGained, Relation::After, draw).optional().when(condition));
}
