//! D4 lane A2: `query::trade_deals`, the list of deals a seat can put on the table and their prices. The rules behind it are the transaction rules
//! (LRR 94: trade goods, commodities, promissory notes, and with Black Market Dealings or Arbiters action cards, secrets and relic fragments) and
//! the promissory notes' own texts; the prices are the heuristic worths the module documents (formulae pinned here).

use ti4_engine_core::choice::trade::Shape;
use ti4_engine_core::generated::{ExploreType, Faction, PromissoryNote, SecretObjective};
use ti4_engine_core::query::trade_deals::{
    available_notes, available_support, deals, kenara_planets, nets, note_cost, note_worth, offer_of, option_price, sides, trade_agreement_worth, DealFacts, Held, Side,
    PLANET_GIVING_COST,
};
use ti4_engine_core::query::trade::NoteMove;
use ti4_engine_core::state::{NoteKey, TransientFlags};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

fn facts() -> DealFacts {
    DealFacts { note_order: |a, b| a.cmp(&b), card_order: |a, b| a.cmp(&b), unlocked: 0, pair: (Seat(0), Seat(1)), may: [true, true], arbiters: [false, false], planet_values: &[], free: [None, None] }
}

fn table(goods0: i16, com0: i16, goods1: i16, com1: i16) -> Scenario {
    Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheArborec]).goods(0, goods0).commodities(0, com0).goods(1, goods1).commodities(1, com1)
}

fn key(note: PromissoryNote, faction: Faction) -> NoteKey {
    NoteKey { note, faction }
}

fn give(s: &mut CoreState, k: NoteKey, to: u8) {
    s.cards_mut().promissory.edit().insert(k, Seat(to));
}

// ---- prices ---------------------------------------------------------------------------------------

#[test]
fn a_side_is_worth_to_its_receiver_what_the_formula_adds_and_costs_its_giver_a_fifth_per_commodity() {
    covers!("query::trade_deals::Side");
    let side = Side { goods: 2, commodities: 5, fragments: 1, note: Some(Held::Support), card: true, secret: true, planet: Some(3) };
    assert_eq!(side.worth(), 2.0 + 5.0 + 1.0 + 4.0 + 1.0 + 1.0 + 3.0, "a commodity is worth a whole good to the one who receives it");
    assert_eq!(side.cost(), 2.0 + 1.0 + 1.0 + 3.0 + 1.0 + 1.0 + PLANET_GIVING_COST, "a commodity costs 0.2; Support costs 3; a planet card costs more than any deal pays");
    let empty = Side::default();
    assert_eq!((empty.worth(), empty.cost()), (0.0, 0.0));
    for part in [Side { goods: 1, ..Side::default() }, Side { commodities: 1, ..Side::default() }, Side { fragments: 1, ..Side::default() }, Side { card: true, ..Side::default() }, Side { secret: true, ..Side::default() }] {
        assert_eq!(part.worth(), 1.0);
    }
    assert_eq!(Side { commodities: 1, ..Side::default() }.cost(), 0.2);
    assert_eq!(Side { planet: Some(2), ..Side::default() }.worth(), 2.0);
}

#[test]
fn a_trade_agreement_is_worth_four_fifths_of_its_owners_commodity_value_and_other_notes_their_table_row() {
    covers!("query::trade_deals::note_worth");
    for f in [Faction::TheEmiratesOfHacan, Faction::TheArborec, Faction::TheFederationOfSol] {
        assert_eq!(trade_agreement_worth(f), 0.8 * f64::from(f.data().commodities), "{f:?}");
        assert_eq!(note_worth(Held::Note(key(PromissoryNote::TradeAgreement, f))), trade_agreement_worth(f));
    }
    let k = |n| Held::Note(key(n, Faction::TheArborec));
    for (n, worth) in [
        (PromissoryNote::ResearchAgreement, 4.0),
        (PromissoryNote::Alliance, 3.0),
        (PromissoryNote::TradeConvoys, 3.0),
        (PromissoryNote::CyberneticEnhancements, 2.0),
        (PromissoryNote::MilitarySupport, 2.0),
        (PromissoryNote::PoliticalFavor, 2.0),
        (PromissoryNote::WarFunding, 2.0),
        (PromissoryNote::Ceasefire, 1.5),
    ] {
        assert_eq!(note_worth(k(n)), worth, "{n:?}");
        assert_eq!(note_cost(k(n)), 0.75 * worth, "{n:?}");
    }
    assert_eq!((note_worth(Held::Support), note_cost(Held::Support)), (4.0, 3.0));
}

#[test]
fn the_posted_price_of_a_note_is_its_row_rounded_half_to_even() {
    covers!("query::trade_deals::option_price");
    let prices: Vec<(PromissoryNote, u8)> = [PromissoryNote::ResearchAgreement, PromissoryNote::Alliance, PromissoryNote::TradeAgreement, PromissoryNote::Ceasefire, PromissoryNote::WarFunding, PromissoryNote::Stymie]
        .into_iter()
        .map(|n| (n, option_price(n)))
        .collect();
    assert_eq!(
        prices,
        vec![
            (PromissoryNote::ResearchAgreement, 4),
            (PromissoryNote::Alliance, 3),
            (PromissoryNote::TradeAgreement, 2), // 2.5 -> 2, not 3
            (PromissoryNote::Ceasefire, 2),      // 1.5 -> 2
            (PromissoryNote::WarFunding, 2),
            (PromissoryNote::Stymie, 2),
        ]
    );
}

#[test]
fn every_shape_names_what_the_proposer_gives_and_what_they_receive() {
    covers!("query::trade_deals::sides");
    let n = key(PromissoryNote::Ceasefire, Faction::TheArborec);
    let m = key(PromissoryNote::WarFunding, Faction::TheArborec);
    let cases: Vec<(Shape, Side, Side)> = vec![
        (Shape::Goods { give: 2, want: 1 }, Side { goods: 2, ..Side::default() }, Side { goods: 1, ..Side::default() }),
        (Shape::Swap { n: 3 }, Side { commodities: 3, ..Side::default() }, Side { commodities: 3, ..Side::default() }),
        (Shape::SwapOne, Side { commodities: 1, ..Side::default() }, Side { commodities: 1, ..Side::default() }),
        (Shape::CommoditiesForGoods { give: 3, want: 2 }, Side { commodities: 3, ..Side::default() }, Side { goods: 2, ..Side::default() }),
        (Shape::GoodsForCommodities { give: 1, want: 3 }, Side { goods: 1, ..Side::default() }, Side { commodities: 3, ..Side::default() }),
        (Shape::GiftCommodities { n: 2 }, Side { commodities: 2, ..Side::default() }, Side::default()),
        (Shape::Sell { note: n, price: 2 }, Side { note: Some(Held::Note(n)), ..Side::default() }, Side { goods: 2, ..Side::default() }),
        (Shape::BuyNote { note: n, price: 2 }, Side { goods: 2, ..Side::default() }, Side { note: Some(Held::Note(n)), ..Side::default() }),
        (Shape::BuyNoteWithCommodities { note: n, price: 2 }, Side { commodities: 2, ..Side::default() }, Side { note: Some(Held::Note(n)), ..Side::default() }),
        (Shape::NoteForCommodities { note: n, paid: 1 }, Side { note: Some(Held::Note(n)), ..Side::default() }, Side { commodities: 1, ..Side::default() }),
        (Shape::NoteForNote { owed: n, want: m }, Side { note: Some(Held::Note(n)), ..Side::default() }, Side { note: Some(Held::Note(m)), ..Side::default() }),
        (Shape::SellCard { card: ActionCard::ALL[0] }, Side { card: true, ..Side::default() }, Side { goods: 1, ..Side::default() }),
        (Shape::CardForNote { card: ActionCard::ALL[0], note: n }, Side { card: true, ..Side::default() }, Side { note: Some(Held::Note(n)), ..Side::default() }),
        (Shape::SellSecret { secret: SecretObjective::ALL[0] }, Side { secret: true, ..Side::default() }, Side { goods: 1, ..Side::default() }),
        (Shape::SellFragment { kind: ExploreType::Cultural }, Side { fragments: 1, ..Side::default() }, Side { goods: 1, ..Side::default() }),
        (Shape::SupportSwap { from: Faction::TheArborec, to: Faction::TheEmiratesOfHacan }, Side { note: Some(Held::Support), ..Side::default() }, Side { note: Some(Held::Support), ..Side::default() }),
    ];
    for (shape, given, received) in cases {
        assert_eq!(sides(shape), (given, received), "{shape:?}");
    }
    let (p, q) = (ti4_engine_core::generated::Planet::MecatolRex, ti4_engine_core::generated::Planet::Lesab);
    assert_eq!(sides(Shape::PlanetSale { planet: p, to: q, price: 3, value: 2 }), (Side { planet: Some(2), ..Side::default() }, Side { goods: 3, ..Side::default() }));
    assert_eq!(sides(Shape::PlanetBuy { planet: p, to: q, price: 3, value: 2 }), (Side { goods: 3, ..Side::default() }, Side { planet: Some(2), ..Side::default() }));
}

#[test]
fn the_net_of_a_deal_is_what_comes_in_less_what_goes_out_for_each_chair() {
    covers!("query::trade_deals::nets");
    let n = key(PromissoryNote::ResearchAgreement, Faction::TheArborec);
    assert_eq!(nets(Shape::Goods { give: 3, want: 1 }), (-2.0, 2.0));
    assert_eq!(nets(Shape::BuyNote { note: n, price: 4 }), (4.0 - 4.0, 4.0 - 3.0), "we pay 4 for a note worth 4; to the seller the 4 goods are worth 4 and the note costs 3");
    assert_eq!(nets(Shape::PlanetSale { planet: ti4_engine_core::generated::Planet::MecatolRex, to: ti4_engine_core::generated::Planet::Lesab, price: 3, value: 2 }).0, 3.0 - PLANET_GIVING_COST);
}

// ---- who can offer what ---------------------------------------------------------------------------

#[test]
fn each_seats_own_support_is_available_unless_it_is_lent() {
    covers!("query::trade_deals::available_support");
    let (mut s, _) = table(0, 0, 0, 0).build();
    assert_eq!(available_support(&s, Seat(0)), Some(Faction::TheEmiratesOfHacan));
    assert_eq!(available_support(&s, Seat(1)), Some(Faction::TheArborec));
    s.trade_mut().support_holders.insert(Seat(0), Seat(1));
    assert_eq!(available_support(&s, Seat(0)), None, "lent");
    assert_eq!(available_support(&s, Seat(1)), Some(Faction::TheArborec));
}

#[test]
fn notes_in_hand_are_listed_in_text_order_and_a_faceup_note_or_an_alliance_without_its_commander_is_left_out() {
    covers!("query::trade_deals::available_notes");
    let (mut s, _) = table(0, 0, 0, 0).build();
    let a = key(PromissoryNote::Ceasefire, Faction::TheArborec);
    let b = key(PromissoryNote::WarFunding, Faction::TheArborec);
    let c = key(PromissoryNote::Alliance, Faction::TheArborec);
    for k in [b, a, c] {
        give(&mut s, k, 0);
    }
    let f = facts();
    assert_eq!(available_notes(&s, &f, Seat(0)), vec![a, b], "the Alliance of the Arborec conveys nothing until its commander is unlocked");
    let unlocked = DealFacts { unlocked: 1 << 1, ..f };
    assert_eq!(available_notes(&s, &unlocked, Seat(0)), vec![c, a, b], "unlocked: the Alliance counts (enum order puts it first)");
    let reversed = DealFacts { note_order: |x, y| y.cmp(&x), ..unlocked };
    assert_eq!(available_notes(&s, &reversed, Seat(0)), vec![b, a, c], "the order is the caller's");
    s.cards_mut().promissory_faceup.edit().insert(a, ());
    assert_eq!(available_notes(&s, &unlocked, Seat(0)), vec![c, b], "a faceup note is on the table, not in hand");
    assert!(available_notes(&s, &unlocked, Seat(1)).is_empty());
}

// ---- the list --------------------------------------------------------------------------------------

#[test]
fn the_plain_deals_of_goods_and_commodities_are_listed_in_the_documented_order() {
    covers!("query::trade_deals::deals");
    let (s, _) = table(3, 2, 2, 1).build();
    let got = deals(&s, &facts(), Seat(0), Seat(1));
    let mut want = vec![
        Shape::SupportSwap { from: Faction::TheEmiratesOfHacan, to: Faction::TheArborec },
        Shape::Swap { n: 1 },
        Shape::CommoditiesForGoods { give: 2, want: 2 },
        Shape::GoodsForCommodities { give: 1, want: 1 },
    ];
    for g in 0..=3u16 {
        for w in 0..=2u16 {
            if g + w > 0 {
                want.push(Shape::Goods { give: g, want: w });
            }
        }
    }
    want.push(Shape::GiftCommodities { n: 2 });
    assert_eq!(got, want);
}

#[test]
fn nothing_to_trade_lists_only_the_support_swap_and_a_lent_support_drops_it() {
    covers!("query::trade_deals::deals");
    let (mut s, _) = table(0, 0, 0, 0).build();
    assert_eq!(deals(&s, &facts(), Seat(0), Seat(1)), vec![Shape::SupportSwap { from: Faction::TheEmiratesOfHacan, to: Faction::TheArborec }]);
    s.trade_mut().support_holders.insert(Seat(1), Seat(0));
    assert!(deals(&s, &facts(), Seat(0), Seat(1)).is_empty());
}

#[test]
fn a_swap_of_more_than_one_commodity_also_offers_the_single_swap() {
    covers!("query::trade_deals::deals");
    let (s, _) = table(0, 3, 0, 2).build();
    let got = deals(&s, &facts(), Seat(0), Seat(1));
    assert!(got.contains(&Shape::Swap { n: 2 }) && got.contains(&Shape::SwapOne));
    assert!(got.iter().position(|d| *d == Shape::Swap { n: 2 }) < got.iter().position(|d| *d == Shape::SwapOne));
    assert!(!got.iter().any(|d| matches!(d, Shape::CommoditiesForGoods { .. } | Shape::GoodsForCommodities { .. })), "no goods on either side");
    assert!(got.contains(&Shape::GiftCommodities { n: 3 }));
}

#[test]
fn goods_for_commodities_and_back_are_capped_at_three_and_at_what_the_other_side_has() {
    covers!("query::trade_deals::deals");
    let (s, _) = table(5, 5, 5, 5).build();
    let got = deals(&s, &facts(), Seat(0), Seat(1));
    assert!(got.contains(&Shape::CommoditiesForGoods { give: 3, want: 3 }));
    assert!(got.contains(&Shape::GoodsForCommodities { give: 3, want: 3 }));
    assert!(got.contains(&Shape::Goods { give: 3, want: 3 }));
    assert!(!got.iter().any(|d| matches!(d, Shape::Goods { give, want } if *give > 3 || *want > 3)));
    let (s, _) = table(1, 5, 5, 1).build();
    let got = deals(&s, &facts(), Seat(0), Seat(1));
    assert!(got.contains(&Shape::CommoditiesForGoods { give: 3, want: 3 }));
    assert!(got.contains(&Shape::GoodsForCommodities { give: 1, want: 1 }), "one trade good buys the one commodity the partner has");
}

#[test]
fn a_note_is_sold_for_its_price_when_the_partner_can_pay_and_given_for_nothing_when_not() {
    covers!("query::trade_deals::deals");
    let n = key(PromissoryNote::Ceasefire, Faction::TheArborec);
    let (mut rich, _) = table(0, 0, 3, 0).build();
    give(&mut rich, n, 0);
    let got = deals(&rich, &facts(), Seat(0), Seat(1));
    assert!(got.contains(&Shape::Sell { note: n, price: 2 }), "{got:?}");
    assert!(!got.contains(&Shape::Sell { note: n, price: 0 }));
    let (mut poor, _) = table(0, 0, 1, 0).build();
    give(&mut poor, n, 0);
    let got = deals(&poor, &facts(), Seat(0), Seat(1));
    assert!(got.contains(&Shape::Sell { note: n, price: 0 }), "the partner has 1 trade good, the price is 2: only a gift is on offer");
    assert!(!got.contains(&Shape::Sell { note: n, price: 2 }));
}

#[test]
fn the_asks_for_the_partners_notes_need_the_goods_or_commodities_to_pay_and_a_mahact_never_receives_an_alliance() {
    covers!("query::trade_deals::deals");
    let theirs = key(PromissoryNote::ResearchAgreement, Faction::TheArborec);
    let (mut s, _) = table(4, 4, 0, 0).build();
    give(&mut s, theirs, 1);
    let got = deals(&s, &facts(), Seat(0), Seat(1));
    assert!(got.contains(&Shape::BuyNote { note: theirs, price: 4 }));
    assert!(got.contains(&Shape::BuyNoteWithCommodities { note: theirs, price: 4 }));
    let (mut s, _) = table(3, 3, 0, 0).build();
    give(&mut s, theirs, 1);
    let got = deals(&s, &facts(), Seat(0), Seat(1));
    assert!(!got.iter().any(|d| matches!(d, Shape::BuyNote { .. } | Shape::BuyNoteWithCommodities { .. })), "3 is short of the price 4");
    // Hubris: nobody gives the Mahact player an Alliance.
    let sc = Scenario::new(&[Faction::TheEmiratesOfHacan, Faction::TheMahactGeneSorcerers]).goods(0, 4).goods(1, 4);
    let (mut s, _) = sc.build();
    let alliance = key(PromissoryNote::Alliance, Faction::TheEmiratesOfHacan);
    give(&mut s, alliance, 0);
    let unlocked = DealFacts { unlocked: 1, ..facts() };
    let got = deals(&s, &unlocked, Seat(0), Seat(1));
    assert!(!got.iter().any(|d| matches!(d, Shape::Sell { note, .. } | Shape::NoteForCommodities { note, .. } if note.note == PromissoryNote::Alliance)), "{got:?}");
}

#[test]
fn a_note_for_commodities_and_a_note_for_a_different_note_are_asked_best_net_first_at_most_four() {
    covers!("query::trade_deals::deals");
    let mine = [key(PromissoryNote::Ceasefire, Faction::TheEmiratesOfHacan), key(PromissoryNote::WarFunding, Faction::TheEmiratesOfHacan)];
    let theirs = [key(PromissoryNote::ResearchAgreement, Faction::TheArborec), key(PromissoryNote::Alliance, Faction::TheArborec), key(PromissoryNote::Stymie, Faction::TheArborec)];
    let (mut s, _) = table(0, 0, 0, 5).build();
    for k in mine {
        give(&mut s, k, 0);
    }
    for k in theirs {
        give(&mut s, k, 1);
    }
    let unlocked = DealFacts { unlocked: 0b11, ..facts() };
    let got = deals(&s, &unlocked, Seat(0), Seat(1));
    assert!(got.contains(&Shape::NoteForCommodities { note: mine[0], paid: 2 }), "a Ceasefire for the 2 commodities of its price");
    let n4: Vec<&Shape> = got.iter().filter(|d| matches!(d, Shape::NoteForNote { .. })).collect();
    assert_eq!(n4.len(), 4, "of the six pairs the best four");
    // The best net is the partner's Research Agreement (worth 4) for the cheapest note of ours (the Ceasefire costs 1.125).
    assert_eq!(*n4[0], Shape::NoteForNote { owed: mine[0], want: theirs[0] });
    let nets_of: Vec<f64> = n4.iter().map(|d| if let Shape::NoteForNote { owed, want } = d { note_worth(Held::Note(*want)) - note_cost(Held::Note(*owed)) } else { unreachable!() }).collect();
    assert!(nets_of.windows(2).all(|w| w[0] >= w[1]), "best net first: {nets_of:?}");
}

#[test]
fn action_cards_secrets_and_fragments_are_offered_only_through_arbiters_or_black_market_dealings() {
    covers!("query::trade_deals::deals");
    let card = ActionCard::ALL[5];
    let secret = SecretObjective::ALL[2];
    let sc = || table(0, 0, 2, 0).hand(0, &[ActionCard::ALL[9], card]);
    let (mut s, _) = sc().build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(secret);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.fragments.set(ExploreType::Cultural, 1);
    let theirs = key(PromissoryNote::Ceasefire, Faction::TheArborec);
    give(&mut s, theirs, 1);
    let plain = deals(&s, &facts(), Seat(0), Seat(1));
    assert!(!plain.iter().any(|d| matches!(d, Shape::SellCard { .. } | Shape::SellSecret { .. } | Shape::SellFragment { .. } | Shape::CardForNote { .. })));
    // Arbiters: the first card by alias is for sale or for a note; secrets and fragments stay shut.
    let arbiters = DealFacts { arbiters: [true, false], ..facts() };
    let got = deals(&s, &arbiters, Seat(0), Seat(1));
    let first = ActionCard::ALL[5].min(ActionCard::ALL[9]);
    assert!(got.contains(&Shape::SellCard { card: first }) && got.contains(&Shape::CardForNote { card: first, note: theirs }), "{got:?}");
    assert!(!got.iter().any(|d| matches!(d, Shape::SellSecret { .. } | Shape::SellFragment { .. })));
    // Black Market Dealings opens everything.
    s.transient_mut().misc.flags.set(TransientFlags::BLACK_MARKET);
    let got = deals(&s, &facts(), Seat(0), Seat(1));
    assert!(got.contains(&Shape::SellCard { card: first }));
    assert!(got.contains(&Shape::SellSecret { secret }));
    assert!(got.contains(&Shape::SellFragment { kind: ExploreType::Cultural }));
    assert!(!got.contains(&Shape::SellFragment { kind: ExploreType::Industrial }), "only the kinds we hold");
    // The partner cannot pay a good: nothing to sell for one.
    let (mut broke, _) = table(0, 0, 0, 0).hand(0, &[card]).build();
    broke.transient_mut().misc.flags.set(TransientFlags::BLACK_MARKET);
    let got = deals(&broke, &facts(), Seat(0), Seat(1));
    assert!(!got.iter().any(|d| matches!(d, Shape::SellCard { .. })));
}

// ---- the deal as a legality question ----------------------------------------------------------------------

#[test]
fn an_offer_of_a_note_deal_knows_whether_each_leg_can_be_paid_and_received() {
    covers!("query::trade_deals::offer_of");
    let mine = key(PromissoryNote::Ceasefire, Faction::TheEmiratesOfHacan);
    let (mut s, _) = table(0, 0, 0, 0).build();
    give(&mut s, mine, 0);
    let held = offer_of(&s, &facts(), Shape::Sell { note: mine, price: 2 }, Seat(0), Seat(1));
    assert_eq!((held.proposer, held.partner), (Seat(0), Seat(1)));
    let leg = held.given.note.expect("a note leg");
    assert!(leg.payable && leg.receivable);
    assert_eq!(leg.mv, Some(NoteMove::Held(mine)));
    assert_eq!(held.received.trade_goods, 2);
    let missing = offer_of(&s, &facts(), Shape::Sell { note: key(PromissoryNote::WarFunding, Faction::TheEmiratesOfHacan), price: 2 }, Seat(0), Seat(1));
    assert!(!missing.given.note.unwrap().payable, "a note we do not hold");
    let swap = offer_of(&s, &facts(), Shape::SupportSwap { from: Faction::TheEmiratesOfHacan, to: Faction::TheArborec }, Seat(0), Seat(1));
    assert_eq!(swap.given.note.unwrap().mv, Some(NoteMove::Support(Faction::TheEmiratesOfHacan)));
    assert!(swap.given.note.unwrap().payable && swap.received.note.unwrap().payable);
    s.trade_mut().support_holders.insert(Seat(1), Seat(0));
    let lent = offer_of(&s, &facts(), Shape::SupportSwap { from: Faction::TheEmiratesOfHacan, to: Faction::TheArborec }, Seat(0), Seat(1));
    assert!(lent.given.note.unwrap().payable && !lent.received.note.unwrap().payable, "the partner's Support is lent");
}

#[test]
fn an_offer_carries_the_card_the_secret_and_the_fragment_it_names() {
    covers!("query::trade_deals::offer_of");
    let (s, _) = table(0, 0, 0, 0).build();
    let card = ActionCard::ALL[3];
    assert_eq!(offer_of(&s, &facts(), Shape::SellCard { card }, Seat(0), Seat(1)).given.action_card, Some(card));
    let secret = SecretObjective::ALL[1];
    assert_eq!(offer_of(&s, &facts(), Shape::SellSecret { secret }, Seat(0), Seat(1)).given.secret, Some(secret));
    let frag = offer_of(&s, &facts(), Shape::SellFragment { kind: ExploreType::Hazardous }, Seat(0), Seat(1));
    assert_eq!(frag.given.fragments[ExploreType::Hazardous as usize], 1);
    assert_eq!(frag.given.fragments.iter().sum::<i32>(), 1);
    let g = offer_of(&s, &facts(), Shape::GiftCommodities { n: 3 }, Seat(0), Seat(1));
    assert_eq!((g.given.commodities, g.received.commodities), (3, 0));
}

// ---- Pride of Kenara --------------------------------------------------------------------------------

#[test]
fn kenara_planets_are_the_controlled_planets_with_a_pride_of_kenara_and_the_planets_its_units_could_move_to() {
    covers!("query::trade_deals::kenara_planets");
    let sys = plain_systems();
    let (a, b, c) = (sys[0].data().planets[0], sys[1].data().planets[0], sys[2].data().planets[0]);
    let sc = || table(0, 0, 0, 0).map_ring(4).control(a, 0).control(b, 0).control(c, 1).unit(sys[0], Some(a), Unit::PrideOfKenara, 0, 1);
    let (s, _) = sc().build();
    assert_eq!(kenara_planets(&s, Seat(0)), vec![(a, vec![b])], "the planet with the mech, and our other planet it could move to");
    assert!(kenara_planets(&s, Seat(1)).is_empty());
    let (old, _) = sc().rules(16).build();
    assert!(kenara_planets(&old, Seat(0)).is_empty(), "before rules version 17 the mech did not trade planets");
    let (alone, _) = table(0, 0, 0, 0).map_ring(4).control(a, 0).unit(sys[0], Some(a), Unit::PrideOfKenara, 0, 1).build();
    assert!(kenara_planets(&alone, Seat(0)).is_empty(), "nowhere to move");
}

#[test]
fn a_pride_of_kenara_adds_planet_sales_and_asks_to_the_list() {
    covers!("query::trade_deals::deals");
    let sys = plain_systems();
    let (a, b) = (sys[0].data().planets[0], sys[1].data().planets[0]);
    let values: &'static [u8] = Box::leak(vec![3u8; ti4_engine_core::generated::Planet::COUNT].into_boxed_slice());
    let f = DealFacts { planet_values: values, ..facts() };
    let (s, _) = table(0, 0, 3, 0).map_ring(4).control(a, 0).control(b, 0).unit(sys[0], Some(a), Unit::PrideOfKenara, 0, 1).build();
    let got = deals(&s, &f, Seat(0), Seat(1));
    assert!(got.contains(&Shape::PlanetSale { planet: a, to: b, price: 0, value: 3 }), "the gift");
    assert!(got.contains(&Shape::PlanetSale { planet: a, to: b, price: 3, value: 3 }), "the sale at the card's worth, the partner has 3");
    let (poor, _) = table(0, 0, 2, 0).map_ring(4).control(a, 0).control(b, 0).unit(sys[0], Some(a), Unit::PrideOfKenara, 0, 1).build();
    let got = deals(&poor, &f, Seat(0), Seat(1));
    assert!(got.contains(&Shape::PlanetSale { planet: a, to: b, price: 0, value: 3 }) && !got.contains(&Shape::PlanetSale { planet: a, to: b, price: 3, value: 3 }));
    // The partner's Pride: we can ask for the card when we can pay.
    let (theirs, _) = table(3, 0, 0, 0).map_ring(4).control(a, 1).control(b, 1).unit(sys[0], Some(a), Unit::PrideOfKenara, 1, 1).build();
    assert!(deals(&theirs, &f, Seat(0), Seat(1)).contains(&Shape::PlanetBuy { planet: a, to: b, price: 3, value: 3 }));
    let (cant, _) = table(2, 0, 0, 0).map_ring(4).control(a, 1).control(b, 1).unit(sys[0], Some(a), Unit::PrideOfKenara, 1, 1).build();
    assert!(!deals(&cant, &f, Seat(0), Seat(1)).iter().any(|d| matches!(d, Shape::PlanetBuy { .. })));
    let offer = offer_of(&s, &f, Shape::PlanetSale { planet: a, to: b, price: 3, value: 3 }, Seat(0), Seat(1));
    let leg = offer.given.planet.expect("a planet leg");
    assert!(leg.payable);
    assert_eq!(leg.mv, Some((a, b)));
}
