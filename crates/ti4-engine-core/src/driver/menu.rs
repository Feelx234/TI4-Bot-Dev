//! The action menu of a turn (package D1b-4): how the options of its sources are put together. The sources are areas
//! (strategic actions, tactical action, transactions, contacts, payments, relics, exploration, technologies, Thunder's
//! Edge, faction and mapped component actions, leaders, action cards) and no area may import another, so the driver puts
//! the menu together: the order of the sources, the failed-action filter, and the closing menu of a turn that has taken an
//! action. An option is whatever the caller names it (`T`: the string side passes its option ids, the typed side its
//! codes); the sources that are not ported are LENT AS READS: the caller supplies their options as read from the string
//! engine and the assembly is still the core's.
//!
//! The string functions this mirrors: `Game::{turn_options, closing_options, is_free_option}` (`game.rs`).

use crate::generated::Technology;
use crate::ids::Seat;
use crate::state::CoreState;

/// The sources of the action menu, in the order `Game::turn_options` appends them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// The strategic action options, or the lone "pass" when the seat holds no unused card. First, so a table that always
    /// takes the first option keeps taking what it took.
    Strategic,
    Tactical,
    Transactions,
    Contacts,
    Payments,
    Relics,
    Exploration,
    Technology,
    /// Thunder's Edge's expedition as a component action (a table with Thunder's Edge on offers it when a turn closes).
    ThundersEdge,
    Faction,
    Mapped,
    Leaders,
    ActionCards,
}

/// The sources in menu order.
pub const ORDER: [Source; 13] = [
    Source::Strategic,
    Source::Tactical,
    Source::Transactions,
    Source::Contacts,
    Source::Payments,
    Source::Relics,
    Source::Exploration,
    Source::Technology,
    Source::ThundersEdge,
    Source::Faction,
    Source::Mapped,
    Source::Leaders,
    Source::ActionCards,
];

/// `Game::turn_options`: the options of every source in menu order (`parts` may come in any order, a source may be absent),
/// then without the component actions that already failed this turn (nothing about the position changed, so they would
/// fail identically; "pass" and every other action stay).
#[must_use]
pub fn turn_menu<T: Clone + PartialEq>(parts: &[(Source, Vec<T>)], thunders_edge_on: bool, failed: &[T]) -> Vec<T> {
    let mut out = Vec::new();
    for source in ORDER {
        // Thunder's Edge's expedition is a component action only on older tables: a table with the expansion on commits at
        // the end of the turn (see [`closing_menu`]).
        if source == Source::ThundersEdge && thunders_edge_on {
            continue;
        }
        for (s, options) in parts {
            if *s == source {
                out.extend(options.iter().cloned());
            }
        }
    }
    out.retain(|option| !failed.contains(option));
    out
}

/// One option of the turn menu as the closing menu sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry<T> {
    pub option: T,
    /// Leaves the turn's action untouched: a contact, a trade, a payment on a deal, an expedition on a table with Thunder's
    /// Edge on, or a leader printed "During the action phase:" (`Game::is_free_option`). Decided by the areas.
    pub free: bool,
    /// The "pass" option.
    pub pass: bool,
}

/// Whether the seat may take a second action this turn: Fleet Logistics and fewer than two actions taken.
#[must_use]
pub fn second_action(core: &CoreState, seat: Seat, actions_this_turn: u8) -> bool {
    actions_this_turn < 2 && core.players().by_seat(seat).is_some_and(|p| p.tech.owned.contains(Technology::FleetLogistics))
}

/// `Game::closing_options`: after an action the seat ends its turn explicitly (first, so a table that takes the first
/// option behaves as before) and may first do what takes no action. With a second action allowed everything but "pass" is
/// offered again. `expedition` is Thunder's Edge's end-of-turn commit (present only on a table with it on, when not yet
/// claimed this turn).
#[must_use]
pub fn closing_menu<T: Clone>(end_turn: T, turn: &[Entry<T>], second_action: bool, expedition: &[T]) -> Vec<T> {
    let mut out = vec![end_turn];
    for entry in turn {
        if entry.free || (second_action && !entry.pass) {
            out.push(entry.option.clone());
        }
    }
    out.extend(expedition.iter().cloned());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::Faction;
    use crate::state::{Player, Players};

    fn parts() -> Vec<(Source, Vec<&'static str>)> {
        vec![
            (Source::ActionCards, vec!["card0"]),
            (Source::Tactical, vec!["tactical"]),
            (Source::Strategic, vec!["strategic|a", "strategic|b"]),
            (Source::Leaders, vec!["agent"]),
            (Source::ThundersEdge, vec!["expedition"]),
            (Source::Transactions, vec!["open|x"]),
        ]
    }

    #[test]
    fn the_sources_are_put_together_in_the_order_of_the_menu() {
        let menu = turn_menu(&parts(), false, &[]);
        assert_eq!(menu, ["strategic|a", "strategic|b", "tactical", "open|x", "expedition", "agent", "card0"]);
        // A table with Thunder's Edge on commits when the turn closes: the component action is gone from the menu.
        assert_eq!(turn_menu(&parts(), true, &[]), ["strategic|a", "strategic|b", "tactical", "open|x", "agent", "card0"]);
    }

    #[test]
    fn a_failed_component_action_is_not_offered_again_but_pass_and_the_rest_stay() {
        let menu = turn_menu(&parts(), false, &["agent", "strategic|b"]);
        assert_eq!(menu, ["strategic|a", "tactical", "open|x", "expedition", "card0"]);
    }

    #[test]
    fn the_order_array_lists_every_source_once_in_declaration_order() {
        assert!(ORDER.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(ORDER.len(), 13);
    }

    fn entries() -> Vec<Entry<&'static str>> {
        vec![
            Entry { option: "pass", free: false, pass: true },
            Entry { option: "tactical", free: false, pass: false },
            Entry { option: "open|x", free: true, pass: false },
            Entry { option: "agent", free: true, pass: false },
            Entry { option: "card0", free: false, pass: false },
        ]
    }

    #[test]
    fn the_closing_menu_ends_the_turn_first_and_keeps_only_the_free_options() {
        assert_eq!(closing_menu("end_turn", &entries(), false, &[]), ["end_turn", "open|x", "agent"]);
    }

    #[test]
    fn a_second_action_offers_everything_but_pass_and_the_expedition_comes_last() {
        assert_eq!(closing_menu("end_turn", &entries(), true, &["commit"]), ["end_turn", "tactical", "open|x", "agent", "card0", "commit"]);
    }

    #[test]
    fn a_second_action_needs_fleet_logistics_and_fewer_than_two_actions() {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::ALL[0])]);
        assert!(!second_action(&s, Seat(0), 1), "no technology");
        s.players.edit(0).tech.owned.insert(Technology::FleetLogistics);
        assert!(second_action(&s, Seat(0), 1));
        assert!(!second_action(&s, Seat(0), 2), "the second action is the last");
        assert!(!second_action(&s, Seat(1), 0), "not seated");
    }
}
