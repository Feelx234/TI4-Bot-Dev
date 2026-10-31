//! The trade island's share of the live loop (package M1): the free-form deal, the one thing of a transaction the core cannot read
//! from its own tables because it arrives as TEXT.
//!
//! A free-form deal is named by the proposer in the `x;gt=2;rc=1;...` grammar (`transactions::FREE_FORM_PREFIX`), never listed. The core's
//! pick is `Shape::Free(slot)`; the terms live in the payload of the window (`state::Trade::free`: slot 0 the deal on the table, slot 1 the
//! one just named). So before the pick of a proposal is applied, [`pick`] reads the text with the string's own reader
//! (`transactions::offer_from_with`, the oracle of the property test), resolves the ids to the typed tables ([`spec_of`]) and writes the
//! terms into slot 1; and before a question is rendered, [`sync`] tells the renderer's table the text of slot 0 and the net the partner's
//! accept option reports (the number is a function of the terms: `trade_deals::free_nets`, which the property test pins to the string's).
//!
//! An id the string cannot read (malformed) is what the string treats it as: the proposer offered nothing. An id it reads but whose terms
//! the typed tables cannot name (an unknown card, note, planet or fragment kind) is a counted gap, never a guess.

use ti4_engine_core::choice::trade::{Ask, Opt, Shape};
use ti4_engine_core::choice::{Ask as CoreAsk, Choice as CoreChoice, Opt as COpt, Pick};
use ti4_engine_core::generated::{ActionCard, ExploreType, Faction, Planet, SecretObjective};
use ti4_engine_core::kernel::ability::Unknown;
use ti4_engine_core::kernel::turn_codes::TurnGap;
use ti4_engine_core::query::trade_deals::free_nets;
use ti4_engine_core::state::{CoreState, FreeNote, FreeSpec, FreeTerms, Window as OpenWindow, WindowKind};
use ti4_model::id::{ActionCardId, PlanetId, PlayerId, SecretObjectiveId};

use crate::core_adapter::typed;
use crate::generated::IdStr;
use crate::render::Render;
use crate::transactions::{free_form_id, offer_from_with, PlanetTerm, Terms, FREE_FORM_PREFIX};

/// The kind a relic fragment's trait names (`transactions::fragment_key`).
fn kind_of(trait_name: &str) -> Option<ExploreType> {
    match crate::transactions::fragment_key(trait_name).as_str() {
        "CULTURAL" => Some(ExploreType::Cultural),
        "HAZARDOUS" => Some(ExploreType::Hazardous),
        "INDUSTRIAL" => Some(ExploreType::Industrial),
        "FRONTIER" => Some(ExploreType::Frontier),
        _ => None,
    }
}

fn terms_of(t: &Terms) -> Option<FreeTerms> {
    let mut fragments = [0_i32; 4];
    for f in &t.fragments {
        fragments[kind_of(f)? as usize] += 1;
    }
    let note = match &t.promissory {
        None => None,
        Some(text) => Some(match text.strip_prefix(crate::promissory::SUPPORT_PREFIX) {
            Some(owner) => FreeNote::Support(typed::<Faction>(owner)?),
            None => FreeNote::Held(crate::trade_write::note_key(text)?),
        }),
    };
    Some(FreeTerms {
        trade_goods: t.trade_goods,
        commodities: t.commodities,
        fragments,
        note,
        action_card: match &t.action_card {
            None => None,
            Some(card) => Some(typed::<ActionCard>(card.as_str())?),
        },
        secret: match &t.secret {
            None => None,
            Some(secret) => Some(typed::<SecretObjective>(secret.as_str())?),
        },
        planet: match &t.planet {
            None => None,
            Some(p) => Some((typed::<Planet>(p.planet.as_str())?, typed::<Planet>(p.moves_to.as_str())?)),
        },
    })
}

/// The typed form of a free-form deal, `None` when a term has no typed form.
#[must_use]
pub fn spec_of(offer: &crate::transactions::Offer) -> Option<FreeSpec> {
    Some(FreeSpec { given: terms_of(&offer.given)?, received: terms_of(&offer.received)? })
}

/// The string's terms of a typed side (the inverse of [`terms_of`]).
fn string_terms(t: &FreeTerms) -> Terms {
    let mut fragments = Vec::new();
    for kind in [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial] {
        for _ in 0..t.fragments[kind as usize] {
            fragments.push(
                match kind {
                    ExploreType::Cultural => "CULTURAL",
                    ExploreType::Frontier => "FRONTIER",
                    ExploreType::Hazardous => "HAZARDOUS",
                    ExploreType::Industrial => "INDUSTRIAL",
                }
                .to_owned(),
            );
        }
    }
    Terms {
        trade_goods: t.trade_goods,
        commodities: t.commodities,
        fragments,
        promissory: t.note.map(|n| match n {
            FreeNote::Held(key) => crate::render::trade::note_id(key),
            FreeNote::Support(owner) => format!("{}{}", crate::promissory::SUPPORT_PREFIX, owner.as_str()),
        }),
        action_card: t.action_card.map(|c| ActionCardId::new(c.as_str())),
        secret: t.secret.map(|s| SecretObjectiveId::new(s.as_str())),
        planet: t.planet.map(|(planet, to)| PlanetTerm { planet: PlanetId::new(planet.as_str()), moves_to: PlanetId::new(to.as_str()) }),
    }
}

/// The id a typed free-form deal is written as (`transactions::free_form_id`).
#[must_use]
pub fn text_of(spec: &FreeSpec) -> String {
    free_form_id(&string_terms(&spec.given), &string_terms(&spec.received))
}

/// The renderer's table of the free-form deals, set to the deal the open window holds on the table (slot 0), so the question about it
/// is rendered with its text and the net its accept option reports. Call it with the position the question is rendered in.
pub fn sync(r: &Render, core: &CoreState) {
    let mut entries = Vec::new();
    if let Some(OpenWindow::Trade(t)) = core.pending().window(WindowKind::Trade)
        && let Some(spec) = &t.free[0]
    {
        entries.push((text_of(spec), free_nets(spec).1));
    }
    r.set_free(entries);
}

/// The pick `id` means in a proposal's question, when it is a free-form deal: the terms are written to slot 1 of the window's payload and the
/// pick names that slot. `None`: not a free-form id (the generic reading applies). `Some(Err(..))`: the deal has a term the typed tables do
/// not name.
///
/// A malformed id is "the proposer offered nothing" (`TradeWindow::resolve`), i.e. a decline.
pub fn pick(core: &mut CoreState, choice: &CoreChoice, id: &str) -> Option<Result<Pick, Unknown>> {
    let CoreAsk::Trade(Ask::Propose { proposer, partner, .. }) = choice.ask else { return None };
    if !id.starts_with(FREE_FORM_PREFIX) {
        return None;
    }
    let owner = |seat: ti4_engine_core::Seat| core.owners.id(seat).map(|b| PlayerId::new(String::from_utf8_lossy(b).into_owned()));
    let (me, other) = (owner(proposer)?, owner(partner)?);
    let Some(offer) = offer_from_with(&|_| String::new(), id, &me, &other) else {
        return Some(Ok(Pick::Opt(COpt::Decline)));
    };
    let Some(spec) = spec_of(&offer) else { return Some(Err(Unknown::TurnGap(TurnGap::TradeTerms))) };
    let Some(OpenWindow::Trade(t)) = core.pending_mut().window_mut(WindowKind::Trade) else { return None };
    t.free[1] = Some(spec);
    Some(Ok(Pick::Opt(COpt::Trade(Opt::Deal(Shape::Free(1))))))
}
