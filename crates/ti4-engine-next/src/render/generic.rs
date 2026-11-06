//! The generic families (decline, done, number), the generic asks, and the codec of the decision
//! context (`Ctx` <-> `DecisionContext`). Owner: F4.
//!
//! Ids: `decline` and `done` are literal; a number is its canonical decimal text (no sign for
//! non-negative values, no leading zeros, no `+`, no `-0`), so `parse(option(o).id) == o`.

use std::convert::Infallible;

use serde_json::Value;
use ti4_engine_core::choice::generic::{Ask as GAsk, Content, Rule, Subtype as GSubtype};
use ti4_engine_core::choice::{Ctx, Opt, Origin, Owed, OwedKind, Subtype, Target};
use ti4_engine_core::generated::{Agenda, ActionCard, Planet, StrategyCard, System, Unit};
use ti4_engine_core::state::Phase as CorePhase;
use ti4_engine_core::{Seat, SmallList};
use ti4_model::id::{PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::Phase;

use super::{Family, Render, Wording};
use crate::wire::choice::{ChoiceOption, DECLINE_ID, DECLINE_KIND};
use crate::wire::decision_context::{
    ConstraintKind, DecisionContext, DecisionSource, DecisionTarget, OutstandingConstraint, CONTEXT_VERSION,
};
use ti4_engine_core::generated as cg;

use crate::generated::{self as g, IdStr};

pub const DONE_ID: &str = "done";
pub const COUNT_KIND: &str = "count";

/// The renderer of the asks that belong to no area.
pub struct Fam;

impl Family for Fam {
    type Ask = GAsk;
    type Opt = Infallible;
    type Subtype = GSubtype;

    fn prompt(_: &Render, a: &GAsk) -> String {
        match a {
            GAsk::Confirm => "Continue?".to_owned(),
            GAsk::Count { .. } => "Choose a number".to_owned(),
            // A random event of the core (lane C3). The chance resolver answers it before any table sees it.
            GAsk::Chance(_) => "Roll".to_owned(),
        }
    }

    fn subtype(_: &Render, s: &GSubtype) -> String {
        match s {
            GSubtype::Unclassified => "unclassified".to_owned(),
            GSubtype::Chance => "chance".to_owned(),
        }
    }

    fn option(_: &Render, _: &GAsk, o: &Infallible) -> ChoiceOption {
        match *o {}
    }

    fn parse(_: &Render, _: &GAsk, _: &str) -> Option<Infallible> {
        None
    }

    fn numbers(a: &GAsk) -> bool {
        matches!(a, GAsk::Count { .. })
    }
}

/// The text of a generic option: `w` overrides the defaults.
///
/// # Panics
/// On an option that is not generic.
#[must_use]
pub fn option(o: &Opt, w: &Wording) -> ChoiceOption {
    let (id, kind, label) = match o {
        Opt::Decline => (DECLINE_ID.to_owned(), DECLINE_KIND, "Decline".to_owned()),
        Opt::Done => (DONE_ID.to_owned(), DECLINE_KIND, "done".to_owned()),
        Opt::Number(n) => (n.to_string(), COUNT_KIND, n.to_string()),
        other => panic!("not a generic option: {other:?}"),
    };
    let mut out = ChoiceOption::labelled(id, w.kind.as_deref().unwrap_or(kind), w.label.clone().unwrap_or(label));
    out.payload = w.payload.clone();
    out.preview = w.preview.clone();
    out
}

/// The generic option an id names: `decline`, `done`, and (when the ask takes numbers) a canonical
/// decimal integer.
#[must_use]
pub fn parse(id: &str, numbers: bool) -> Option<Opt> {
    match id {
        DECLINE_ID => Some(Opt::Decline),
        DONE_ID => Some(Opt::Done),
        _ if numbers => {
            let n: i32 = id.parse().ok()?;
            (n.to_string() == id).then_some(Opt::Number(n))
        }
        _ => None,
    }
}

// ---- context -------------------------------------------------------------------------------------

const RULES: [(Rule, &str); 32] = [
    (Rule::R2_4, "2.4"),
    (Rule::R8, "8"),
    (Rule::R8_10, "8.10"),
    (Rule::R8_11, "8.11"),
    (Rule::R8_16, "8.16"),
    (Rule::R8_18, "8.18"),
    (Rule::R22, "22"),
    (Rule::R22_1, "22.1"),
    (Rule::R26, "26"),
    (Rule::R27_2, "27.2"),
    (Rule::R34_2, "34.2"),
    (Rule::R34_3_75_2_75_3, "34.3/75.2/75.3"),
    (Rule::R42, "42"),
    (Rule::R45_4, "45.4"),
    (Rule::R49, "49"),
    (Rule::R52_3, "52.3"),
    (Rule::R52_4, "52.4"),
    (Rule::R60, "60"),
    (Rule::R94, "94"),
    (Rule::R61_6, "61.6"),
    (Rule::R68, "68"),
    (Rule::R78_4, "78.4"),
    (Rule::R78_7, "78.7"),
    (Rule::R78_9, "78.9"),
    (Rule::R81_5, "81.5"),
    (Rule::R82, "82"),
    (Rule::R89_1, "89.1"),
    (Rule::R89_2, "89.2"),
    (Rule::R95, "95"),
    (Rule::Coexistence, "Coexistence"),
    (Rule::Coexistence7_2, "Coexistence 7.2"),
    (Rule::EndOfTurn, "end of turn"),
];

fn rule_text(r: &Render, rule: Rule) -> String {
    match rule {
        Rule::Other(c) => r.code_text(c),
        _ => RULES.iter().find(|(x, _)| *x == rule).map(|(_, t)| (*t).to_owned()).expect("listed rule"),
    }
}

fn rule_of(r: &Render, text: &str) -> Rule {
    RULES.iter().find(|(_, t)| *t == text).map_or_else(|| Rule::Other(r.code(text)), |(x, _)| *x)
}

fn id_of<T: IdStr>(x: T) -> String {
    x.as_str().to_owned()
}

/// Which table an alias came from (first match in this order).
pub fn content_of(r: &Render, alias: &str) -> Content {
    macro_rules! try_tables {
        ($($ty:ty => $v:ident),*) => {$(
            if let Some(x) = <$ty as IdStr>::from_id(alias) { return Content::$v(x); }
        )*};
    }
    try_tables!(
        cg::Ability => Ability, cg::Leader => Leader, cg::Relic => Relic,
        cg::Explore => Explore, cg::Breakthrough => Breakthrough,
        cg::PromissoryNote => Promissory, cg::Technology => Technology,
        cg::GenericCard => GenericCard, cg::Attachment => Attachment,
        cg::ActionCard => ActionCard
    );
    if let Some(step) = super::explore::step_of(alias) {
        return Content::Step(step);
    }
    if alias == RESEARCH_WAIVER {
        return Content::ResearchWaiver;
    }
    Content::Other(r.code(alias))
}

fn content_text(r: &Render, c: Content) -> String {
    match c {
        Content::Ability(x) => id_of(x),
        Content::Leader(x) => id_of(x),
        Content::Relic(x) => id_of(x),
        Content::Explore(x) => id_of(x),
        Content::Breakthrough(x) => id_of(x),
        Content::Promissory(x) => id_of(x),
        Content::Technology(x) => id_of(x),
        Content::GenericCard(x) => id_of(x),
        Content::Attachment(x) => id_of(x),
        Content::ActionCard(x) => id_of(x),
        Content::Step(s) => super::explore::step_text(s).to_owned(),
        Content::Other(c) => r.code_text(c),
        Content::ResearchWaiver => RESEARCH_WAIVER.to_owned(),
    }
}

/// The source text of the research window's waiver questions (`strategy_cards::resolve_research`).
const RESEARCH_WAIVER: &str = "research_waiver";

fn card_of(name: &str) -> Option<StrategyCard> {
    (0..StrategyCard::COUNT)
        .filter_map(StrategyCard::from_index)
        .find(|c| g::strategy_cards::text(*c).name == name)
}

fn origin_text(r: &Render, o: &Origin) -> DecisionSource {
    match o {
        Origin::Rule(x) => DecisionSource::Rule(rule_text(r, *x)),
        Origin::StrategyCard { card, secondary } => {
            DecisionSource::StrategyCard { card: g::strategy_cards::text(*card).name.to_owned(), secondary: *secondary }
        }
        Origin::ActionCard(x) => DecisionSource::ActionCard(id_of(*x)),
        Origin::FactionAbility(c) => DecisionSource::FactionAbility(content_text(r, *c)),
        Origin::Content(c) => DecisionSource::Content(content_text(r, *c)),
        Origin::Agenda(x) => DecisionSource::Agenda(id_of(*x)),
        Origin::Reaction(k) => DecisionSource::Reaction(crate::kernel_sync::event_text(r, *k)),
    }
}

fn origin_of(r: &Render, s: &DecisionSource) -> Option<Origin> {
    Some(match s {
        DecisionSource::Rule(t) => Origin::Rule(rule_of(r, t)),
        DecisionSource::StrategyCard { card, secondary } => Origin::StrategyCard { card: card_of(card)?, secondary: *secondary },
        DecisionSource::ActionCard(t) => Origin::ActionCard(ActionCard::from_id(t)?),
        DecisionSource::FactionAbility(t) => Origin::FactionAbility(content_of(r, t)),
        DecisionSource::Content(t) => Origin::Content(content_of(r, t)),
        DecisionSource::Agenda(t) => Origin::Agenda(Agenda::from_id(t)?),
        DecisionSource::Reaction(t) => Origin::Reaction(crate::kernel_sync::event_kind(r, t)),
    })
}

fn target_text(r: &Render, t: &Target) -> DecisionTarget {
    match t {
        Target::System(s) => DecisionTarget::System(SystemId::new(s.as_str())),
        Target::Planet { system, planet } => {
            DecisionTarget::Planet { system: SystemId::new(system.as_str()), planet: PlanetId::new(planet.as_str()) }
        }
        Target::Unit { system, unit } => {
            DecisionTarget::Unit { system: SystemId::new(system.as_str()), unit: UnitTypeId::new(unit.as_str()) }
        }
        Target::Seat(s) => DecisionTarget::Player(PlayerId::new(r.seat_text(*s))),
    }
}

fn target_of(r: &Render, t: &DecisionTarget) -> Option<Target> {
    Some(match t {
        DecisionTarget::System(s) => Target::System(System::from_id(s.as_str())?),
        DecisionTarget::Planet { system, planet } => {
            Target::Planet { system: System::from_id(system.as_str())?, planet: Planet::from_id(planet.as_str())? }
        }
        DecisionTarget::Unit { system, unit } => {
            Target::Unit { system: System::from_id(system.as_str())?, unit: Unit::from_id(unit.as_str())? }
        }
        DecisionTarget::Player(p) => Target::Seat(r.seat(p.as_str())?),
    })
}

const OWED: [(OwedKind, ConstraintKind); 9] = [
    (OwedKind::Resources, ConstraintKind::Resources),
    (OwedKind::Influence, ConstraintKind::Influence),
    (OwedKind::TradeGoods, ConstraintKind::TradeGoods),
    (OwedKind::CommandTokens, ConstraintKind::CommandTokens),
    (OwedKind::ProductionCapacity, ConstraintKind::ProductionCapacity),
    (OwedKind::TransportCapacity, ConstraintKind::TransportCapacity),
    (OwedKind::FleetSupply, ConstraintKind::FleetSupply),
    (OwedKind::UnitsToRemove, ConstraintKind::UnitsToRemove),
    (OwedKind::Votes, ConstraintKind::Votes),
];

/// The string `DecisionContext` of a typed one. `subtype` is the text of `ctx.subtype` (the registry
/// supplies it; the corpus harness passes the recorded text). The display-only parts (trigger, hit,
/// previews) are left empty: the area's renderer fills them.
#[must_use]
pub fn encode_context(r: &Render, ctx: &Ctx, actor: Seat, phase: Phase, round: u32, subtype: String) -> DecisionContext {
    let mut out = DecisionContext::new(r.player(actor), origin_text(r, &ctx.origin), subtype, phase, round);
    out.version = CONTEXT_VERSION;
    out.optional = ctx.optional;
    out.target = ctx.target.as_ref().map(|t| target_text(r, t));
    out.space_battle = ctx.space_battle;
    out.invasion_seq = ctx.invasion_seq;
    out.outstanding = ctx
        .outstanding
        .iter()
        .map(|o| {
            let kind = OWED.iter().find(|(k, _)| *k == o.kind).expect("listed kind").1;
            OutstandingConstraint::new(kind, o.amount, o.paid)
        })
        .collect();
    out
}

/// A recorded context as typed values: everything except the subtype text, which only the owning area
/// can read (the typed subtype is [`GSubtype::Unclassified`]). `None` when a text names something the
/// tables do not know.
#[must_use]
pub fn decode_context(r: &Render, c: &DecisionContext) -> Option<(Ctx, Seat, Phase, u32)> {
    let mut outstanding = SmallList::new();
    for o in &c.outstanding {
        let kind = OWED.iter().find(|(_, k)| *k == o.kind)?.0;
        outstanding.push(Owed { kind, amount: o.amount, paid: o.paid });
    }
    let ctx = Ctx {
        origin: origin_of(r, &c.source)?,
        subtype: Subtype::Generic(GSubtype::Unclassified),
        optional: c.optional,
        target: match &c.target {
            Some(t) => Some(target_of(r, t)?),
            None => None,
        },
        outstanding,
        space_battle: c.space_battle,
        invasion_seq: c.invasion_seq,
    };
    Some((ctx, r.seat(c.actor.as_str())?, c.phase, c.round))
}

/// Compile-time reminder that core phases and wire phases agree.
#[must_use]
pub const fn core_phase(p: Phase) -> CorePhase {
    match p {
        Phase::Strategy => CorePhase::Strategy,
        Phase::Action => CorePhase::Action,
        Phase::Status => CorePhase::Status,
        Phase::Agenda => CorePhase::Agenda,
    }
}

/// `Value` of a number as the string engine writes payload counts.
#[must_use]
pub fn count_value(n: i32) -> Value {
    Value::from(n)
}
