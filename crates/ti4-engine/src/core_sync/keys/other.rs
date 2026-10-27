//! The smaller codecs: agenda outcomes, ledger reasons, destruction causes, deck keys.

use ti4_engine_core::generated::{
    Agenda, AgendaElects, Leader, Planet, Relic, SecretObjective, StrategyCard,
};
use ti4_engine_core::keys::{
    ActionCardCause, DestroyReason, Expedition, ExploreDeck, FactionAbilityCause, LedgerReason,
    Outcome, UnitAbilityCause,
};

use super::text::Codec;
use crate::generated::IdStr;

impl Codec {
    /// The outcome with this text. `elects` is the agenda's row: with it only the kind the agenda
    /// elects is tried (`For` / `Against` / the empty text always are); without it the kinds are
    /// tried in the order of [`Outcome`]'s variants.
    #[must_use]
    pub fn parse_outcome(&self, text: &str, elects: Option<AgendaElects>) -> Option<Outcome> {
        match text {
            "" => return Some(Outcome::Empty),
            "for" => return Some(Outcome::For),
            "against" => return Some(Outcome::Against),
            _ => {}
        }
        let kinds: &[AgendaElects] = match elects {
            Some(AgendaElects::ForAgainst | AgendaElects::HiddenNextAgenda) => return None,
            Some(AgendaElects::Law) => &[AgendaElects::Law],
            Some(AgendaElects::Player) => &[AgendaElects::Player],
            Some(AgendaElects::Planet | AgendaElects::NonHomeNonMecatolPlanet) => {
                &[AgendaElects::Planet]
            }
            Some(AgendaElects::ScoredSecretObjective) => &[AgendaElects::ScoredSecretObjective],
            Some(AgendaElects::StrategyCard) => &[AgendaElects::StrategyCard],
            None => &[
                AgendaElects::Player,
                AgendaElects::Planet,
                AgendaElects::StrategyCard,
                AgendaElects::Law,
                AgendaElects::ScoredSecretObjective,
            ],
        };
        kinds.iter().find_map(|kind| match kind {
            AgendaElects::Player => self.owners.as_ref()?.seat(text).map(Outcome::Seat),
            AgendaElects::Planet | AgendaElects::NonHomeNonMecatolPlanet => {
                Planet::from_id(text).map(Outcome::Planet)
            }
            AgendaElects::StrategyCard => StrategyCard::from_id(text).map(Outcome::StrategyCard),
            AgendaElects::Law => Agenda::from_id(text).map(Outcome::Law),
            AgendaElects::ScoredSecretObjective => {
                SecretObjective::from_id(text).map(Outcome::Secret)
            }
            AgendaElects::ForAgainst | AgendaElects::HiddenNextAgenda => None,
        })
    }

    /// The text of an outcome.
    #[must_use]
    pub fn render_outcome(&self, outcome: Outcome) -> Option<String> {
        Some(match outcome {
            Outcome::Empty => String::new(),
            Outcome::For => "for".to_owned(),
            Outcome::Against => "against".to_owned(),
            Outcome::Seat(s) => self.owners.as_ref()?.id(s)?.to_owned(),
            Outcome::Planet(x) => x.as_str().to_owned(),
            Outcome::StrategyCard(x) => x.as_str().to_owned(),
            Outcome::Law(x) => x.as_str().to_owned(),
            Outcome::Secret(x) => x.as_str().to_owned(),
        })
    }
}

/// The text of the preset's starting points row.
pub const PRESET_START: &str = "Preset starting points";

const LEDGER_WORDS: [(&str, LedgerReason); 8] = [
    ("custodians", LedgerReason::Custodians),
    ("imperial_primary", LedgerReason::ImperialPrimary),
    ("imperial_rider", LedgerReason::ImperialRider),
    ("objective", LedgerReason::Objective),
    (PRESET_START, LedgerReason::PresetStart),
    ("secret_objective", LedgerReason::SecretObjective),
    ("styx", LedgerReason::Styx),
    ("support_for_the_throne", LedgerReason::SupportForTheThrone),
];

/// The ledger reason with this text.
#[must_use]
pub fn parse_ledger(text: &str) -> Option<LedgerReason> {
    if let Some((_, reason)) = LEDGER_WORDS.iter().find(|(word, _)| *word == text) {
        return Some(*reason);
    }
    if let Some(alias) = text.strip_prefix("agenda:") {
        return Agenda::from_id(alias).map(LedgerReason::Agenda);
    }
    Relic::from_id(text).map(LedgerReason::Relic)
}

/// The text of a ledger reason.
#[must_use]
pub fn render_ledger(reason: LedgerReason) -> String {
    match reason {
        LedgerReason::Agenda(a) => format!("agenda:{}", a.as_str()),
        LedgerReason::Relic(r) => r.as_str().to_owned(),
        word => LEDGER_WORDS
            .iter()
            .find(|(_, r)| *r == word)
            .map(|(text, _)| (*text).to_owned())
            .unwrap_or_default(),
    }
}

const DESTROY_WORDS: [(&str, DestroyReason); 11] = [
    ("bombardment", DestroyReason::Bombardment),
    (
        "breakthrough:nova_seed",
        DestroyReason::BreakthroughNovaSeed,
    ),
    ("ground_combat", DestroyReason::GroundCombat),
    ("last_dispatch", DestroyReason::LastDispatch),
    ("nova_seed", DestroyReason::NovaSeed),
    ("plot_seethe", DestroyReason::PlotSeethe),
    ("retreat_capacity", DestroyReason::RetreatCapacity),
    ("space_combat", DestroyReason::SpaceCombat),
    ("sundered", DestroyReason::Sundered),
    (
        "technology:impulse_core",
        DestroyReason::TechnologyImpulseCore,
    ),
    (
        "technology_parasite_obs",
        DestroyReason::TechnologyParasiteObs,
    ),
];

const ACTION_CARD_CAUSES: [(&str, ActionCardCause); 5] = [
    ("courageous", ActionCardCause::Courageous),
    ("direct_hit", ActionCardCause::DirectHit),
    ("plague", ActionCardCause::Plague),
    ("reflective_shielding", ActionCardCause::ReflectiveShielding),
    ("unstable_planet", ActionCardCause::UnstablePlanet),
];

const FACTION_ABILITIES: [(&str, FactionAbilityCause); 1] =
    [("devotion", FactionAbilityCause::Devotion)];

const UNIT_ABILITIES: [(&str, UnitAbilityCause); 3] = [
    ("anti_fighter_barrage", UnitAbilityCause::AntiFighterBarrage),
    ("space_cannon", UnitAbilityCause::SpaceCannon),
    ("yin_flagship", UnitAbilityCause::YinFlagship),
];

/// The destruction cause with this text.
#[must_use]
pub fn parse_destroy(text: &str) -> Option<DestroyReason> {
    if let Some((_, reason)) = DESTROY_WORDS.iter().find(|(word, _)| *word == text) {
        return Some(*reason);
    }
    if let Some(id) = text.strip_prefix("action_card:") {
        return ACTION_CARD_CAUSES
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, cause)| DestroyReason::ActionCard(*cause));
    }
    if let Some(id) = text.strip_prefix("agenda:") {
        return Agenda::from_id(id).map(DestroyReason::Agenda);
    }
    if let Some(id) = text.strip_prefix("faction_ability:") {
        return FACTION_ABILITIES
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, cause)| DestroyReason::FactionAbility(*cause));
    }
    if let Some(id) = text.strip_prefix("leader:") {
        return Leader::from_id(id).map(DestroyReason::Leader);
    }
    if let Some(id) = text.strip_prefix("unit_ability:") {
        return UNIT_ABILITIES
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, cause)| DestroyReason::UnitAbility(*cause));
    }
    if let Some(id) = text.strip_prefix("leader_") {
        return Leader::from_id(id).map(DestroyReason::LeaderUnderscore);
    }
    Leader::from_id(text).map(DestroyReason::LeaderBare)
}

/// The text of a destruction cause.
#[must_use]
pub fn render_destroy(reason: DestroyReason) -> String {
    match reason {
        DestroyReason::ActionCard(c) => format!(
            "action_card:{}",
            ACTION_CARD_CAUSES
                .iter()
                .find(|(_, x)| *x == c)
                .map_or("", |(n, _)| n)
        ),
        DestroyReason::Agenda(x) => format!("agenda:{}", x.as_str()),
        DestroyReason::FactionAbility(c) => format!(
            "faction_ability:{}",
            FACTION_ABILITIES
                .iter()
                .find(|(_, x)| *x == c)
                .map_or("", |(n, _)| n)
        ),
        DestroyReason::Leader(x) => format!("leader:{}", x.as_str()),
        DestroyReason::LeaderUnderscore(x) => format!("leader_{}", x.as_str()),
        DestroyReason::LeaderBare(x) => x.as_str().to_owned(),
        DestroyReason::UnitAbility(c) => format!(
            "unit_ability:{}",
            UNIT_ABILITIES
                .iter()
                .find(|(_, x)| *x == c)
                .map_or("", |(n, _)| n)
        ),
        word => DESTROY_WORDS
            .iter()
            .find(|(_, r)| *r == word)
            .map(|(text, _)| (*text).to_owned())
            .unwrap_or_default(),
    }
}

const DECK_TEXT: [&str; ExploreDeck::COUNT] = ["CULTURAL", "FRONTIER", "HAZARDOUS", "INDUSTRIAL"];

/// The exploration deck with this key text.
#[must_use]
pub fn parse_explore_deck(text: &str) -> Option<ExploreDeck> {
    DECK_TEXT
        .iter()
        .position(|t| *t == text)
        .map(|i| ExploreDeck::ALL[i])
}

/// The key text of an exploration deck.
#[must_use]
pub fn render_explore_deck(deck: ExploreDeck) -> &'static str {
    DECK_TEXT[deck.index()]
}

const EXPEDITION_TEXT: [&str; Expedition::COUNT] = [
    "action_cards",
    "influence",
    "resources",
    "secret",
    "tech_planet",
    "trade_goods",
];

/// The expedition slice with this key text.
#[must_use]
pub fn parse_expedition(text: &str) -> Option<Expedition> {
    EXPEDITION_TEXT
        .iter()
        .position(|t| *t == text)
        .map(|i| Expedition::ALL[i])
}

/// The key text of an expedition slice.
#[must_use]
pub fn render_expedition(slice: Expedition) -> &'static str {
    EXPEDITION_TEXT[slice.index()]
}
