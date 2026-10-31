//! Decision types of the `strategy` area: strategy phase, strategy cards, status phase, draft.
//!
//! Owner: package B8 (docs/ENGINE_CORE_STATE.md 4.4). The adapter side is
//! `ti4-engine/src/render/strategy.rs`. Every type stays `Copy`; what the text shows of the position
//! (pool counts, planet worth) is completed by the adapter from the state, never carried here.

use super::agenda::Shown;
use super::generic::Rule;
use super::{Ask as AnyAsk, Choice, Ctx, Opt as AnyOpt, Opts, Origin, Subtype as AnySubtype};
use crate::containers::SmallList;
use crate::generated::{Agenda, Planet, PublicObjective, StrategyCard, System, Technology};
use crate::ids::Seat;
use crate::state::Code;

/// A command token pool (`TokenPool`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pool {
    Tactic,
    Fleet,
    Strategic,
}

/// What the area asks. Parameters only; the option list is derived by the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Trade primary: "let another player replenish commodities" (one seat per question, until none is
    /// left that can or the holder is done).
    TradeReplenish,
    /// 52.4: "gain a command token into which pool" (`left` tokens still to place, this one included).
    GainPool { left: u8 },
    /// Warfare primary: "recall a command token" from one of the systems that holds one.
    WarfareRecall,
    /// Warfare: the final split of the `total` command tokens on the sheet, at least `floor` in the fleet
    /// pool.
    WarfareRedistribute { total: u8, floor: u8 },
    /// Diplomacy primary: the system whose other players get a command token.
    DiplomacySystem,
    /// 34.2: "ready which planet" (Diplomacy primary and secondary).
    ReadyPlanet,
    /// Politics primary: "who becomes speaker".
    PoliticsSpeaker,
    /// Politics primary: "place {agenda} where".
    PoliticsAgenda { agenda: Agenda },
    /// 52.3: "spend {owed} influence for a command token" (`more`: a carried credit pays part of it, so the
    /// text says "more").
    LeadershipBuy { owed: u8, more: bool },
    /// Construction: "place a structure" (`step` of 2 for the primary's two placements, 0 for the secondary's
    /// one; `only_pds`: the placement may only be a PDS).
    PlaceStructure { step: u8, only_pds: bool },
    /// Construction, a PDS picked on a planet where a faction offers something else instead (Titans' Hecatoncheires): "place a PDS or an
    /// alternative" (no decline; the answers are the PDS or the alternative).
    PdsAlternative { system: System, planet: Planet },
    /// Imperial primary: "score a public objective with Imperial".
    ImperialScore,
    /// Thunder's Edge Construction: "a structure or a production" (the first of its two abilities).
    ConstructionAbility,
    /// Thunder's Edge Warfare: the final split of the `total` command tokens before the free tactical action, at least
    /// `floor` in the fleet pool.
    TeWarfareRedistribute { total: u8, floor: u8 },
    /// Thunder's Edge Warfare: the system of the tactical action without a command token.
    TeWarfareTactical,
    /// "research a technology": the bill in resources (the payload `cost`), the technologies the seat owns
    /// (the preview), and whether the holder may decline.
    ResearchTechnology { cost: u8, owned: u8, decline: bool },
    /// 26: "choose a strategy card" at the draft of the strategy phase.
    Draft,
    /// 82: a follower's question on the secondary of `card`, played by `primary` (every kind; Leadership's is the
    /// purchase question itself). `costs_token`: the follow takes a strategy token.
    Follow { card: StrategyCard, primary: Seat, costs_token: bool },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Trade primary: this seat replenishes (the end of the list is the generic `Done`).
    Replenish(Seat),
    /// Gain the token into this pool.
    Pool(Pool),
    /// A system (Warfare's recall, Diplomacy's choice).
    System(System),
    /// Ready this planet; `Shown.system` is the tile the map shows it on.
    Ready(Planet, Shown),
    /// This seat becomes speaker.
    Speaker(Seat),
    /// The tokens end up split this way across the pools.
    Split { tactic: u8, fleet: u8, strategic: u8 },
    /// An agenda goes on top of the deck.
    Top,
    /// An agenda goes on the bottom of the deck.
    Bottom,
    /// Leadership: spend the influence (`true`) or nothing further (`false`).
    Buy(bool),
    /// Construction: this structure on this planet of this system.
    Structure { pds: bool, system: System, planet: Planet },
    /// Construction: the PDS as planned (the answer `pds`).
    PlacePds,
    /// Construction: the faction's alternative to the PDS, Hecatoncheires (the answer `titans|hecatoncheires`).
    Alternative,
    /// Imperial: score this public objective.
    Objective(PublicObjective),
    /// Thunder's Edge Construction: place a structure (the answer `structure`).
    Build,
    /// Thunder's Edge Construction: use PRODUCTION in this system (the answer `produce|<system>`).
    Produce(System),
    /// Research this technology.
    Research(Technology),
    /// The draft takes this card off the mat.
    Card(StrategyCard),
    /// A follower does not use the secondary (`no`).
    Skip,
    /// A follower uses the secondary (`yes`): spends the strategy token when the question says so.
    Follow,
    /// A token-free way to follow offered by a faction module (`follow|waived|<module>|<waiver>`): the option is the
    /// string module's own (its label is the module's text), interned as an option shape.
    Waive(Code),
    /// Enervate: the follower performs the primary ability instead (`follow|enervate_primary`).
    PrimaryInstead,
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    TradeReplenish,
    GainCommandToken,
    WarfareRecallToken,
    WarfareRedistributeTokens,
    DiplomacyChooseSystem,
    ReadyPlanet,
    PoliticsChooseSpeaker,
    PoliticsPlaceAgenda,
    BuyTokenWithInfluence,
    PlaceStructure,
    /// `place_structure_pds_alternative`.
    PlaceStructurePdsAlternative,
    ImperialScoreObjective,
    ResearchTechnology,
    /// A follower's question (Leadership's is `BuyTokenWithInfluence`).
    FollowSecondary,
    ConstructionChooseAbility,
    WarfareFreeTactical,
    /// `draft_strategy_card`.
    Draft,
}

/// The context of a strategy card question.
#[must_use]
pub fn ctx(card: StrategyCard, secondary: bool, subtype: Subtype) -> Ctx {
    ctx_from(Origin::StrategyCard { card, secondary }, subtype)
}

/// The context of a question a rule raises inside a strategy card (`DecisionSource::Rule`).
#[must_use]
pub fn rule_ctx(rule: Rule, subtype: Subtype) -> Ctx {
    ctx_from(Origin::Rule(rule), subtype)
}

fn ctx_from(origin: Origin, subtype: Subtype) -> Ctx {
    Ctx {
        origin,
        subtype: AnySubtype::Strategy(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

/// The pool question of a token gain (`strategy_cards::gain_tokens`), `left` tokens still to place.
#[must_use]
pub fn gain_pool_choice(who: Seat, left: u8) -> Choice {
    Choice {
        actor: who,
        ask: AnyAsk::Strategy(Ask::GainPool { left }),
        ctx: rule_ctx(Rule::R52_4, Subtype::GainCommandToken),
        opts: [Pool::Tactic, Pool::Fleet, Pool::Strategic].into_iter().map(|p| AnyOpt::Strategy(Opt::Pool(p))).collect::<Opts>(),
    }
}
