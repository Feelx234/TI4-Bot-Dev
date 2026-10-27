//! Typed readings of the card fields whose free text decides a rule.
//!
//! Five fields of the corpus are prose that the rules used to parse where they read it: the
//! printed window of an action card, the `target` of an agenda, the `abilityWindow` of a leader,
//! the `text` of a technology ("This technology cannot be researched.") and the printed `name` of a
//! strategy card. Each is mapped here, once, when a [`crate::Record`] is built, onto a small enum;
//! the rules read the enum ([`crate::Record::action_card_window`] and its siblings) and never the
//! text. The corpus itself is unchanged: it is the extraction, with its manifest and digest.
//!
//! Every table lists each distinct value the corpus prints, with the variant it maps to. There is
//! no catch-all: a value that is not in its table is a [`crate::ContentError::UnknownFieldValue`]
//! when the corpus is loaded, so a reworded card or a new printing fails loudly instead of
//! silently turning into a card that is never offered. The tables are the committed value mapping
//! that the card generator (`cargo xtask gen-cards`) takes from `card_fields.json` (the rows are
//! written to `card_fields_tables.rs`, which this file includes); see `docs/CARD_TEXT_FIELDS.md`.
//!
//! Values are compared after `trim()`, as the rules compared them before. No corpus value carries
//! surrounding whitespace today, so the trim changes nothing.

use ti4_model::content_types::ContentType;

/// A field value that its table does not list.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{field} value {value:?} is not in its table")]
pub struct UnknownValue {
    /// The corpus field (`window`, `target`, `abilityWindow`, `text`, `id`).
    pub field: &'static str,
    /// The value as the corpus prints it.
    pub value: String,
}

fn lookup<T: Copy>(
    table: &[(&'static str, T)],
    field: &'static str,
    value: &str,
) -> Result<T, UnknownValue> {
    let wanted = value.trim();
    table
        .iter()
        .find(|(printed, _)| *printed == wanted)
        .map(|(_, variant)| *variant)
        .ok_or_else(|| UnknownValue {
            field,
            value: value.to_owned(),
        })
}

// -- action cards: `window` -------------------------------------------------------------------------

macro_rules! action_card_windows {
    ($( $(#[$meta:meta])* $variant:ident = $printed:literal, )*) => {
        /// The printed window of an action card (`window`), one variant per printed sentence.
        ///
        /// [`Self::Action`] is a component action (22.1); every other variant is a reaction window,
        /// which the engine binds to an event (`ti4_engine::reactions::window_table`).
        ///
        /// Declared in the byte order of the printed text, so `Ord` (and the iteration order of a
        /// map keyed by it) is the order of the strings it replaces.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum ActionCardWindow {
            $( $(#[$meta])* #[doc = $printed] $variant, )*
        }

        impl ActionCardWindow {
            /// Every printed window of the corpus with its variant, in declaration order.
            pub const TABLE: &'static [(&'static str, Self)] = &[ $( ($printed, Self::$variant), )* ];

            /// The sentence as the corpus prints it.
            #[must_use]
            pub const fn printed(self) -> &'static str {
                match self {
                    $( Self::$variant => $printed, )*
                }
            }
        }
    };
}

// The window variants (the invocation of `action_card_windows!`) and the rows of the other value
// tables are generated from `card_fields.json` by `cargo xtask gen-cards`.
include!("card_fields_tables.rs");

impl ActionCardWindow {
    /// The variant of a printed window.
    ///
    /// # Errors
    /// [`UnknownValue`] for a window that [`Self::TABLE`] does not list.
    pub fn parse(printed: &str) -> Result<Self, UnknownValue> {
        lookup(Self::TABLE, "window", printed)
    }

    /// Whether the card is a component action (its window reads "Action", 22.1) rather than a
    /// reaction.
    #[must_use]
    pub const fn is_component_action(self) -> bool {
        matches!(self, Self::Action)
    }
}

// -- agendas: `target` --------------------------------------------------------------------------------

/// What an agenda elects, read from its printed `target`.
///
/// The variants are the outcome sets of 8.8 to 8.11 as the engine offers them; two printed
/// targets can share one (the "Elect Law" with a parenthetical discard rule elects a law like the
/// plain one: an empty ballot is already discarded).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ElectType {
    /// "For/Against".
    ForAgainst,
    /// "Elect Player": every seated player.
    Player,
    /// "Elect Planet": any controlled planet.
    Planet,
    /// "Elect Non-Home Planet Other Than Mecatol Rex".
    NonHomeNonMecatolPlanet,
    /// "Elect Law": a law in play.
    Law,
    /// "Elect Scored Secret Objective (...)". The engine offers no candidate (I03 of the card audit).
    ScoredSecretObjective,
    /// "Elect Strategy Card". The engine offers no candidate (I02 of the card audit).
    StrategyCard,
    /// Covert Legislation: the printed target is a rules sentence (the speaker draws the next agenda
    /// and reads its outcomes). It does not begin "Elect", so the engine votes it For/Against
    /// (I04 of the card audit).
    HiddenNextAgenda,
}

impl ElectType {
    /// Every printed `target` of the corpus with its variant.
    pub const TABLE: &'static [(&'static str, Self)] = ELECT_TYPE_ROWS;

    /// The variant of a printed target.
    ///
    /// # Errors
    /// [`UnknownValue`] for a target that [`Self::TABLE`] does not list.
    pub fn parse(printed: &str) -> Result<Self, UnknownValue> {
        lookup(Self::TABLE, "target", printed)
    }

    /// Whether the agenda elects a planet (either planet variant).
    #[must_use]
    pub const fn elects_planet(self) -> bool {
        matches!(self, Self::Planet | Self::NonHomeNonMecatolPlanet)
    }
}

// -- leaders: `abilityWindow` -----------------------------------------------------------------------

/// Whether a leader's ability is an action, read from its printed `abilityWindow`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LeaderAbilityKind {
    /// "ACTION:": a component action; using it ends the turn (OP-07).
    Action,
    /// "During the action phase:": usable on its owner's turn in the action phase without ending
    /// it (the Prophecy of Kings agents such as Carth of Golden Sands).
    ActionPhase,
    /// Any other window: a triggered or standing ability, never offered as a turn action.
    Other,
}

impl LeaderAbilityKind {
    /// Every printed `abilityWindow` of the corpus with its kind, in byte order of the text.
    pub const TABLE: &'static [(&'static str, Self)] = LEADER_ABILITY_KIND_ROWS;

    /// The kind of a printed ability window.
    ///
    /// # Errors
    /// [`UnknownValue`] for a window that [`Self::TABLE`] does not list.
    pub fn parse(printed: &str) -> Result<Self, UnknownValue> {
        lookup(Self::TABLE, "abilityWindow", printed)
    }

    /// Whether the leader is offered as an action-phase ability on its owner's turn.
    #[must_use]
    pub const fn is_action_phase_ability(self) -> bool {
        matches!(self, Self::Action | Self::ActionPhase)
    }

    /// Whether using the leader is a component action that ends the turn.
    #[must_use]
    pub const fn uses_the_action(self) -> bool {
        matches!(self, Self::Action)
    }
}

// -- technologies: `text` -------------------------------------------------------------------------

/// Whether a technology may be researched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResearchRestriction {
    /// The ordinary case.
    Researchable,
    /// The card prints "This technology cannot be researched." (it is gained another way).
    NotResearchable,
}

impl ResearchRestriction {
    /// The sentence a card prints, on a line of its own, when it cannot be researched.
    pub const MARKER: &'static str = "This technology cannot be researched.";

    /// The restriction a technology's `text` prints.
    ///
    /// The text is prose, so it is not tabled by value: a text whose last line is [`Self::MARKER`]
    /// is [`Self::NotResearchable`] and any other text that mentions "cannot be researched"
    /// (ignoring case) is an error, because the rule used to be a substring test and a text that
    /// only the old test would match must be looked at rather than guessed. Every other text is
    /// [`Self::Researchable`].
    ///
    /// # Errors
    /// [`UnknownValue`] for a text that mentions the restriction in another form.
    pub fn parse(text: &str) -> Result<Self, UnknownValue> {
        let mentions = text.to_ascii_lowercase().contains("cannot be researched");
        if !mentions {
            return Ok(Self::Researchable);
        }
        if text.lines().last().map(str::trim) == Some(Self::MARKER)
            && text
                .to_ascii_lowercase()
                .matches("cannot be researched")
                .count()
                == 1
        {
            return Ok(Self::NotResearchable);
        }
        Err(UnknownValue {
            field: "text",
            value: text.to_owned(),
        })
    }

    /// Whether the card may be researched at all.
    #[must_use]
    pub const fn is_researchable(self) -> bool {
        matches!(self, Self::Researchable)
    }
}

// -- strategy cards: `id` -----------------------------------------------------------------------------

/// Which of the eight strategy cards a printing is, mapped from the card id.
///
/// Every printing of one initiative maps to the same kind, which is how the rules dispatched it
/// when they matched the printed `name`: base and Codex Diplomacy (`base2`, `pok2diplomacy`) are
/// both [`Self::Diplomacy`] and resolve the same way, although the base card prints a different
/// secondary (see `docs/CARD_TEXT_FIELDS.md`; changing that is a separate, gated rules fix). The
/// Thunder's Edge printings that differ (`te4construction`, `te6warfare`) are told apart by id
/// where they are resolved.
///
/// Declared in initiative order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StrategyCardKind {
    Leadership,
    Diplomacy,
    Politics,
    Construction,
    Trade,
    Warfare,
    Technology,
    Imperial,
}

impl StrategyCardKind {
    /// Every strategy-card id of the corpus with its kind, in file order.
    pub const TABLE: &'static [(&'static str, Self)] = STRATEGY_CARD_KIND_ROWS;

    /// Every kind, in initiative order.
    pub const ALL: [Self; 8] = [
        Self::Leadership,
        Self::Diplomacy,
        Self::Politics,
        Self::Construction,
        Self::Trade,
        Self::Warfare,
        Self::Technology,
        Self::Imperial,
    ];

    /// The kind of a strategy-card id.
    ///
    /// # Errors
    /// [`UnknownValue`] for an id that [`Self::TABLE`] does not list.
    pub fn of_id(id: &str) -> Result<Self, UnknownValue> {
        lookup(Self::TABLE, "id", id)
    }

    /// The printed name every card of this kind carries ("Leadership", ...). Display text: the
    /// rules dispatch on the kind, never on this.
    #[must_use]
    pub const fn printed_name(self) -> &'static str {
        match self {
            Self::Leadership => "Leadership",
            Self::Diplomacy => "Diplomacy",
            Self::Politics => "Politics",
            Self::Construction => "Construction",
            Self::Trade => "Trade",
            Self::Warfare => "Warfare",
            Self::Technology => "Technology",
            Self::Imperial => "Imperial",
        }
    }
}

// -- per record -------------------------------------------------------------------------------------

/// The typed fields of one record, mapped when the record is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CardFields {
    /// A category with no typed text field.
    None,
    ActionCard(Option<ActionCardWindow>),
    Agenda(Option<ElectType>),
    Leader(Option<LeaderAbilityKind>),
    Technology(ResearchRestriction),
    StrategyCard(StrategyCardKind),
}

impl CardFields {
    /// Map the typed fields of a record of `category` from its JSON fields. A field that is absent
    /// (or not a string) maps to `None`, as reading it with `Record::text` did.
    pub(crate) fn of(
        category: ContentType,
        fields: &serde_json::Map<String, serde_json::Value>,
        identity: Option<&str>,
    ) -> Result<Self, UnknownValue> {
        let text = |key: &str| fields.get(key).and_then(serde_json::Value::as_str);
        Ok(match category {
            ContentType::ActionCards => {
                Self::ActionCard(text("window").map(ActionCardWindow::parse).transpose()?)
            }
            ContentType::Agendas => Self::Agenda(text("target").map(ElectType::parse).transpose()?),
            ContentType::Leaders => Self::Leader(
                text("abilityWindow")
                    .map(LeaderAbilityKind::parse)
                    .transpose()?,
            ),
            ContentType::Technologies => Self::Technology(
                text("text")
                    .map(ResearchRestriction::parse)
                    .transpose()?
                    .unwrap_or(ResearchRestriction::Researchable),
            ),
            ContentType::StrategyCards => {
                Self::StrategyCard(StrategyCardKind::of_id(identity.unwrap_or_default())?)
            }
            _ => Self::None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use ti4_model::content_types::ContentType;

    use super::*;
    use crate::loader::ContentStore;
    use crate::record::Record;

    fn records(category: ContentType) -> &'static [Record] {
        ContentStore::embedded().records(category)
    }

    /// Every distinct value of `field` in `category`, as printed.
    fn distinct(category: ContentType, field: &str) -> BTreeSet<&'static str> {
        records(category)
            .iter()
            .filter_map(|record| record.text(field))
            .collect()
    }

    fn table_keys<T>(table: &[(&'static str, T)]) -> BTreeSet<&'static str> {
        let keys: BTreeSet<_> = table.iter().map(|(printed, _)| *printed).collect();
        assert_eq!(keys.len(), table.len(), "a value is listed twice");
        keys
    }

    // -- every record maps, and every table row is a value the corpus prints ----------------------

    #[test]
    fn every_action_card_window_maps_and_every_row_is_printed() {
        for record in records(ContentType::ActionCards) {
            let printed = record
                .text("window")
                .expect("every action card prints a window");
            let window = record.action_card_window().expect("mapped at load");
            assert_eq!(window.printed(), printed.trim(), "{:?}", record.id());
        }
        assert_eq!(
            table_keys(ActionCardWindow::TABLE),
            distinct(ContentType::ActionCards, "window")
        );
    }

    #[test]
    fn every_agenda_target_maps_and_every_row_is_printed() {
        for record in records(ContentType::Agendas) {
            assert!(record.elect_type().is_some(), "{:?}", record.id());
        }
        assert_eq!(
            table_keys(ElectType::TABLE),
            distinct(ContentType::Agendas, "target")
        );
    }

    #[test]
    fn every_leader_window_maps_and_every_row_is_printed() {
        for record in records(ContentType::Leaders) {
            assert!(record.leader_ability_kind().is_some(), "{:?}", record.id());
        }
        assert_eq!(
            table_keys(LeaderAbilityKind::TABLE),
            distinct(ContentType::Leaders, "abilityWindow")
        );
    }

    #[test]
    fn every_technology_maps_and_exactly_the_two_obsidian_cards_cannot_be_researched() {
        let not_researchable: BTreeSet<&str> = records(ContentType::Technologies)
            .iter()
            .filter(|record| {
                record.research_restriction() == Some(ResearchRestriction::NotResearchable)
            })
            .filter_map(Record::id)
            .collect();
        assert_eq!(
            not_researchable,
            BTreeSet::from(["parasite-obs", "planesplitter-obs"])
        );
        assert!(
            records(ContentType::Technologies)
                .iter()
                .all(|record| record.research_restriction().is_some())
        );
    }

    #[test]
    fn every_strategy_card_maps_and_every_row_is_an_id_of_the_corpus() {
        for record in records(ContentType::StrategyCards) {
            assert!(record.strategy_card_kind().is_some(), "{:?}", record.id());
        }
        let ids: BTreeSet<&str> = records(ContentType::StrategyCards)
            .iter()
            .filter_map(Record::id)
            .collect();
        assert_eq!(table_keys(StrategyCardKind::TABLE), ids);
    }

    #[test]
    fn records_of_other_categories_have_no_typed_text_field() {
        let units = &records(ContentType::Units)[0];
        assert_eq!(units.action_card_window(), None);
        assert_eq!(units.elect_type(), None);
        assert_eq!(units.leader_ability_kind(), None);
        assert_eq!(units.research_restriction(), None);
        assert_eq!(units.strategy_card_kind(), None);
    }

    // -- the enums agree with the text tests they replace, on every record ------------------------

    #[test]
    fn the_window_variant_agrees_with_the_old_component_action_test() {
        for record in records(ContentType::ActionCards) {
            let printed = record.text("window").unwrap();
            let window = record.action_card_window().unwrap();
            // `action_cards::is_component_action` and `reactions::window_for` compared the trimmed
            // text; `keleres::has_component_action` compared it ignoring case.
            assert_eq!(window.is_component_action(), printed.trim() == "Action");
            assert_eq!(
                window.is_component_action(),
                printed.eq_ignore_ascii_case("action")
            );
        }
    }

    /// The old `vote::outcomes` reading of a target: the head before any '(' and its words.
    fn old_elect_reading(target: &str) -> &'static str {
        let head = target.split('(').next().unwrap_or(target).trim();
        if !head.starts_with("Elect") {
            return "for/against";
        }
        if head.contains("Player") {
            return "player";
        }
        if head.contains("Planet") {
            if head.contains("Non-Home") || head.contains("Other Than Mecatol") {
                return "non-home planet";
            }
            return "planet";
        }
        if head.contains("Law") {
            return "law";
        }
        "none"
    }

    #[test]
    fn the_elect_type_agrees_with_the_old_target_parsing() {
        for record in records(ContentType::Agendas) {
            let target = record.text("target").unwrap();
            let elects = record.elect_type().unwrap();
            let expected = match elects {
                ElectType::ForAgainst | ElectType::HiddenNextAgenda => "for/against",
                ElectType::Player => "player",
                ElectType::Planet => "planet",
                ElectType::NonHomeNonMecatolPlanet => "non-home planet",
                ElectType::Law => "law",
                ElectType::ScoredSecretObjective | ElectType::StrategyCard => "none",
            };
            assert_eq!(old_elect_reading(target), expected, "{target:?}");
            // game.rs (Committee Formation) and the bot read these two forms separately.
            assert_eq!(
                target.starts_with("Elect Player"),
                elects == ElectType::Player
            );
            assert_eq!(target == "For/Against", elects == ElectType::ForAgainst);
            let head = target.split('(').next().unwrap_or(target).trim();
            assert_eq!(
                head.starts_with("Elect") && !head.contains("Player") && head.contains("Planet"),
                elects.elects_planet()
            );
        }
    }

    #[test]
    fn the_leader_kind_agrees_with_the_old_window_tests() {
        for record in records(ContentType::Leaders) {
            let window = record.text("abilityWindow").unwrap();
            let kind = record.leader_ability_kind().unwrap();
            let printed = window.trim().trim_end_matches(':').trim_end();
            assert_eq!(
                kind.is_action_phase_ability(),
                printed.starts_with("ACTION")
                    || printed.eq_ignore_ascii_case("during the action phase"),
                "{window:?}"
            );
            assert_eq!(
                kind.uses_the_action(),
                window.trim().starts_with("ACTION"),
                "{window:?}"
            );
        }
    }

    #[test]
    fn the_research_restriction_agrees_with_the_old_substring_test() {
        for record in records(ContentType::Technologies) {
            let old = record
                .text("text")
                .is_some_and(|text| text.to_ascii_lowercase().contains("cannot be researched"));
            assert_eq!(
                !record.research_restriction().unwrap().is_researchable(),
                old,
                "{:?}",
                record.id()
            );
        }
    }

    #[test]
    fn the_strategy_card_kind_agrees_with_the_printed_name() {
        // The rules dispatched on `name`; the kind from the id must be the same on every card,
        // or dispatching by kind would change a game.
        for record in records(ContentType::StrategyCards) {
            assert_eq!(
                Some(record.strategy_card_kind().unwrap().printed_name()),
                record.text("name"),
                "{:?}",
                record.id()
            );
        }
    }

    // -- the full value tables, as reviewed --------------------------------------------------------

    #[test]
    fn the_action_card_window_table() {
        use ActionCardWindow as W;
        let expected: BTreeMap<&str, ActionCardWindow> = [
            ("Action", W::Action),
            ("After 1 of your ships is destroyed during a space combat", W::AfterOwnShipDestroyedInSpaceCombat),
            ("After 2 of your neighbors resolve a transaction", W::AfterNeighborsTransact),
            ("After a player moves ships into a system that contains your ships", W::AfterShipsMoveIntoSystemWithYourShips),
            ("After an agenda is revealed", W::AfterAgendaRevealed),
            ("After another player activates a system that contains 1 of your command tokens", W::AfterOtherActivatesSystemWithYourCommandToken),
            ("After another player activates a system that contains 1 or more of your structures", W::AfterOtherActivatesSystemWithYourStructures),
            ("After another player activates a system that contains your units", W::AfterOtherActivatesSystemWithYourUnits),
            ("After another player commits units to land on a planet you control", W::AfterOtherCommitsToYourPlanet),
            ("After another player discards an action card that has a component action", W::AfterOtherDiscardsComponentActionCard),
            ("After another player gains control of a planet you control", W::AfterOtherGainsYourPlanet),
            ("After another player makes a BOMBARDMENT, SPACE CANNON, or ANTI-FIGHTER BARRAGE roll", W::AfterOtherRollsUnitAbility),
            ("After another player's ship uses SUSTAIN DAMAGE to cancel a hit produced by your units or abilities", W::AfterOtherSustainsAgainstYourHit),
            ("After the active player moves ships into the active system during a tactical action", W::AfterActivePlayerMovesIntoActiveSystem),
            ("After the speaker votes on an agenda", W::AfterSpeakerVotes),
            ("After you activate a system", W::AfterYouActivateSystem),
            ("After you activate a system that contains 1 or more of your ships", W::AfterYouActivateSystemWithYourShips),
            ("After you activate a system that contains another player's ships", W::AfterYouActivateSystemWithOtherShips),
            ("After you activate an anomaly", W::AfterYouActivateAnomaly),
            ("After you cast votes on an outcome of an agenda", W::AfterYouCastVotes),
            ("After you perform an action", W::AfterYouPerformAction),
            ("After you win a space combat", W::AfterYouWinSpaceCombat),
            ("After your ground forces make combat rolls during a round of ground combat", W::AfterYourGroundForcesRoll),
            ("After your opponent declares a retreat during a space combat", W::AfterOpponentDeclaresRetreat),
            ("At the end of a player's turn, if you have passed", W::EndOfTurnIfYouPassed),
            ("At the end of any players turn, if there are at least 2 players who have not passed", W::EndOfTurnIfTwoNotPassed),
            ("At the start of a combat", W::StartOfCombat),
            ("At the start of a combat round", W::StartOfCombatRound),
            ("At the start of an invasion", W::StartOfInvasion),
            ("At the start of an invasion in a system that contains 1 or more of your opponents' PDS units", W::StartOfInvasionAgainstPds),
            ("At the start of another player's turn, if they have a readied strategy card", W::StartOfOtherTurnWithReadiedStrategyCard),
            ("At the start of the 'Announce Retreats' step of space combat, if you are the defender", W::StartOfAnnounceRetreatsAsDefender),
            ("At the start of the agenda phase", W::StartOfAgendaPhase),
            ("At the start of the first round of a space combat", W::StartOfFirstSpaceCombatRound),
            ("At the start of the strategy phase", W::StartOfStrategyPhase),
            ("At the start or end of a combat round", W::StartOrEndOfCombatRound),
            ("Before you assign hits produced by another player's SPACE CANNON roll", W::BeforeAssigningSpaceCannonHits),
            ("Before you assign hits to your ships during a space combat", W::BeforeAssigningSpaceCombatHits),
            ("Before you roll dice for ANTI-FIGHTER BARRAGE", W::BeforeAntiFighterBarrageRoll),
            ("During the agenda phase when an outcome would be resolved", W::WhenOutcomeWouldBeResolved),
            ("When 1 or more of your units use PRODUCTION", W::WhenYourUnitsUseProduction),
            ("When an agenda is revealed", W::WhenAgendaRevealed),
            ("When another player chooses a strategy card during the strategy phase", W::WhenOtherChoosesStrategyCard),
            ("When another player is elected as the outcome of an agenda", W::WhenOtherIsElected),
            ("When another player plays an action card other than 'Sabotage'", W::WhenOtherPlaysActionCardNotSabotage),
            ("When another player would perform a strategic action", W::WhenOtherPerformsStrategicAction),
            ("When one of your ships uses SUSTAIN DAMAGE during combat", W::WhenYourShipSustainsDamage),
            ("When you are elected as the outcome of an agenda", W::WhenYouAreElected),
            ("When you are negotiating a transaction", W::WhenNegotiatingTransaction),
            ("When you gain control of a planet", W::WhenYouGainPlanet),
            ("When you would return your strategy card(s) during the status phase", W::WhenYouWouldReturnStrategyCards),
            ("When your last ship in the active system is destroyed", W::WhenYourLastShipIsDestroyed),
        ]
        .into_iter()
        .collect();
        assert_eq!(expected.len(), 52);
        let actual: BTreeMap<&str, ActionCardWindow> =
            ActionCardWindow::TABLE.iter().copied().collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn the_action_card_window_order_is_the_order_of_the_printed_text() {
        // A map keyed by the variant iterates as the map keyed by the sentence did.
        let mut by_text: Vec<ActionCardWindow> =
            ActionCardWindow::TABLE.iter().map(|(_, w)| *w).collect();
        by_text.sort_by_key(|w| w.printed());
        let mut by_variant = by_text.clone();
        by_variant.sort();
        assert_eq!(by_text, by_variant);
    }

    #[test]
    fn the_elect_type_table() {
        use ElectType as E;
        let rows: Vec<(String, ElectType)> = ElectType::TABLE
            .iter()
            .map(|(printed, elects)| ((*printed).to_owned(), *elects))
            .collect();
        let short = |printed: &str| {
            printed
                .split(" (")
                .next()
                .unwrap()
                .chars()
                .take(40)
                .collect::<String>()
        };
        let summary: Vec<(String, ElectType)> = rows.iter().map(|(p, e)| (short(p), *e)).collect();
        assert_eq!(
            summary,
            vec![
                ("For/Against".to_owned(), E::ForAgainst),
                ("Elect Player".to_owned(), E::Player),
                ("Elect Planet".to_owned(), E::Planet),
                (
                    "Elect Non-Home Planet Other Than Mecatol".to_owned(),
                    E::NonHomeNonMecatolPlanet
                ),
                ("Elect Law".to_owned(), E::Law),
                ("Elect Law".to_owned(), E::Law),
                (
                    "Elect Scored Secret Objective".to_owned(),
                    E::ScoredSecretObjective
                ),
                ("Elect Strategy Card".to_owned(), E::StrategyCard),
                (
                    "When this agenda is revealed, the speake".to_owned(),
                    E::HiddenNextAgenda
                ),
            ]
        );
        // How many agendas print each.
        let mut count: BTreeMap<ElectType, usize> = BTreeMap::new();
        for record in records(ContentType::Agendas) {
            *count.entry(record.elect_type().unwrap()).or_default() += 1;
        }
        assert_eq!(
            count,
            BTreeMap::from([
                (E::ForAgainst, 27),
                (E::Player, 20),
                (E::Planet, 10),
                (E::NonHomeNonMecatolPlanet, 1),
                (E::Law, 2),
                (E::ScoredSecretObjective, 1),
                (E::StrategyCard, 1),
                (E::HiddenNextAgenda, 1),
            ])
        );
    }

    #[test]
    fn the_leader_ability_kind_table() {
        let action: Vec<&str> = LeaderAbilityKind::TABLE
            .iter()
            .filter(|(_, kind)| *kind == LeaderAbilityKind::Action)
            .map(|(printed, _)| *printed)
            .collect();
        let action_phase: Vec<&str> = LeaderAbilityKind::TABLE
            .iter()
            .filter(|(_, kind)| *kind == LeaderAbilityKind::ActionPhase)
            .map(|(printed, _)| *printed)
            .collect();
        assert_eq!(action, ["ACTION:"]);
        assert_eq!(action_phase, ["During the action phase:"]);
        // The other 61 printed windows (listed in `LeaderAbilityKind::TABLE`) are `Other`.
        assert_eq!(LeaderAbilityKind::TABLE.len(), 63);
        let mut count: BTreeMap<LeaderAbilityKind, usize> = BTreeMap::new();
        for record in records(ContentType::Leaders) {
            *count
                .entry(record.leader_ability_kind().unwrap())
                .or_default() += 1;
        }
        assert_eq!(
            count,
            BTreeMap::from([
                (LeaderAbilityKind::Action, 30),
                (LeaderAbilityKind::ActionPhase, 2),
                (LeaderAbilityKind::Other, 71),
            ])
        );
        let mut sorted: Vec<&str> = LeaderAbilityKind::TABLE.iter().map(|(p, _)| *p).collect();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            LeaderAbilityKind::TABLE
                .iter()
                .map(|(p, _)| *p)
                .collect::<Vec<_>>(),
            "kept in byte order"
        );
    }

    #[test]
    fn the_strategy_card_kind_table() {
        use StrategyCardKind as K;
        assert_eq!(
            StrategyCardKind::TABLE,
            &[
                ("base2", K::Diplomacy),
                ("base4", K::Construction),
                ("pok1leadership", K::Leadership),
                ("pok2diplomacy", K::Diplomacy),
                ("pok3politics", K::Politics),
                ("pok4construction", K::Construction),
                ("pok5trade", K::Trade),
                ("pok6warfare", K::Warfare),
                ("pok7technology", K::Technology),
                ("pok8imperial", K::Imperial),
                ("te4construction", K::Construction),
                ("te6warfare", K::Warfare),
            ]
        );
        let initiative: Vec<i64> = StrategyCardKind::ALL
            .iter()
            .map(|kind| {
                records(ContentType::StrategyCards)
                    .iter()
                    .find(|record| record.strategy_card_kind() == Some(*kind))
                    .and_then(|record| record.int("initiative"))
                    .unwrap()
            })
            .collect();
        assert_eq!(initiative, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    // -- unknown values fail loudly ----------------------------------------------------------------

    #[test]
    fn an_unknown_value_is_rejected_by_each_parser() {
        assert_eq!(
            ActionCardWindow::parse("After you activate a sistem"),
            Err(UnknownValue {
                field: "window",
                value: "After you activate a sistem".to_owned()
            })
        );
        assert!(ElectType::parse("Elect Planet Or Player").is_err());
        assert!(LeaderAbilityKind::parse("ACTION").is_err());
        assert!(StrategyCardKind::of_id("pok9bureaucracy").is_err());
        // The restriction worded in another way is not guessed either way.
        assert!(ResearchRestriction::parse("This card cannot be researched by you.").is_err());
        assert!(
            ResearchRestriction::parse("Gain 1 trade good.\nThis technology cannot be researched.")
                .is_ok_and(|r| r == ResearchRestriction::NotResearchable)
        );
        assert_eq!(
            ResearchRestriction::parse("Research 1 technology."),
            Ok(ResearchRestriction::Researchable)
        );
    }

    #[test]
    fn a_record_with_an_unknown_value_does_not_load() {
        use serde_json::json;
        let err = Record::new(
            ContentType::ActionCards,
            7,
            json!({"alias": "x", "source": "base", "window": "Whenever you like"}),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            crate::ContentError::UnknownFieldValue { index: 7, field: "window", ref value, .. }
                if value == "Whenever you like"
        ));
        assert!(
            Record::new(
                ContentType::Agendas,
                0,
                json!({"alias": "x", "source": "base", "target": "Elect Anything"}),
            )
            .is_err()
        );
        assert!(
            Record::new(
                ContentType::Leaders,
                0,
                json!({"id": "x", "source": "base", "abilityWindow": "Now:"}),
            )
            .is_err()
        );
        assert!(
            Record::new(
                ContentType::StrategyCards,
                0,
                json!({"id": "pok9x", "source": "base", "name": "Leadership"}),
            )
            .is_err()
        );
        // An absent field still loads, as it did, and reads as absent.
        let quiet = Record::new(
            ContentType::ActionCards,
            0,
            json!({"alias": "x", "source": "base"}),
        )
        .unwrap();
        assert_eq!(quiet.action_card_window(), None);
    }
}
