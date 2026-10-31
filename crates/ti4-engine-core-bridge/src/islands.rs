//! The islands of the zero-bail work (`docs/todo/D2-islands-machines.md` and `D2-islands-composition.md`) and the sort of a counted
//! bail into one of them: the `playthrough` bin prints its bails per island, so a lane sees only the rows it owns and a green row
//! is visible at once.
//!
//! A bail is counted under a key `<label> [bail: <why>]` (or `[gate: <why>]`, a position a call site runs bare). The label names
//! the place (`d1::step`, `invasion::control_gain_exploration`, `turn::hooks`), the why often the arm (`tactical`, `ask.closing`).
//! [`island_of`] reads both, case-insensitively, by the FIRST rule that matches; rules are ordered so that the most specific word
//! wins (`strategy_announce` is the turn flow's, not the strategic action's). A key no rule knows is [`Island::Other`] and is
//! printed as such: a new cause is never hidden, and the table below is the place to teach it.

use std::collections::BTreeMap;

/// One of the places the remaining bails belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Island {
    /// M1: the trade window and the transactions.
    Trade,
    /// M2: the production step.
    Production,
    /// M3: the invasion (commit, bombardment, landing, ground combat, control change).
    Invasion,
    /// M4: space combat as the aftermath's fight.
    Combat,
    /// M5: the aftermath chain.
    Aftermath,
    /// M6: the tactical window (activation, movement).
    Tactical,
    /// Lane B: turn hooks, phase events, the step prelude, the ask-side phases (draft, closing, starting technology).
    TurnFlow,
    /// Lane C: the status phase (scoring, token gain, the status steps).
    Status,
    /// Lane D: the agenda phase.
    Agenda,
    /// Lane E: the strategic action (primary, secondary, followers).
    Strategic,
    /// Lane F: the action menu, leaders, exploration, relics, action cards, components.
    Menu,
    /// Nothing above knows the cause.
    Other,
}

impl Island {
    /// Every island, in the order of the report.
    pub const ALL: [Self; 12] = [
        Self::Trade,
        Self::Production,
        Self::Invasion,
        Self::Combat,
        Self::Aftermath,
        Self::Tactical,
        Self::TurnFlow,
        Self::Status,
        Self::Agenda,
        Self::Strategic,
        Self::Menu,
        Self::Other,
    ];

    /// The lane that owns it and its name, as the report prints it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Trade => "M1 trade",
            Self::Production => "M2 production",
            Self::Invasion => "M3 invasion",
            Self::Combat => "M4 combat",
            Self::Aftermath => "M5 aftermath",
            Self::Tactical => "M6 tactical",
            Self::TurnFlow => "B turn flow",
            Self::Status => "C status",
            Self::Agenda => "D agenda",
            Self::Strategic => "E strategic action",
            Self::Menu => "F menu and leaders",
            Self::Other => "other",
        }
    }
}

/// The words of each island, in the order the rules are tried (the first island with a word in the key wins).
const RULES: [(Island, &[&str]); 15] = [
    (Island::Trade, &["trade", "transaction"]),
    (Island::Production, &["production", "produce", "purchase"]),
    (Island::Invasion, &["invasion", "bombard", "landing", "ground_combat", "control_gain", "custodians"]),
    (Island::Combat, &["combat", "cannon", "barrage", "retreat", "sustain", "casualt"]),
    (Island::Aftermath, &["aftermath"]),
    // The follower window of the strategic action is the strategic lane's even when it follows a tactical action.
    (Island::Strategic, &["secondary", "leader_followers"]),
    (Island::Tactical, &["tactical", "activation", "movement"]),
    // The words of the live path's own gaps (`native_play`): the commander unlock is the menu lane's although the turn flow raises it.
    (Island::Menu, &["commanderunlock", "leader_action"]),
    // The turn flow before the strategic action: `strategy_announce` is the phase's, not the card's.
    (Island::TurnFlow, &["turn_prelude", "turn::", "prelude", "phase", "closing", "draft", "starting_tech", "strategy_announce", "end_turn", "hooks", "turngap", "reconcile"]),
    (Island::Status, &["status", "token", "scoring", "objective"]),
    (Island::Agenda, &["agenda", "voting", "vote", "diplomacy"]),
    (Island::Strategic, &["strategy", "primary"]),
    (Island::Menu, &["action_menu", "leader", "hero", "agent", "commander", "explor", "relic", "action_card", "component", "promissory"]),
    // The live path's `Unknown` words (`ti4_engine_core::kernel::ability::Unknown`): the invasion's `Unported(..)` reasons, and the card plays and
    // abilities the reaction port owns (listed with the menu lane: they are cards and leaders).
    (Island::Invasion, &["unported("]),
    (Island::Menu, &["playfacts", "unportedcard", "immediatecardplay", "harnessorinfiltrate", "briberygoods", "cardcost", "dataskimmer", "pendingdestructions", "nomadagentexhaustion", "missingfact"]),
];

/// The island a counted cause belongs to: `key` is the whole `<label> [bail: <why>]` (the label alone also works).
#[must_use]
pub fn island_of(key: &str) -> Island {
    let key = key.to_ascii_lowercase();
    RULES.iter().find(|(_, words)| words.iter().any(|w| key.contains(w))).map_or(Island::Other, |(island, _)| *island)
}

/// Whether a window key of the dual report is a counted bail or gate (the rest of the report is the windows that ran).
#[must_use]
pub fn is_cause(key: &str) -> bool {
    key.contains("[bail:") || key.contains("[gate:")
}

/// What the bails of one island came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IslandRow {
    /// Bails and gates counted.
    pub count: u64,
    /// The distinct causes, biggest first.
    pub causes: Vec<(String, u64)>,
}

/// The counted causes (`key -> count`, any other key is ignored) sorted into islands. Every island is present, an empty row means
/// the island is green in this run.
#[must_use]
pub fn by_island<'a>(causes: impl IntoIterator<Item = (&'a str, u64)>) -> BTreeMap<Island, IslandRow> {
    let mut out: BTreeMap<Island, IslandRow> = Island::ALL.iter().map(|i| (*i, IslandRow::default())).collect();
    for (key, n) in causes {
        if !is_cause(key) || n == 0 {
            continue;
        }
        let row = out.entry(island_of(key)).or_default();
        row.count += n;
        row.causes.push((key.to_owned(), n));
    }
    for row in out.values_mut() {
        row.causes.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    }
    out
}

/// The report lines of [`by_island`]: one line per island, then under every island that is not green its biggest `top` causes.
#[must_use]
pub fn render(rows: &BTreeMap<Island, IslandRow>, top: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for island in Island::ALL {
        let Some(row) = rows.get(&island) else { continue };
        if row.count == 0 {
            lines.push(format!("{:>9}  {:<20} green", 0, island.name()));
            continue;
        }
        lines.push(format!("{:>9}  {:<20} {} causes", row.count, island.name(), row.causes.len()));
        for (cause, n) in row.causes.iter().take(top) {
            lines.push(format!("{:>9}      {cause}", n));
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The causes of the first playthrough (docs/D2_LF_RESUME.md section 6), each with the island its lane is.
    #[test]
    fn the_causes_of_the_first_playthrough_are_sorted_into_their_lanes() {
        let cases = [
            ("d1::step [bail: tactical]", Island::Tactical),
            ("d1::step [bail: aftermath]", Island::Aftermath),
            ("d1::step [bail: turn_prelude]", Island::TurnFlow),
            ("turn::hooks [bail: legendary::end_turn]", Island::TurnFlow),
            ("turn::hooks [bail: minister_of_war]", Island::TurnFlow),
            ("turn::hooks [bail: technology::end_turn]", Island::TurnFlow),
            ("d1::step [bail: ask.closing]", Island::TurnFlow),
            ("invasion::control_gain_exploration [gate: exploration]", Island::Invasion),
            ("d1::step [bail: ask.draft]", Island::TurnFlow),
            ("d1::step [bail: tokens]", Island::Status),
            ("d1::step [bail: trade]", Island::Trade),
            ("d1::step [bail: status]", Island::Status),
            ("d1::step [bail: strategy_announce]", Island::TurnFlow),
            ("d1::step [bail: secondary]", Island::Strategic),
            ("d1::pending.close [bail: StrategySecondaryAfterTactical]", Island::Strategic),
            ("strategy_cards::technology_primary [bail: a seat whose own effects]", Island::Strategic),
            ("d1::step [bail: agenda]", Island::Agenda),
            ("d1::step [bail: ask.action_menu]", Island::Menu),
            ("d1::step [bail: something new]", Island::Other),
            // The gaps of the native playthrough (`native_play::Gap::key`).
            ("ask.action_menu [gap: ask source not native: question source action_menu]", Island::Menu),
            ("ask.secondary [gap: ask source not native: question source secondary]", Island::Strategic),
            ("status [gap: arm not native: no live function]", Island::Status),
            ("tactical [gap: bail of a live window: UnportedTactical(ActivationHook)]", Island::Tactical),
            ("live [gap: bail of a live window: Unported(Dacxive)]", Island::Invasion),
            ("ask.closing [gap: bail of a live window: TurnGap(MenuSources)]", Island::TurnFlow),
            ("turn_prelude [gap: bail of a live window: TurnGap(CommanderUnlock)]", Island::Menu),
            ("aftermath [gap: bail of a live window: UnportedAftermath(CombatUnported)]", Island::Combat),
            ("live [gap: bail of a live window: PlayFacts]", Island::Menu),
            ("tokens [gap: close lent: the status steps 81.6-81.8 after the last token (KS5, lane C)]", Island::Status),
        ];
        for (key, island) in cases {
            assert_eq!(island_of(key), island, "{key}");
        }
    }

    #[test]
    fn the_first_island_with_a_word_in_the_key_wins() {
        // Trade before production: the trade window's purchase of a ... is the trade lane's.
        assert_eq!(island_of("live::trade [bail: production]"), Island::Trade);
        // Case does not matter.
        assert_eq!(island_of("D1::STEP [BAIL: TACTICAL]"), Island::Tactical);
        // The label alone sorts too.
        assert_eq!(island_of("aftermath"), Island::Aftermath);
    }

    #[test]
    fn a_report_has_every_island_and_counts_only_causes() {
        let rows = by_island([
            ("d1::step [bail: tactical]", 110),
            ("d1::step [bail: aftermath]", 64),
            ("invasion::control_gain_exploration [gate: x]", 21),
            ("invasion::control_gain_exploration [gate: y]", 25),
            ("d1::step", 9_999),
            ("d1::step [bail: trade]", 0),
        ]);
        assert_eq!(rows.len(), Island::ALL.len());
        assert_eq!(rows[&Island::Tactical].count, 110);
        assert_eq!(rows[&Island::Invasion].count, 46);
        assert_eq!(rows[&Island::Invasion].causes[0], ("invasion::control_gain_exploration [gate: y]".to_owned(), 25), "biggest first");
        assert_eq!(rows[&Island::Trade], IslandRow::default(), "a zero count is not a cause");
        assert_eq!(rows.values().map(|r| r.count).sum::<u64>(), 110 + 64 + 46, "a window that ran is not a bail");
    }

    #[test]
    fn the_rendering_shows_green_islands_and_the_top_causes_of_the_others() {
        let rows = by_island([("a [bail: tactical]", 3), ("b [bail: tactical movement]", 2), ("c [bail: tactical x]", 1)]);
        let lines = render(&rows, 2);
        assert!(lines.iter().any(|l| l.contains("M1 trade") && l.ends_with("green")), "{lines:#?}");
        assert!(lines.iter().any(|l| l.contains("M6 tactical") && l.ends_with("3 causes")), "{lines:#?}");
        assert_eq!(lines.iter().filter(|l| l.contains("[bail:")).count(), 2, "only the top two causes");
    }
}
