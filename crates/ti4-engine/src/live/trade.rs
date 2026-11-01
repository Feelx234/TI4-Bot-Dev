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
    let owner = |seat: ti4_engine_core::Seat| core.owners.id(seat).map(|b| PlayerId::new(String::from_utf8_lossy(b)));
    let (me, other) = (owner(proposer)?, owner(partner)?);
    let Some(offer) = offer_from_with(&|_| String::new(), id, &me, &other) else {
        return Some(Ok(Pick::Opt(COpt::Decline)));
    };
    let Some(spec) = spec_of(&offer) else { return Some(Err(Unknown::TurnGap(TurnGap::TradeTerms))) };
    let Some(OpenWindow::Trade(t)) = core.pending_mut().window_mut(WindowKind::Trade) else { return None };
    t.free[1] = Some(spec);
    Some(Ok(Pick::Opt(COpt::Trade(Opt::Deal(Shape::Free(1))))))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use ti4_content::ContentStore;
    use ti4_engine_core::generated::PromissoryNote;
    use ti4_engine_core::state::NoteKey;

    use super::*;
    use crate::generated::IdStr;

    /// The tables the core pins without text must agree with the string's text: the order of note ids and of cards, the worth of a planet
    /// card, the limits of the negotiations.
    #[test]
    fn the_cores_text_free_tables_agree_with_the_strings_text() {
        let factions = [Faction::TheEmiratesOfHacan, Faction::TheArborec, Faction::TheYinBrotherhood, Faction::TheUniversitiesOfJolNar];
        let keys: Vec<NoteKey> = PromissoryNote::ALL.iter().flat_map(|n| factions.iter().map(move |f| NoteKey { note: *n, faction: *f })).collect();
        for a in &keys {
            for b in &keys {
                assert_eq!(
                    ti4_engine_core::query::trade_deals::note_order(*a, *b),
                    crate::render::trade::note_id(*a).cmp(&crate::render::trade::note_id(*b)),
                    "{a:?} against {b:?}"
                );
            }
        }
        for a in ActionCard::ALL.iter().step_by(7) {
            for b in ActionCard::ALL.iter().step_by(5) {
                assert_eq!(ti4_engine_core::query::trade_deals::card_order(*a, *b), a.as_str().cmp(b.as_str()));
            }
        }
        assert_eq!(ti4_engine_core::query::trade_deals::planet_values(), crate::render::trade::planet_values(), "the worth of every planet card");
        assert_eq!(ti4_engine_core::rules::trade_window::MAX_NEGOTIATIONS_PER_ACTION, ti4_model::state::MAX_NEGOTIATIONS_PER_ACTION);
        assert_eq!(ti4_engine_core::rules::trade_window::MAX_NEGOTIATIONS_PER_ROUND, ti4_model::state::MAX_NEGOTIATIONS_PER_ROUND);
    }

    /// One field of a free-form id: a known key with a value that is sometimes valid, sometimes junk.
    fn field() -> impl Strategy<Value = String> {
        let side = prop_oneof![Just('g'), Just('r'), Just('q')];
        let kind = prop_oneof![Just('t'), Just('c'), Just('n'), Just('a'), Just('s'), Just('f'), Just('p'), Just('z')];
        let count = prop_oneof![(0..6u32).prop_map(|n| n.to_string()), Just("+2".to_owned()), Just(String::new()), Just("12345".to_owned())];
        let notes: Vec<&'static str> = vec![
            "cf:hacan", "ps:sol", "ta:hacan", "an:arborec", "convoys:hacan", "ms:sol", "support:hacan", "support:sol", "support:nobody", "bogus:hacan", "cf", "ps:nobody", "dark_pact:empyrean", "ra:jolnar",
        ];
        let cards: Vec<&'static str> = ActionCard::ALL.iter().step_by(11).map(|c| c.as_str()).chain(["nosuchcard"]).collect();
        let secrets: Vec<&'static str> = ti4_engine_core::generated::SecretObjective::ALL.iter().step_by(5).map(|c| c.as_str()).chain(["nosuchsecret"]).collect();
        let planets: Vec<&'static str> = Planet::ALL.iter().step_by(13).map(|c| c.as_str()).chain(["nosuchplanet"]).collect();
        (side, kind, count, 0..notes.len(), 0..cards.len(), 0..secrets.len(), 0..planets.len(), 0..planets.len(), 0..5usize).prop_map(move |(side, kind, count, n, c, s, p, q, f)| {
            let value = match kind {
                't' | 'c' => count,
                'n' => notes[n].to_owned(),
                'a' => cards[c].to_owned(),
                's' => secrets[s].to_owned(),
                'f' => ["cultural", "HAZARDOUS,industrial", "frontier", "unknown,frontier", "weird"][f].to_owned(),
                'p' => format!("{}>{}", planets[p], planets[q]),
                _ => "1".to_owned(),
            };
            format!("{side}{kind}={value};")
        })
    }

    fn id() -> impl Strategy<Value = String> {
        prop::collection::vec(field(), 0..6).prop_map(|fields| format!("x;{}", fields.concat()))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4000))]

        /// The typed reading of a free-form id agrees with the string's `offer_from` wherever the typed tables name every term: the price the
        /// partner's accept option reports is the string's own number to the last bit (the sums are taken in the same order), and the typed
        /// deal written back as text reads as the same deal.
        #[test]
        fn a_free_form_id_is_priced_and_round_trips_like_the_strings_reading(id in id()) {
            let state = crate::fixtures::seated_dealt(&[("a", "hacan"), ("b", "sol")], ti4_model::content_types::DEFAULT);
            let content = ContentStore::embedded();
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            let Some(offer) = offer_from_with(&|_| String::new(), &id, &a, &b) else { return Ok(()) };
            let Some(spec) = spec_of(&offer) else { return Ok(()) };
            let (net, theirs) = free_nets(&spec);
            let string_net = offer.received.worth_to_receiver(&state, content) - offer.given.cost_to_giver(&state, content);
            let string_theirs = offer.given.worth_to_receiver(&state, content) - offer.received.cost_to_giver(&state, content);
            prop_assert_eq!(net.to_bits(), string_net.to_bits(), "the proposer's net of {}", id);
            prop_assert_eq!(theirs.to_bits(), string_theirs.to_bits(), "the partner's net of {}", id);
            // The text written back is a free-form id that reads as the same typed deal.
            let again = offer_from_with(&|_| String::new(), &text_of(&spec), &a, &b).expect("the text of a typed deal is readable");
            prop_assert_eq!(spec_of(&again), Some(spec), "{}", id);
        }
    }

    /// The generator reaches the assertions of the property often enough: a property that returns early on every case proves nothing.
    #[test]
    fn the_generator_produces_many_typed_deals_with_every_kind_of_term() {
        use proptest::strategy::{Strategy, ValueTree};
        use proptest::test_runner::{Config, TestRunner};
        let mut runner = TestRunner::new(Config::default());
        let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
        let (mut typed, mut seen) = (0, [false; 7]);
        for _ in 0..2000 {
            let text = id().new_tree(&mut runner).expect("a tree").current();
            let Some(spec) = offer_from_with(&|_| String::new(), &text, &a, &b).and_then(|o| spec_of(&o)) else { continue };
            typed += 1;
            for side in [spec.given, spec.received] {
                seen[0] |= side.trade_goods > 0;
                seen[1] |= side.commodities > 0;
                seen[2] |= side.note.is_some();
                seen[3] |= side.action_card.is_some();
                seen[4] |= side.secret.is_some();
                seen[5] |= side.fragments.iter().any(|n| *n > 0);
                seen[6] |= side.planet.is_some();
            }
        }
        assert!(typed >= 150, "only {typed} of 2000 ids were typed deals");
        assert!(seen.iter().all(|s| *s), "every kind of term is generated: {seen:?}");
    }

    #[test]
    fn a_malformed_free_form_id_is_no_deal_and_a_term_the_tables_do_not_name_is_not_typed() {
        let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
        let read = |id: &str| offer_from_with(&|_| String::new(), id, &a, &b);
        assert!(read("x;bogus").is_none() && read("x;gt=1;gt=2;").is_none() && read("x;gt=+2;").is_none() && read("x;gt=;").is_none());
        let named = read("x;ga=nosuchcard;rt=1;").expect("the string reads it");
        assert!(spec_of(&named).is_none(), "an unknown card has no typed form: a counted gap, not a guess");
        let ok = read("x;gt=2;rn=support:hacan;gf=cultural,unknown;").expect("reads");
        let spec = spec_of(&ok).expect("typed");
        assert_eq!((spec.given.trade_goods, spec.received.note, spec.given.fragments), (2, Some(FreeNote::Support(Faction::TheEmiratesOfHacan)), [1, 1, 0, 0]));
    }
}
