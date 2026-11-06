//! Which asks the search decides (docs/MCTS_BOT_PLAN.md 2.2) and a stable name per ask kind.

use ti4_engine_core::choice::{objectives, strategy, tactical, turn, Ask, Choice};

/// The ask kinds the tree decides; every other ask is answered by the rule bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TreeKind {
    /// `Strategy(Draft)`.
    Draft,
    /// `Turn(ActionMenu)`: tactical, strategic, pass, component actions.
    Menu,
    /// `Turn(Closing)`: end the turn (or take the second action).
    Closing,
    /// `Tactical(Activate)` and Thunder's Edge Warfare's free tactical action (`Strategy(TeWarfareTactical)`), hard-filtered
    /// ([`crate::filter`]).
    Activate,
    /// `Tactical(Move)`: one hull per ask, or done.
    Move,
    /// `Objectives(Score)`: public and secret scoring.
    Score,
    /// `Strategy(ImperialScore)`.
    Imperial,
}

impl TreeKind {
    pub const ALL: [Self; 7] = [Self::Draft, Self::Menu, Self::Closing, Self::Activate, Self::Move, Self::Score, Self::Imperial];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Menu => "menu",
            Self::Closing => "closing",
            Self::Activate => "activate",
            Self::Move => "move",
            Self::Score => "score",
            Self::Imperial => "imperial",
        }
    }

    /// The index in [`Self::ALL`] (the feature id of the kind).
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// The tree kind of an ask, by the table of the plan; `None` for the asks the rule bot answers. The option count is not
/// looked at here (an ask with one option after the filter is never a node: the caller decides that).
#[must_use]
pub const fn tree_kind(ask: &Ask) -> Option<TreeKind> {
    match ask {
        Ask::Strategy(strategy::Ask::Draft) => Some(TreeKind::Draft),
        Ask::Strategy(strategy::Ask::ImperialScore) => Some(TreeKind::Imperial),
        Ask::Turn(turn::Ask::ActionMenu) => Some(TreeKind::Menu),
        Ask::Turn(turn::Ask::Closing { .. }) => Some(TreeKind::Closing),
        // Thunder's Edge Warfare (the product's default card set) activates a system without a token: the same decision.
        Ask::Tactical(tactical::Ask::Activate { .. }) | Ask::Strategy(strategy::Ask::TeWarfareTactical) => Some(TreeKind::Activate),
        Ask::Tactical(tactical::Ask::Move { .. }) => Some(TreeKind::Move),
        Ask::Objectives(objectives::Ask::Score { .. }) => Some(TreeKind::Score),
        _ => None,
    }
}

/// [`tree_kind`] of a choice.
#[must_use]
pub const fn tree_kind_of(choice: &Choice) -> Option<TreeKind> {
    tree_kind(&choice.ask)
}

/// `Area.Variant` of an ask (`Tactical.Activate`, `Strategy.Draft`, `Generic.Confirm`): the area and the variant name of its
/// `Debug` text, without the parameters. Stable as long as the variant names are.
#[must_use]
pub fn ask_name(ask: &Ask) -> String {
    let text = format!("{ask:?}");
    let (area, rest) = text.split_once('(').unwrap_or((text.as_str(), ""));
    let variant: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    if variant.is_empty() { area.to_owned() } else { format!("{area}.{variant}") }
}

/// FNV-1a of a text, for the hashed id features (ask names, option variants).
#[must_use]
pub fn fnv(text: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for b in text.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::Seat;

    #[test]
    fn names_drop_the_parameters() {
        assert_eq!(ask_name(&Ask::Tactical(tactical::Ask::Activate { seat: Seat(2) })), "Tactical.Activate");
        assert_eq!(ask_name(&Ask::Strategy(strategy::Ask::Draft)), "Strategy.Draft");
        assert_eq!(ask_name(&Ask::Turn(turn::Ask::Closing { second: true })), "Turn.Closing");
    }

    #[test]
    fn the_tree_kinds_follow_the_plan() {
        assert_eq!(tree_kind(&Ask::Strategy(strategy::Ask::Draft)), Some(TreeKind::Draft));
        assert_eq!(tree_kind(&Ask::Tactical(tactical::Ask::Move { seat: Seat(0) })), Some(TreeKind::Move));
        assert_eq!(tree_kind(&Ask::Objectives(objectives::Ask::Score { vp: 3 })), Some(TreeKind::Score));
        assert_eq!(tree_kind(&Ask::Strategy(strategy::Ask::WarfareRecall)), None);
        for (i, k) in TreeKind::ALL.iter().enumerate() {
            assert_eq!(k.index(), i);
        }
    }
}
