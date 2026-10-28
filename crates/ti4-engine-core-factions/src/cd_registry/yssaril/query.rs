//! yssaril: tier Q: pure hooks, stacks, predicates and gathers (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CD).
//!
//! | slot | string function | tier | target |
//! |---|---|---|---|
//! | `cards.transaction_reach` | `yssaril::deepgloom_reach` | Q | `cards.transaction_reach` |
//! | `cards.transaction_limit_exempt` | `yssaril::transaction_limit_exempt` | Q | `cards.transaction_limit_exempt` |
//! | `economy.action_card_draw_bonus` | `yssaril::action_card_draw_bonus` (Scheming) | Q | `economy.action_card_draw_bonus` |
//! | `economy.action_card_limit` | `yssaril::action_card_limit` (Crafty) | Q | `economy.action_card_limit` (a fold) |
//! | `economy.action_cards_forbidden` | `yssaril::action_cards_forbidden` (Transparasteel Plating) | Q | `economy.action_cards_forbidden` |
//! | `general.commander_unlocked` | `yssaril::commander_unlocked` | Q | `general.commander_unlocked` |
//! | `economy.action_card_draw_requested`, `economy.action_cards_drawn` | | W | `offer.rs` (B6's draw asks) |
//! | `movement.may_move_through_ships` | | Q | `movement.rs` (A2, done) |
//! | `component_actions`, `perform_component`, `leader_action`, `use_leader`, `timing_abilities` | | W | B7 |

use ti4_engine_core::generated::{Faction, Leader, Technology};
use ti4_engine_core::hooks::HookTable;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::keys::{MarkKey, Word};
use ti4_engine_core::state::{CoreState, Phase};

use crate::common::{has_tech_text, is_faction, mark, num, player};

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.general.commander_unlocked.push(commander_unlocked);
    t.cards.transaction_reach.push(deepgloom_reach);
    t.cards.transaction_limit_exempt.push(transaction_limit_exempt);
    t.economy.action_card_draw_bonus.push(action_card_draw_bonus);
    t.economy.action_card_limit.push(action_card_limit);
    t.economy.action_cards_forbidden.push(action_cards_forbidden);
}

fn plays_yssaril(s: &CoreState, who: Seat) -> bool {
    is_faction(s, who, Faction::TheYssarilTribes)
}

/// Crafty: "You can have any number of action cards in your hand. Game effects cannot prevent you from using this
/// ability." Modules run after the law cap, so this beats it.
pub fn action_card_limit(s: &CoreState, _: &Kernel, who: Seat, limit: usize) -> usize {
    if plays_yssaril(s, who) { usize::MAX } else { limit }
}

/// Scheming: "Draw 1 additional action card." Also the player a Deepgloom Executable owner allowed to use it.
pub fn action_card_draw_bonus(s: &CoreState, _: &Kernel, who: Seat, _requested: usize) -> usize {
    usize::from(plays_yssaril(s, who) || mark(s, MarkKey::PrivateBorrowedScheming(who)).is_some())
}

/// Transparasteel Plating: during the holder's turn of the action phase, players that have passed cannot play action
/// cards.
pub fn action_cards_forbidden(s: &CoreState, who: Seat) -> bool {
    if s.meta().phase != Phase::Action || !player(s, who).is_some_and(|p| p.econ.passed) {
        return false;
    }
    s.meta().active.is_some_and(|active| active != who && has_tech_text(s, active, Technology::TransparasteelPlating))
}

/// Deepgloom Executable: a transaction between the owner and a player it allowed to use Stall Tactics reaches across the
/// board (either direction).
pub fn deepgloom_reach(s: &CoreState, _: &Kernel, a: Seat, b: Seat) -> bool {
    mark(s, MarkKey::YssarilBt(Word::Negotiating, a, b)).is_some() || mark(s, MarkKey::YssarilBt(Word::Negotiating, b, a)).is_some()
}

fn bt_exempt(s: &CoreState, owner: Seat, user: Seat) -> bool {
    mark(s, MarkKey::YssarilBt(Word::Exempt, owner, user)).and_then(num) == Some(s.meta().turn_seq)
}

/// A transaction between a Yssaril seat and a player it allowed to use Stall Tactics this turn does not count against the
/// once-per-player limit.
pub fn transaction_limit_exempt(s: &CoreState, _: &Kernel, active: Seat, other: Seat) -> bool {
    s.meta().phase == Phase::Action && (bt_exempt(s, active, other) || bt_exempt(s, other, active))
}

/// So Ata, unlock: "Have 7 action cards."
pub fn commander_unlocked(s: &CoreState, _: &Kernel, who: Seat, leader: Leader) -> Option<bool> {
    (leader == Leader::SoAta).then(|| player(s, who).map_or(0, |p| p.cards.hand.len()) >= 7)
}
