//! Decision types of the `strategy` area: strategy phase, strategy cards, status phase, draft.
//!
//! Owner: package B8 (docs/ENGINE_CORE_STATE.md 4.4). The adapter side is
//! `ti4-engine/src/render/strategy.rs`. Every type stays `Copy`; what the text shows of the position
//! (pool counts, planet worth) is completed by the adapter from the state, never carried here.

use super::agenda::Shown;
use super::generic::Rule;
use super::{Ask as AnyAsk, Choice, Ctx, Opt as AnyOpt, Opts, Origin, Subtype as AnySubtype};
use crate::containers::SmallList;
use crate::generated::{Agenda, Faction, Planet, PublicObjective, StrategyCard, System, Technology};
use crate::ids::Seat;
use crate::state::Code;

/// A waiver a faction module offers (with a secondary, or on a research; `hooks_strategy::SecondaryWaiver` and `ResearchWaiverPayment`,
/// `{id, label}`): the module and its code; the label is the adapter's. (Re-exported by `kernel::hooks`.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Waiver {
    pub module: Faction,
    pub code: u16,
}

/// A command token pool (`TokenPool`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pool {
    Tactic,
    Fleet,
    Strategic,
}

/// Where Politics puts the two agendas it looked at (`first` the one that was on top; [`Ask::PoliticsAgendas`]): the six orders 66.2
/// allows, in the order [`Stack::ALL`] offers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stack {
    /// One on top, the other on the bottom (`top_first`: the first one goes on top).
    Split { top_first: bool },
    /// Both on top (`first_up`: the first one is the new top card, the second below it).
    Top { first_up: bool },
    /// Both on the bottom (`first_up`: the first one lies above the second, which is the new bottom card).
    Bottom { first_up: bool },
}

impl Stack {
    /// The six orders, as the question offers them.
    pub const ALL: [Self; 6] = [
        Self::Split { top_first: true },
        Self::Split { top_first: false },
        Self::Top { first_up: true },
        Self::Top { first_up: false },
        Self::Bottom { first_up: true },
        Self::Bottom { first_up: false },
    ];

    /// The agendas that go on top (the first of the list is the new top card) and on the bottom (the last of the list is the new bottom
    /// card), for the two agendas `first` and `second`.
    #[must_use]
    pub fn piles(self, first: Agenda, second: Agenda) -> (SmallList<Agenda, 2>, SmallList<Agenda, 2>) {
        let list = |v: &[Agenda]| v.iter().copied().collect::<SmallList<Agenda, 2>>();
        match self {
            Self::Split { top_first: true } => (list(&[first]), list(&[second])),
            Self::Split { top_first: false } => (list(&[second]), list(&[first])),
            Self::Top { first_up: true } => (list(&[first, second]), list(&[])),
            Self::Top { first_up: false } => (list(&[second, first]), list(&[])),
            Self::Bottom { first_up: true } => (list(&[]), list(&[first, second])),
            Self::Bottom { first_up: false } => (list(&[]), list(&[second, first])),
        }
    }
}

/// What the area asks. Parameters only; the option list is derived by the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Trade primary: "let another player replenish commodities" (one seat per question, until none is
    /// left that can or the holder is done).
    TradeReplenish,
    /// 52.4: "gain `left` command tokens into which pools": one question for the whole gain, an [`Opt::Gain`] per split.
    GainPool { left: u8 },
    /// Warfare primary: "recall a command token" from one of the systems that holds one.
    WarfareRecall,
    /// Warfare: the final split of the `total` command tokens on the sheet, at least `floor` in the fleet
    /// pool.
    WarfareRedistribute { total: u8, floor: u8 },
    /// 81.5: the final split of the `total` command tokens on the sheet at the end of the status phase's token gain, at least
    /// `floor` in the fleet pool (`strategy_cards::redistribute_tokens`, subtype `status_redistribute_tokens`).
    StatusRedistribute { total: u8, floor: u8 },
    /// Diplomacy primary: the system whose other players get a command token.
    DiplomacySystem,
    /// 34.2: "ready which planet" (Diplomacy primary and secondary).
    ReadyPlanet,
    /// Politics primary: "who becomes speaker".
    PoliticsSpeaker,
    /// Politics primary: "place {agenda} where" (only one agenda was left to look at).
    PoliticsAgenda { agenda: Agenda },
    /// Politics primary (66.2): "place each of the two agendas looked at on the top or the bottom of the deck, in any order": ONE question
    /// over the six final orders ([`Stack`]). `first` was the top card of the deck, `second` the one below it.
    PoliticsAgendas { first: Agenda, second: Agenda },
    /// Construction: "place a structure" (`step` of 2 for the primary's two placements, 0 for the secondary's
    /// one; `only_pds`: the placement may only be a PDS).
    PlaceStructure { step: u8, only_pds: bool },
    /// Construction, a PDS picked on a planet where a faction offers something else instead (Titans' Hecatoncheires): "place a PDS or an
    /// alternative" (no decline; the answers are the PDS or the alternative).
    PdsAlternative { system: System, planet: Planet },
    /// Imperial primary: "score a public objective with Imperial".
    ImperialScore,
    /// Xxcha's Peace Accords after Diplomacy: "annex a planet" (`controlled`: the planets the seat controls now, for the preview of the gain).
    PeaceAccords { controlled: u8 },
    /// Galactic Crisis Pact: a player's question on the secondary of the elected `card`, without the token (the follower's question of the card).
    CrisisPact { card: StrategyCard },
    /// Thunder's Edge Construction: "a structure or a production" (the first of its two abilities).
    ConstructionAbility,
    /// Thunder's Edge Warfare: the final split of the `total` command tokens before the free tactical action, at least
    /// `floor` in the fleet pool.
    TeWarfareRedistribute { total: u8, floor: u8 },
    /// Thunder's Edge Warfare: the same split after the free tactical action has ended.
    TeWarfareRedistributeAfter { total: u8, floor: u8 },
    /// Thunder's Edge Warfare: the system of the tactical action without a command token.
    TeWarfareTactical,
    /// "research a technology": the bill in resources (the payload `cost`), the technologies the seat owns
    /// (the preview), and whether the holder may decline.
    ResearchTechnology { cost: u8, owned: u8, decline: bool },
    /// Doctor Sucaban's offer in a paid research (`strategy_cards::doctor_sucaban`), asked to the holder of the agent (`source`'s agent, or the
    /// holder's own Ssruu when `copied`): "exhaust to let {researcher} trade infantry for research".
    SucabanOffer { researcher: Seat, source: Seat, copied: bool },
    /// Doctor Sucaban's trade (`strategy_cards::trade_infantry_for_research`), asked to the researcher: "remove an infantry to reduce the cost by 1".
    SucabanTrade,
    /// 26: "choose a strategy card" at the draft of the strategy phase.
    Draft,
    /// Imperial Arbiter (`Game::imperial_arbiter`), asked to the owner after the offer to discard it: "give away which of your strategy cards".
    ArbiterGive,
    /// Imperial Arbiter: "and take which" card of another seat.
    ArbiterTake,
    /// 82: a follower's question on the secondary of `card`, played by `primary` (every kind; Leadership's is the
    /// purchase question itself). `costs_token`: the follow takes a strategy token.
    Follow { card: StrategyCard, primary: Seat, costs_token: bool },
    /// `strategy_cards::resolve_research`: "research {tech}: choose a prerequisite waiver", the waivers the modules offer (in the string's
    /// order: the index of an answer `waiver|{index}` is the place in this list; two modules at most).
    ResearchWaiver { tech: Technology, offered: [Option<Waiver>; 2] },
    /// `strategy_cards::resolve_research`: "{waiver label}: choose the payment" of the waiver chosen.
    ResearchWaiverPayment { tech: Technology, waiver: Waiver },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Trade primary: this seat replenishes (the end of the list is the generic `Done`).
    Replenish(Seat),
    /// A token gain (52.4) answered at once: how many of the `left` tokens go into each pool (the three sum to `left`). The gained
    /// tokens are placed in the canonical order tactic, fleet, strategic ([`gain_pools`]).
    Gain { tactic: u8, fleet: u8, strategic: u8 },
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
    /// Where the two agendas of a [`Ask::PoliticsAgendas`] go.
    Stack(Stack),
    /// Construction: this structure on this planet of this system.
    Structure { pds: bool, system: System, planet: Planet },
    /// Construction: the PDS as planned (the answer `pds`).
    PlacePds,
    /// Construction: the faction's alternative to the PDS, Hecatoncheires (the answer `titans|hecatoncheires`).
    Alternative,
    /// Imperial: score this public objective.
    Objective(PublicObjective),
    /// Imperial: score this secret objective a leak moved into the public area (the answer is its id, as for a public one).
    LeakedSecret(crate::generated::SecretObjective),
    /// Peace Accords: gain control of this planet of this system (the answer is the planet's id).
    Annex(System, Planet),
    /// Thunder's Edge Construction: place a structure (the answer `structure`).
    Build,
    /// Thunder's Edge Construction: use PRODUCTION in this system (the answer `produce|<system>`).
    Produce(System),
    /// Research this technology.
    Research(Technology),
    /// Doctor Sucaban's offer is taken: exhaust the agent (the answer `yes`).
    Exhaust,
    /// Doctor Sucaban's trade: remove an infantry of the researcher from this space area (`planet` is `None`) or planet (the answer `system:planet`).
    Infantry { system: System, planet: Option<Planet> },
    /// The draft takes this card off the mat; Imperial Arbiter's owner gives this card away.
    Card(StrategyCard),
    /// Imperial Arbiter's owner takes this card from this seat (the answer `{holder}|{card}`).
    Take(Seat, StrategyCard),
    /// A follower does not use the secondary (`no`).
    Skip,
    /// A follower uses the secondary (`yes`): spends the strategy token when the question says so.
    Follow,
    /// A token-free way to follow offered by a faction module (`follow|waived|<module>|<waiver>`): the option is the
    /// string module's own (its label is the module's text), interned as an option shape. THE DUAL'S form (the string's `follow|waived|..`
    /// option as it was offered); [`Opt::WaiveBy`] is the live loop's form of the same option, the machine treats them alike.
    Waive(Code),
    /// A token-free way to follow offered by a faction module of the core (a hook of the kernel: `Waiver { module, code }`): the module and its code.
    /// THE LIVE LOOP's form of [`Opt::Waive`] (the renderer writes the string's id and label from the module and the code).
    WaiveBy(crate::generated::Faction, u16),
    /// Enervate: the follower performs the primary ability instead (`follow|enervate_primary`).
    PrimaryInstead,
    /// A prerequisite waiver of a faction module, the place in the offered list (the answer `waiver|{index}`).
    ResearchWaiver(u8),
    /// The payment of a research waiver, the module's own code (the Cabal's captured model, Yin's infantry; the answer is the module's id).
    ResearchPayment(Waiver),
}

impl Opt {
    /// The [`Opt::Gain`] that puts one token into each of `pools` (repeats count).
    #[must_use]
    pub fn gain_of(pools: &[Pool]) -> Self {
        let n = |p: Pool| u8::try_from(pools.iter().filter(|q| **q == p).count()).unwrap_or(u8::MAX);
        Self::Gain { tactic: n(Pool::Tactic), fleet: n(Pool::Fleet), strategic: n(Pool::Strategic) }
    }
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    TradeReplenish,
    GainCommandToken,
    WarfareRecallToken,
    WarfareRedistributeTokens,
    /// `status_redistribute_tokens` (81.5).
    StatusRedistributeTokens,
    DiplomacyChooseSystem,
    ReadyPlanet,
    PoliticsChooseSpeaker,
    PoliticsPlaceAgenda,
    BuyTokenWithInfluence,
    PlaceStructure,
    /// `place_structure_pds_alternative`.
    PlaceStructurePdsAlternative,
    ImperialScoreObjective,
    /// `peace_accords_annex`.
    PeaceAccordsAnnex,
    ResearchTechnology,
    /// `doctor_sucaban_exhaust`.
    SucabanExhaust,
    /// `doctor_sucaban_borrowed_exhaust`.
    SucabanBorrowedExhaust,
    /// `doctor_sucaban_remove_infantry`.
    SucabanRemoveInfantry,
    /// A follower's question (Leadership's is `BuyTokenWithInfluence`).
    FollowSecondary,
    ConstructionChooseAbility,
    WarfareFreeTactical,
    /// `draft_strategy_card`.
    Draft,
    /// The two questions of Imperial Arbiter's swap (the string asks them without a context; the live layer drops it).
    ArbiterGive,
    ArbiterTake,
    /// `research_waiver`.
    ResearchWaiver,
    /// `research_waiver_payment`.
    ResearchWaiverPayment,
}

/// The context of a strategy card question.
#[must_use]
pub fn ctx(card: StrategyCard, secondary: bool, subtype: Subtype) -> Ctx {
    ctx_from(Origin::StrategyCard { card, secondary }, subtype)
}

/// The context of a question a leader's ability raises inside a strategy card (`DecisionSource::Content` of the leader).
#[must_use]
pub fn leader_ctx(leader: crate::generated::Leader, subtype: Subtype) -> Ctx {
    ctx_from(Origin::Content(super::generic::Content::Leader(leader)), subtype)
}

/// The context of a question a rule raises inside a strategy card (`DecisionSource::Rule`).
#[must_use]
pub fn rule_ctx(rule: Rule, subtype: Subtype) -> Ctx {
    ctx_from(Origin::Rule(rule), subtype)
}

/// The context of the research window's waiver questions (`DecisionSource::FactionAbility("research_waiver")`).
#[must_use]
pub fn waiver_ctx(subtype: Subtype) -> Ctx {
    ctx_from(Origin::FactionAbility(super::generic::Content::ResearchWaiver), subtype)
}

/// The context of a question a faction ability raises (`DecisionSource::FactionAbility`).
#[must_use]
pub fn ability_ctx(ability: crate::generated::Ability, subtype: Subtype) -> Ctx {
    ctx_from(Origin::FactionAbility(super::generic::Content::Ability(ability)), subtype)
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

/// Every way to put `left` gained tokens into the three pools, the most tactic tokens first (for one token: tactic, fleet, strategic).
pub fn gains(left: u8) -> impl Iterator<Item = Opt> {
    (0..=left).rev().flat_map(move |tactic| (0..=(left - tactic)).rev().map(move |fleet| Opt::Gain { tactic, fleet, strategic: left - tactic - fleet }))
}

/// The pool question of a token gain (`strategy_cards::gain_tokens`): ONE question for all `left` tokens, an option per split ([`gains`]).
#[must_use]
pub fn gain_pool_choice(who: Seat, left: u8) -> Choice {
    Choice {
        actor: who,
        ask: AnyAsk::Strategy(Ask::GainPool { left }),
        ctx: rule_ctx(Rule::R52_4, Subtype::GainCommandToken),
        opts: gains(left).map(AnyOpt::Strategy).collect::<Opts>(),
    }
}

/// The answer to a [`gain_pool_choice`] of `left` tokens: the split, when `pick` is one that sums to `left`.
#[must_use]
pub fn gain_answer(pick: super::Pick, left: u8) -> Option<[u8; 3]> {
    match pick {
        super::Pick::Opt(AnyOpt::Strategy(Opt::Gain { tactic, fleet, strategic }))
            if u16::from(tactic) + u16::from(fleet) + u16::from(strategic) == u16::from(left) =>
        {
            Some([tactic, fleet, strategic])
        }
        _ => None,
    }
}

/// The pools of a gain split, one per token, in the canonical order (tactic, fleet, strategic).
pub fn gain_pools(split: [u8; 3]) -> impl Iterator<Item = Pool> {
    [Pool::Tactic, Pool::Fleet, Pool::Strategic].into_iter().zip(split).flat_map(|(pool, n)| std::iter::repeat_n(pool, usize::from(n)))
}
