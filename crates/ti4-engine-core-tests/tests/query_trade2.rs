//! D4 lane A3: `query/trade.rs` beyond lane A1: what a deal needs to be paid (`can_pay`), why an offer is illegal (`why_illegal`, in the order the
//! rules ask), the neighbours who dealt this round, the partners one may open a negotiation with. LRR 60 (transactions) and 94 (negotiations).

use std::cell::Cell;

use ti4_engine_core::galaxy::Adjacency;
use ti4_engine_core::generated::{ActionCard, ExploreType, Planet, PromissoryNote, SecretObjective};
use ti4_engine_core::query::trade::{self as q, Cx, Env, Illegal, Leg, Limits, NoteFacts, Offer, PlanetFacts, Terms};
use ti4_engine_core::state::NoteKey;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

#[derive(Default)]
struct Rules {
    wide: Vec<u8>,
    reach: bool,
    arbiters: Vec<u8>,
    black_market: bool,
    reach_asked: Cell<u32>,
}

impl Env for Rules {
    fn has_iihq(&self, _: Seat) -> bool {
        false
    }
    fn wide(&self, who: Seat) -> bool {
        self.wide.contains(&who.0)
    }
    fn transaction_reach(&self, _: Seat, _: Seat) -> bool {
        self.reach_asked.set(self.reach_asked.get() + 1);
        self.reach
    }
    fn exempt_from_limit(&self, _: Seat, _: Seat) -> bool {
        false
    }
    fn trades_action_cards(&self, who: Seat) -> bool {
        self.arbiters.contains(&who.0)
    }
    fn black_market(&self) -> bool {
        self.black_market
    }
}

fn four() -> Scenario {
    Scenario::lobby(4).map_ring(7)
}

fn adj_of(s: &CoreState) -> std::sync::Arc<Adjacency> {
    s.map().adjacency().expect("a map").clone()
}

fn cx<'a>(s: &'a CoreState, adj: &'a Adjacency, env: &'a Rules) -> Cx<'a> {
    Cx { core: s, adj, mecatol: plain_systems()[0], limits: Limits { per_action: 1, per_round: 2 }, env }
}

fn goods(n: i32) -> Terms {
    Terms { trade_goods: n, ..Terms::default() }
}

fn offer(given: Terms, received: Terms) -> Offer {
    Offer { proposer: Seat(0), partner: Seat(1), given, received }
}

/// Seats 0 and 1 share a system (neighbours); 2 and 3 are far away.
fn together() -> Scenario {
    let t = plain_systems();
    four()
        .unit(t[1], None, Unit::CruiserI, 0, 1)
        .unit(t[1], None, Unit::CruiserI, 1, 1)
        .goods(0, 3)
        .commodities(0, 2)
        .goods(1, 1)
        .commodities(1, 1)
}

// ---- terms and what can be paid -----------------------------------------------------------------------

#[test]
fn nothing_changes_hands_exactly_when_every_part_of_the_terms_is_empty() {
    assert!(Terms::default().is_empty());
    let tweaks: [fn(&mut Terms); 7] = [
        |t| t.trade_goods = 1,
        |t| t.commodities = 1,
        |t| t.fragments[ExploreType::Industrial as usize] = 1,
        |t| t.note = Some(NoteFacts::default()),
        |t| t.action_card = Some(ActionCard::ALL[0]),
        |t| t.secret = Some(SecretObjective::ALL[0]),
        |t| t.planet = Some(PlanetFacts { payable: true, mv: None }),
    ];
    for (i, tweak) in tweaks.iter().enumerate() {
        let mut t = Terms::default();
        tweak(&mut t);
        assert!(!t.is_empty(), "part {i}");
    }
    for kind in 0..4 {
        let mut t = Terms::default();
        t.fragments[kind] = 2;
        assert!(!t.is_empty(), "fragments {kind}");
    }
}

#[test]
fn a_seat_can_pay_what_it_holds_and_not_more() {
    let mut s = together().state();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.fragments.set(ExploreType::Hazardous, 2);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.secrets.push(SecretObjective::ALL[3]);
    // Trade goods: three held.
    assert!(q::can_pay(&s, Seat(0), &goods(3)));
    assert!(!q::can_pay(&s, Seat(0), &goods(4)));
    assert!(q::can_pay(&s, Seat(0), &Terms::default()));
    // Commodities: two held, independently of the goods.
    assert!(q::can_pay(&s, Seat(0), &Terms { commodities: 2, ..Terms::default() }));
    assert!(!q::can_pay(&s, Seat(0), &Terms { commodities: 3, ..Terms::default() }));
    assert!(q::can_pay(&s, Seat(0), &Terms { trade_goods: 3, commodities: 2, ..Terms::default() }));
    // Fragments by kind: two hazardous, no cultural.
    let frag = |kind: ExploreType, n: i32| {
        let mut t = Terms::default();
        t.fragments[kind as usize] = n;
        t
    };
    assert!(q::can_pay(&s, Seat(0), &frag(ExploreType::Hazardous, 2)));
    assert!(!q::can_pay(&s, Seat(0), &frag(ExploreType::Hazardous, 3)));
    assert!(!q::can_pay(&s, Seat(0), &frag(ExploreType::Cultural, 1)));
    assert!(!q::can_pay(&s, Seat(0), &frag(ExploreType::Frontier, 1)));
    assert!(!q::can_pay(&s, Seat(0), &frag(ExploreType::Industrial, 1)));
    assert!(q::can_pay(&s, Seat(0), &frag(ExploreType::Cultural, 0)));
    // A secret objective only from the hand.
    assert!(q::can_pay(&s, Seat(0), &Terms { secret: Some(SecretObjective::ALL[3]), ..Terms::default() }));
    assert!(!q::can_pay(&s, Seat(0), &Terms { secret: Some(SecretObjective::ALL[4]), ..Terms::default() }));
    // A note or a planet card the string engine says the giver cannot pay.
    let note = |payable| Terms { note: Some(NoteFacts { payable, receivable: true, mv: None }), ..Terms::default() };
    assert!(q::can_pay(&s, Seat(0), &note(true)));
    assert!(!q::can_pay(&s, Seat(0), &note(false)));
    let planet = |payable| Terms { planet: Some(PlanetFacts { payable, mv: None }), ..Terms::default() };
    assert!(q::can_pay(&s, Seat(0), &planet(true)));
    assert!(!q::can_pay(&s, Seat(0), &planet(false)));
    // A seat that is not at the table pays nothing, not even nothing.
    assert!(!q::can_pay(&s, Seat(9), &Terms::default()));
}

// ---- why a deal is illegal --------------------------------------------------------------------------------

#[test]
fn an_offer_is_refused_for_the_first_reason_in_the_order_of_the_rules() {
    let s = together().state();
    let adj = adj_of(&s);
    let env = Rules::default();
    let c = cx(&s, &adj, &env);
    let ok = offer(goods(1), goods(1));
    assert_eq!(q::why_illegal(&c, &ok), None);
    assert_eq!(q::why_illegal(&c, &Offer { partner: Seat(0), ..ok }), Some(Illegal::SamePlayer));
    assert_eq!(q::why_illegal(&c, &Offer { proposer: Seat(9), ..ok }), Some(Illegal::PlayerMissing(Seat(9))));
    assert_eq!(q::why_illegal(&c, &Offer { partner: Seat(9), ..ok }), Some(Illegal::PlayerMissing(Seat(9))));
    // The proposer is checked before the partner.
    assert_eq!(q::why_illegal(&c, &Offer { proposer: Seat(8), partner: Seat(9), ..ok }), Some(Illegal::PlayerMissing(Seat(8))));
    assert_eq!(q::why_illegal(&c, &offer(Terms::default(), Terms::default())), Some(Illegal::Empty));
    // Empty on one side only is a gift: legal.
    assert_eq!(q::why_illegal(&c, &offer(goods(1), Terms::default())), None);
    assert_eq!(q::why_illegal(&c, &offer(Terms::default(), goods(1))), None);
    // Same player wins over a missing seat (checked first), and a missing seat over an empty offer.
    assert_eq!(q::why_illegal(&c, &Offer { proposer: Seat(9), partner: Seat(9), ..ok }), Some(Illegal::SamePlayer));
    assert_eq!(q::why_illegal(&c, &Offer { partner: Seat(9), given: Terms::default(), received: Terms::default(), ..ok }), Some(Illegal::PlayerMissing(Seat(9))));
}

#[test]
fn a_deal_needs_neighbours_except_in_the_agenda_phase_and_the_reach_is_asked_only_then() {
    let s = together().state();
    let adj = adj_of(&s);
    let env = Rules::default();
    let far = Offer { partner: Seat(3), ..offer(goods(1), goods(0)) };
    assert_eq!(q::why_illegal(&cx(&s, &adj, &env), &far), Some(Illegal::NotNeighbours(Seat(0), Seat(3))));
    // The agenda phase: anybody deals with anybody (LRR 94), and the map is not even asked.
    let mut agenda = s.clone();
    agenda.meta_mut().phase = Phase::Agenda;
    let env2 = Rules::default();
    assert_eq!(q::why_illegal(&cx(&agenda, &adj, &env2), &far), None);
    assert_eq!(env2.reach_asked.get(), 0);
    // A module's reach, or a wide seat on either side, makes them partners.
    let reach = Rules { reach: true, ..Rules::default() };
    assert_eq!(q::why_illegal(&cx(&s, &adj, &reach), &far), None);
    let wide = Rules { wide: vec![3], ..Rules::default() };
    assert_eq!(q::why_illegal(&cx(&s, &adj, &wide), &far), None);
    let wide0 = Rules { wide: vec![0], ..Rules::default() };
    assert_eq!(q::why_illegal(&cx(&s, &adj, &wide0), &far), None);
    // Empty offers are refused before the map is asked.
    let env3 = Rules::default();
    assert_eq!(q::why_illegal(&cx(&s, &adj, &env3), &Offer { partner: Seat(3), ..offer(Terms::default(), Terms::default()) }), Some(Illegal::Empty));
    assert_eq!(env3.reach_asked.get(), 0);
}

#[test]
fn each_side_has_to_hold_what_it_gives_and_the_proposer_is_named_first() {
    let s = together().state();
    let adj = adj_of(&s);
    let env = Rules::default();
    let c = cx(&s, &adj, &env);
    assert_eq!(q::why_illegal(&c, &offer(goods(4), goods(1))), Some(Illegal::CannotPay(Seat(0))), "seat 0 holds three goods");
    assert_eq!(q::why_illegal(&c, &offer(goods(1), goods(2))), Some(Illegal::CannotPay(Seat(1))), "seat 1 holds one");
    assert_eq!(q::why_illegal(&c, &offer(goods(4), goods(2))), Some(Illegal::CannotPay(Seat(0))));
    let commodities = Terms { commodities: 2, ..Terms::default() };
    assert_eq!(q::why_illegal(&c, &offer(goods(1), commodities)), Some(Illegal::CannotPay(Seat(1))), "seat 1 holds one commodity");
    assert_eq!(q::why_illegal(&c, &offer(commodities, goods(1))), None);
}

#[test]
fn a_note_the_receiver_may_not_take_is_refused_for_the_leg_that_carries_it() {
    let s = together().state();
    let adj = adj_of(&s);
    let env = Rules::default();
    let c = cx(&s, &adj, &env);
    let key = NoteKey { note: PromissoryNote::ALL[0], faction: Faction::TheArborec };
    let note = |receivable| Terms { note: Some(NoteFacts { payable: true, receivable, mv: Some(q::NoteMove::Held(key)) }), ..Terms::default() };
    // The note the proposer gives goes to the partner.
    assert_eq!(q::why_illegal(&c, &offer(note(false), goods(1))), Some(Illegal::NoteRefused(Seat(1), Leg::Given)));
    // The note the proposer receives goes to the proposer.
    assert_eq!(q::why_illegal(&c, &offer(goods(1), note(false))), Some(Illegal::NoteRefused(Seat(0), Leg::Received)));
    assert_eq!(q::why_illegal(&c, &offer(note(true), note(true))), None);
    // Paying comes before receiving: an unpayable note is a payment problem.
    let unpayable = Terms { note: Some(NoteFacts { payable: false, receivable: false, mv: None }), ..Terms::default() };
    assert_eq!(q::why_illegal(&c, &offer(unpayable, goods(1))), Some(Illegal::CannotPay(Seat(0))));
}

#[test]
fn action_cards_are_traded_only_through_the_arbiters_or_black_market_dealings_and_only_from_the_hand() {
    let mut s = together().hand(0, &[ActionCard::ALL[5]]).hand(1, &[ActionCard::ALL[6]]).state();
    let adj = adj_of(&s);
    let card = |c: ActionCard| Terms { action_card: Some(c), ..Terms::default() };
    let deal = offer(card(ActionCard::ALL[5]), goods(1));
    assert_eq!(q::why_illegal(&cx(&s, &adj, &Rules::default()), &deal), Some(Illegal::ActionCardsNotTradeable));
    let received = offer(goods(1), card(ActionCard::ALL[6]));
    assert_eq!(q::why_illegal(&cx(&s, &adj, &Rules::default()), &received), Some(Illegal::ActionCardsNotTradeable));
    // Either side's Arbiters, or Black Market Dealings, opens the door.
    for env in [Rules { arbiters: vec![0], ..Rules::default() }, Rules { arbiters: vec![1], ..Rules::default() }, Rules { black_market: true, ..Rules::default() }] {
        assert_eq!(q::why_illegal(&cx(&s, &adj, &env), &deal), None);
        assert_eq!(q::why_illegal(&cx(&s, &adj, &env), &received), None);
    }
    let arbiters = Rules { arbiters: vec![0], ..Rules::default() };
    // The card has to be in the hand of the side that gives it.
    let not_held = offer(card(ActionCard::ALL[7]), goods(1));
    assert_eq!(q::why_illegal(&cx(&s, &adj, &arbiters), &not_held), Some(Illegal::MissingActionCard(Seat(0), ActionCard::ALL[7])));
    let partner_card = offer(goods(1), card(ActionCard::ALL[5]));
    assert_eq!(q::why_illegal(&cx(&s, &adj, &arbiters), &partner_card), Some(Illegal::MissingActionCard(Seat(1), ActionCard::ALL[5])));
    // Both sides: the proposer's card is checked first.
    let both = offer(card(ActionCard::ALL[7]), card(ActionCard::ALL[8]));
    assert_eq!(q::why_illegal(&cx(&s, &adj, &arbiters), &both), Some(Illegal::MissingActionCard(Seat(0), ActionCard::ALL[7])));
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(ActionCard::ALL[7]);
    assert_eq!(q::why_illegal(&cx(&s, &adj, &arbiters), &both), Some(Illegal::MissingActionCard(Seat(1), ActionCard::ALL[8])));
}

#[test]
fn the_action_card_door_is_not_asked_for_a_deal_without_cards() {
    struct Spy(Cell<u32>);
    impl Env for Spy {
        fn has_iihq(&self, _: Seat) -> bool {
            false
        }
        fn wide(&self, _: Seat) -> bool {
            false
        }
        fn transaction_reach(&self, _: Seat, _: Seat) -> bool {
            false
        }
        fn exempt_from_limit(&self, _: Seat, _: Seat) -> bool {
            false
        }
        fn trades_action_cards(&self, _: Seat) -> bool {
            self.0.set(self.0.get() + 1);
            false
        }
        fn black_market(&self) -> bool {
            self.0.set(self.0.get() + 100);
            false
        }
    }
    let s = together().state();
    let adj = adj_of(&s);
    let spy = Spy(Cell::new(0));
    let c = Cx { core: &s, adj: &adj, mecatol: plain_systems()[0], limits: Limits { per_action: 1, per_round: 2 }, env: &spy };
    assert_eq!(q::why_illegal(&c, &offer(goods(1), goods(1))), None);
    assert_eq!(spy.0.get(), 0);
    let with_card = offer(Terms { action_card: Some(ActionCard::ALL[1]), ..Terms::default() }, goods(1));
    assert_eq!(q::why_illegal(&c, &with_card), Some(Illegal::ActionCardsNotTradeable));
    assert!(spy.0.get() >= 100, "Black Market Dealings is looked at first");
}

// ---- presence, round book, open partners ----------------------------------------------------------------------

#[test]
fn presence_includes_the_tile_a_placed_planet_stands_in() {
    let t = plain_systems();
    let planet = t[2].data().planets[0];
    let mut s = four().control(planet, 0).state();
    assert_eq!(q::presence(&s, Seat(0)).iter().collect::<Vec<_>>(), vec![t[2].index()]);
    // A planet moved to another tile (Mirage, Nova Seed): presence follows the tile it stands in.
    s.board_mut().planet_mut(planet).placed_in = Some(t[4]);
    assert_eq!(q::presence(&s, Seat(0)).iter().collect::<Vec<_>>(), vec![t[4].index()]);
    // Units on a planet count, controlled or not; a rival's do not.
    let other = t[3].data().planets[0];
    let s2 = four().unit(t[3], Some(other), Unit::InfantryI, 1, 1).unit(t[5], None, Unit::CruiserI, 1, 1).state();
    assert_eq!(q::presence(&s2, Seat(1)).iter().collect::<Vec<_>>(), vec![t[3].index(), t[5].index()]);
    assert!(q::presence(&s2, Seat(0)).is_empty());
}

#[test]
fn the_neighbours_who_dealt_this_round_are_listed_once_in_order_of_first_appearance() {
    let t = plain_systems();
    let s0 = four().unit(t[1], None, Unit::CruiserI, 0, 1).unit(t[1], None, Unit::CruiserI, 1, 1).unit(t[1], None, Unit::CruiserI, 2, 1).unit(t[6], None, Unit::CruiserI, 3, 1);
    let mut s = s0.state();
    let adj = adj_of(&s);
    let env = Rules::default();
    {
        let book = s.trade_mut().this_round.edit();
        book.push((Seat(2), Seat(0))); // seat 0 is the second of the pair
        book.push((Seat(0), Seat(1)));
        book.push((Seat(1), Seat(2)));
        book.push((Seat(0), Seat(2))); // seat 2 again
        book.push((Seat(1), Seat(3))); // not a neighbour of seat 0 and no deal of seat 0
        book.push((Seat(0), Seat(3))); // a deal with a seat that is not (any more) a neighbour
    }
    let c = cx(&s, &adj, &env);
    assert_eq!(q::neighbours(&c, Seat(0)).as_slice(), &[Seat(1), Seat(2)]);
    assert_eq!(q::neighbours_who_transacted(&c, Seat(0)).as_slice(), &[Seat(2), Seat(1)]);
    // Any deal of a neighbour counts, whoever the partner was: seat 1's neighbours 2 and 0 dealt in the first pair, (2, 0), in that order.
    assert_eq!(q::neighbours_who_transacted(&c, Seat(1)).as_slice(), &[Seat(2), Seat(0)]);
    assert!(q::neighbours_who_transacted(&c, Seat(3)).is_empty());
}

#[test]
fn a_negotiation_may_be_opened_with_the_neighbours_not_yet_dealt_with_this_turn() {
    let t = plain_systems();
    let base = four().unit(t[1], None, Unit::CruiserI, 0, 1).unit(t[1], None, Unit::CruiserI, 1, 1).unit(t[1], None, Unit::CruiserI, 2, 1).unit(t[6], None, Unit::CruiserI, 3, 1);
    let mut s = base.clone().state();
    let adj = adj_of(&s);
    let env = Rules::default();
    assert_eq!(q::open_partners(&cx(&s, &adj, &env), Seat(0)).as_slice(), &[Seat(1), Seat(2)]);
    assert_eq!(q::open_partners(&cx(&s, &adj, &env), Seat(3)).as_slice(), &[] as &[Seat], "seat 3 is alone, but a neighbour may deal with it");
    // The agenda-phase open door does not apply to opening; the neighbour reaches both ways, so seat 3 is not a partner of 0.
    s.trade_mut().transactions_this_turn.get_or_insert_with(Seat(0), Default::default).insert(Seat(1));
    assert_eq!(q::open_partners(&cx(&s, &adj, &env), Seat(0)).as_slice(), &[Seat(2)]);
    // A wide seat opens with everybody (except itself and those it dealt with this turn).
    let wide = Rules { wide: vec![0], ..Rules::default() };
    assert_eq!(q::open_partners(&cx(&s, &adj, &wide), Seat(0)).as_slice(), &[Seat(2), Seat(3)]);
    // The limit of negotiations per action empties the list.
    *s.trade_mut().negotiations_this_action.get_or_insert_with(Seat(0), || 0) = 1;
    assert!(q::open_partners(&cx(&s, &adj, &wide), Seat(0)).is_empty());
    // Structured diplomacy replaces the plain negotiation: nobody to open it with.
    let mut dip = base.state();
    dip.trade_mut().diplomacy.edit().enabled = true;
    assert!(q::open_partners(&cx(&dip, &adj, &env), Seat(0)).is_empty());
    let _ = Planet::COUNT;
}

// ---- survivor-driven (mutation round 1) ------------------------------------------------------------------------------

struct Iihq(u8);

impl Env for Iihq {
    fn has_iihq(&self, who: Seat) -> bool {
        who.0 == self.0
    }
    fn wide(&self, _: Seat) -> bool {
        false
    }
    fn transaction_reach(&self, _: Seat, _: Seat) -> bool {
        false
    }
    fn exempt_from_limit(&self, _: Seat, _: Seat) -> bool {
        false
    }
    fn trades_action_cards(&self, _: Seat) -> bool {
        false
    }
    fn black_market(&self) -> bool {
        false
    }
}

/// Mecatol in the middle of seven tiles; seat 0 on one outer tile, seat 1 on another that does not touch it, seat 2 on the middle.
fn around_mecatol() -> (CoreState, System, Vec<System>) {
    let mut sys = vec![System::MecatolRex18];
    sys.extend(plain_systems().into_iter().filter(|s| !matches!(s, System::MecatolRex18 | System::MecatolRex112)).take(18));
    (Scenario::lobby(4).map_from(&sys).state(), System::MecatolRex18, sys)
}

#[test]
fn the_imperial_hq_makes_the_holder_a_neighbour_of_whoever_is_in_or_beside_mecatol_and_of_nobody_else() {
    let (base, mecatol, sys) = around_mecatol();
    let adj = adj_of(&base);
    // Seats 0 and 1 stand on two outer tiles that do not touch; seat 2 stands on Mecatol; seat 3 two rings out.
    let (a, b) = (1..7).flat_map(|i| (1..7).map(move |j| (i, j))).find(|(i, j)| i != j && !adj.of(sys[*i]).contains(sys[*j].index())).unwrap();
    let far = (7..19).map(|i| sys[i]).find(|x| !adj.of(*x).contains(mecatol.index()) && !adj.of(*x).contains(sys[a].index()) && !adj.of(*x).contains(sys[b].index())).unwrap();
    let mut s = base.clone();
    for (system, seat) in [(sys[a], 0u8), (sys[b], 1), (mecatol, 2), (far, 3)] {
        s.board_mut().sys_mut(system).units.push(ti4_engine_core::state::Piece::new(Unit::CruiserI, Seat(seat)));
    }
    let plain = Rules::default();
    let c = Cx { mecatol, ..cx(&s, &adj, &plain) };
    assert!(!q::are_neighbours(&c, Seat(0), Seat(1)), "two outer tiles apart");
    let holder = |who: u8| Cx { mecatol, env: Box::leak(Box::new(Iihq(who))), ..cx(&s, &adj, &plain) };
    // The holder is a neighbour of the seat beside Mecatol (seat 1 is beside it), in both directions; the far seat is not near.
    let h0 = holder(0);
    assert!(q::mecatol_neighbours(&h0, Seat(0), Seat(1)) && q::mecatol_neighbours(&h0, Seat(1), Seat(0)));
    assert!(q::are_neighbours(&h0, Seat(0), Seat(1)) && q::are_neighbours(&h0, Seat(1), Seat(0)));
    assert!(q::are_neighbours(&h0, Seat(0), Seat(2)), "a seat on Mecatol itself");
    assert!(!q::mecatol_neighbours(&h0, Seat(0), Seat(3)) && !q::are_neighbours(&h0, Seat(0), Seat(3)), "two rings out is not near Mecatol");
    assert!(!q::mecatol_neighbours(&h0, Seat(1), Seat(3)), "the modernization is the holder's alone");
    assert!(q::near_mecatol(&c, Seat(1)) && q::near_mecatol(&c, Seat(2)) && !q::near_mecatol(&c, Seat(3)));
    assert_eq!(q::neighbours(&h0, Seat(0)).as_slice(), &[Seat(1), Seat(2)]);
    assert_eq!(q::partners(&h0, Seat(0)).as_slice(), &[Seat(1), Seat(2)]);
    assert!(q::may_transact(&h0, Seat(0), Seat(1)));
    // Another holder, not involved in the pair: no change.
    let h3 = holder(3);
    assert!(!q::are_neighbours(&h3, Seat(0), Seat(1)));
    // Nobody holds it: seats 0 and 1 are strangers.
    assert!(!q::may_transact(&c, Seat(0), Seat(1)));
}

#[test]
fn a_wide_partner_reaches_every_seated_player_and_an_unseated_one_is_nobodys_partner() {
    let s = together().state();
    let adj = adj_of(&s);
    let wide3 = Rules { wide: vec![3], ..Rules::default() };
    let c = cx(&s, &adj, &wide3);
    assert!(q::may_transact(&c, Seat(0), Seat(3)), "seat 3 reaches seat 0 whatever the map says");
    assert!(!q::may_transact(&c, Seat(9), Seat(3)), "an unseated proposer is no one's");
    assert!(!q::may_transact(&c, Seat(3), Seat(3)));
    let wide0 = Rules { wide: vec![0], ..Rules::default() };
    let c0 = cx(&s, &adj, &wide0);
    assert!(q::may_transact(&c0, Seat(0), Seat(3)));
    assert!(!q::may_transact(&c0, Seat(0), Seat(9)), "an unseated partner");
    assert_eq!(q::partners(&c0, Seat(0)).as_slice(), &[Seat(1), Seat(2), Seat(3)]);
}

#[test]
fn structured_diplomacy_is_a_switched_off_feature_that_the_negotiation_list_reports() {
    use ti4_engine_core::unsupported::{with_policy, Policy};
    let mut dip = together().state();
    dip.trade_mut().diplomacy.edit().enabled = true;
    let adj = adj_of(&dip);
    let env = Rules::default();
    // With the switch off (the default) the list is empty; with it on, meeting the feature is a panic that names it.
    assert!(q::open_partners(&cx(&dip, &adj, &env), Seat(0)).is_empty());
    let hit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| with_policy(Policy::Panic, || q::open_partners(&cx(&dip, &adj, &env), Seat(0)))));
    assert!(hit.is_err(), "the switched-off feature panics under the Panic policy");
    // A plain position never touches the switch.
    let plain = together().state();
    let adj2 = adj_of(&plain);
    let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| with_policy(Policy::Panic, || q::open_partners(&cx(&plain, &adj2, &env), Seat(0)).len())));
    assert!(ok.is_ok());
}
