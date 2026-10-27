//! obsidian: tier Q: pure hooks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CE). Owned by CE only.
//!
//! | string slot | string function | tier | target |
//! |---|---|---|---|
//! | `strategy.secondary_waivers` | `firmament_plots::secondary_waivers` | Q | `strategy.secondary_waivers` (Enervate, code 0) |
//! | `strategy.secondary_waived` | `firmament_plots::secondary_waived` | Q | `strategy.secondary_waived` (costs nothing: a no-op) |
//! | `substitutes_primary` | `firmament_plots::substitutes_primary` | Q | `general.substitutes_primary` |
//! | `direct.obsidian_listens_for_tactical_end` | `obsidian::listens_for_tactical_end` | Q | `direct` |
//! | `timing_abilities`, `commander_unlocked`, `leader_action`, `use_leader` | | W | B7 (leaders), Malevolency and the plots' windows |

use ti4_engine_core::generated::{Faction, PromissoryNote, StrategyCardKind};
use ti4_engine_core::hooks::strategy::Waivers;
use ti4_engine_core::hooks::{HookTable, Waiver};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::{Cx, Kernel};
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::state::{CoreState, NoteKey, PlotCard};

use crate::common::{is_faction, mark, player, seat};

/// The code of the Enervate waiver (the module is the Obsidian; the adapter renders `enervate`).
pub const ENERVATE: u16 = 0;
const MALEVOLENCY: NoteKey = NoteKey { note: PromissoryNote::Malevolency, faction: Faction::TheObsidian };

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.strategy.secondary_waivers.push(secondary_waivers);
    t.strategy.secondary_waived.push(secondary_waived);
    t.general.substitutes_primary.push(substitutes_primary);
    t.direct.obsidian_listens_for_tactical_end.push(listens_for_tactical_end);
}

/// Whether the players with a token on `owner`'s faceup Enervate include `token` (`firmament_plots::puppets`: only an
/// Obsidian owner has any).
fn puppeted_by_enervate(s: &CoreState, owner: Seat, token: Seat) -> bool {
    is_faction(s, owner, Faction::TheObsidian)
        && player(s, owner).is_some_and(|p| p.cards.plots.as_slice().iter().any(|plot| plot.faceup && plot.card == Some(PlotCard::Enervate) && plot.tokens.contains(token)))
}

/// Enervate: the Obsidian `follower` may follow `primary`'s strategic action without a command token (not Leadership).
pub fn secondary_waivers(s: &CoreState, _: &Kernel, follower: Seat, primary: Seat, card: Option<StrategyCardKind>) -> Waivers {
    let mut out = Waivers::new();
    if follower != primary && card != Some(StrategyCardKind::Leadership) && puppeted_by_enervate(s, follower, primary) {
        out.push(Waiver { module: Faction::TheObsidian, code: ENERVATE });
    }
    out
}

/// The waiver costs nothing.
pub fn secondary_waived(_: &mut Cx<'_>, _: Seat, _: Seat, _: Waiver) {}

/// Leadership's secondary is resolved as its primary for a follower who chose that.
pub fn substitutes_primary(s: &CoreState, _: &Kernel, who: Seat, card: Option<StrategyCardKind>) -> bool {
    card == Some(StrategyCardKind::Leadership) && mark(s, MarkKey::FirmamentEnervate()).and_then(seat) == Some(who)
}

/// Whether Malevolency is in the game, so the game must announce the end of tactical actions for it.
pub fn listens_for_tactical_end(s: &CoreState) -> bool {
    s.cards().promissory.get(&MALEVOLENCY).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ce_movement::testkit::table;
    use ti4_engine_core::containers::SeatSet;
    use ti4_engine_core::keys::{Atom, MarkVal};
    use ti4_engine_core::state::Plot;

    const OBS: Faction = Faction::TheObsidian;
    const SOL: Faction = Faction::TheFederationOfSol;

    fn enervate(s: &mut CoreState, owner: u8, token: Seat, faceup: bool) {
        let mut tokens = SeatSet::EMPTY;
        tokens.insert(token);
        s.players_mut().edit(owner as usize).cards.plots.push(Plot { card: Some(PlotCard::Enervate), tokens, faceup });
    }

    #[test]
    fn enervate_waives_the_secondary_of_a_puppeted_player_except_leadership() {
        let (mut s, k) = table(&[OBS, SOL, SOL]);
        let other = Some(StrategyCardKind::Politics);
        assert!(secondary_waivers(&s, &k, Seat(0), Seat(1), other).is_empty(), "no plot");
        enervate(&mut s, 0, Seat(1), true);
        assert_eq!(secondary_waivers(&s, &k, Seat(0), Seat(1), other).as_slice(), &[Waiver { module: OBS, code: ENERVATE }]);
        assert!(secondary_waivers(&s, &k, Seat(0), Seat(1), Some(StrategyCardKind::Leadership)).is_empty(), "Leadership has the primary instead");
        assert!(secondary_waivers(&s, &k, Seat(0), Seat(2), other).is_empty(), "no token on the other player");
        assert!(secondary_waivers(&s, &k, Seat(0), Seat(0), other).is_empty(), "not the own card");
        assert!(secondary_waivers(&s, &k, Seat(1), Seat(1), other).is_empty());
    }

    #[test]
    fn a_facedown_enervate_waives_nothing() {
        let (mut s, k) = table(&[OBS, SOL]);
        enervate(&mut s, 0, Seat(1), false);
        assert!(secondary_waivers(&s, &k, Seat(0), Seat(1), None).is_empty());
    }

    #[test]
    fn the_primary_is_substituted_for_the_follower_who_chose_it() {
        let (mut s, k) = table(&[OBS, SOL]);
        let lead = Some(StrategyCardKind::Leadership);
        assert!(!substitutes_primary(&s, &k, Seat(0), lead));
        s.marks_mut().tail.edit().insert(MarkKey::FirmamentEnervate(), MarkVal::Atom(Atom::Seat(Seat(0))));
        assert!(substitutes_primary(&s, &k, Seat(0), lead));
        assert!(!substitutes_primary(&s, &k, Seat(1), lead));
        assert!(!substitutes_primary(&s, &k, Seat(0), Some(StrategyCardKind::Politics)));
    }

    #[test]
    fn malevolency_in_play_makes_the_game_listen() {
        let (mut s, _) = table(&[OBS, SOL]);
        assert!(!listens_for_tactical_end(&s));
        s.cards_mut().promissory.edit().insert(MALEVOLENCY, Seat(1));
        assert!(listens_for_tactical_end(&s));
    }

    #[test]
    fn registering_fills_four_slots() {
        let mut t = HookTable::new();
        register(&mut t);
        assert_eq!(t.registered(), 4);
    }
}
