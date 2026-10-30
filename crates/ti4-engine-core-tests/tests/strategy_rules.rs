//! D4-2a: `rules::strategy` (the draft order, the next picker, who may follow a secondary and why).

use std::cell::Cell;

use ti4_engine_core::generated::{StrategyCard, StrategyCardKind};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::rules::strategy::{self as st, Cx, Env};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::plain_systems;

/// Every answer of the string side settable per test.
struct Answers {
    free: bool,
    leadership: bool,
    research: bool,
    specialty: bool,
    tech_affordable: bool,
    secrets_scored: bool,
    produce: bool,
    asked: Cell<u32>,
}

impl Default for Answers {
    fn default() -> Self {
        Self { free: false, leadership: true, research: true, specialty: false, tech_affordable: false, secrets_scored: false, produce: true, asked: Cell::new(0) }
    }
}

impl Env for Answers {
    fn secondary_is_free(&self, _: Seat, _: Option<StrategyCardKind>) -> bool {
        self.asked.set(self.asked.get() + 1);
        self.free
    }
    fn substitutes_primary(&self, _: Seat, _: Option<StrategyCardKind>) -> bool {
        false
    }
    fn leadership_affordable(&self, _: Seat) -> bool {
        self.leadership
    }
    fn can_research(&self, _: Seat) -> bool {
        self.research
    }
    fn has_specialty_research_planet(&self, _: Seat) -> bool {
        self.specialty
    }
    fn technology_secondary_affordable(&self, _: Seat) -> bool {
        self.tech_affordable
    }
    fn secret_hand_all_scored(&self, _: Seat) -> bool {
        self.secrets_scored
    }
    fn can_produce_in(&self, _: Seat, _: System) -> bool {
        self.produce
    }
}

fn table(n: usize, per: u8) -> CoreState {
    let mut s = Scenario::lobby(n).state();
    s.meta_mut().strategy_cards_per_player = per;
    s.strategy_mut().unclaimed = StrategyCard::ALL[..8].iter().copied().collect();
    s
}

fn order(s: &CoreState) -> Vec<u8> {
    st::pick_order(s).as_slice().iter().map(|x| x.0).collect()
}

fn card_of(kind: StrategyCardKind) -> StrategyCard {
    StrategyCard::ALL.into_iter().find(|c| c.data().kind == kind).unwrap_or_else(|| panic!("a {kind:?} card"))
}

#[test]
fn the_draft_goes_clockwise_from_the_speaker_and_snakes_back_on_the_second_lap() {
    let mut s = table(4, 1);
    s.meta_mut().speaker = Seat(2);
    assert_eq!(order(&s), vec![2, 3, 0, 1]);
    let mut two = table(3, 2);
    two.meta_mut().speaker = Seat(1);
    assert_eq!(order(&two), vec![1, 2, 0, 0, 2, 1]);
    let mut three = table(3, 3);
    three.meta_mut().speaker = Seat(0);
    assert_eq!(order(&three), vec![0, 1, 2, 2, 1, 0, 0, 1, 2], "forward, back, forward");
    assert!(order(&table(3, 0)).is_empty());
}

#[test]
fn a_seat_under_political_stability_keeps_its_cards_and_sits_the_draft_out() {
    let mut s = table(4, 1);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().econ.stability = true;
    assert_eq!(order(&s), vec![0, 2, 3]);
}

#[test]
fn the_next_picker_follows_the_cards_already_dealt() {
    let mut s = table(3, 1);
    assert_eq!(st::next_picker(&s), Some(Seat(0)));
    fx::deal_strategy_card(&mut s, Seat(0), StrategyCard::ALL[0]);
    assert_eq!(st::next_picker(&s), Some(Seat(1)));
    fx::deal_strategy_card(&mut s, Seat(1), StrategyCard::ALL[1]);
    fx::deal_strategy_card(&mut s, Seat(2), StrategyCard::ALL[2]);
    assert_eq!(st::next_picker(&s), None, "everyone has picked");
    let empty = {
        let mut e = table(3, 1);
        e.strategy_mut().unclaimed.clear();
        e
    };
    assert_eq!(st::next_picker(&empty), None, "no card left on the mat");
    assert_eq!(st::next_picker(&table(3, 0)), None);
}

#[test]
fn a_kept_card_does_not_count_as_a_pick() {
    let mut s = table(3, 1);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.stability = true;
    fx::deal_strategy_card(&mut s, Seat(0), StrategyCard::ALL[0]);
    assert_eq!(st::next_picker(&s), Some(Seat(1)), "the retained card is not a pick; seat 0 is not in the order");
}

#[test]
fn the_draft_offers_the_cards_on_the_mat_to_the_next_picker() {
    let mut s = table(3, 1);
    let d = st::draft(&s).expect("a draft");
    assert_eq!(d.picker, Seat(0));
    assert_eq!(d.cards.len(), 8);
    s.strategy_mut().unclaimed.clear();
    assert_eq!(st::draft(&s), None);
}

#[test]
fn unused_cards_are_the_held_ones_that_are_not_exhausted() {
    let mut s = table(3, 2);
    let (a, b) = (StrategyCard::ALL[0], StrategyCard::ALL[3]);
    fx::deal_strategy_card(&mut s, Seat(0), a);
    fx::deal_strategy_card(&mut s, Seat(0), b);
    assert_eq!(st::unused_cards(&s, Seat(0)).len(), 2);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().strat_exhausted.insert(a);
    assert_eq!(st::unused_cards(&s, Seat(0)).as_slice(), &[b]);
    assert!(st::unused_cards(&s, Seat(9)).is_empty());
}

#[test]
fn only_leadership_follows_without_a_token() {
    assert!(!st::secondary_costs_token(Some(StrategyCardKind::Leadership)));
    assert!(st::secondary_costs_token(Some(StrategyCardKind::Warfare)));
    assert!(st::secondary_costs_token(None));
    assert_eq!(st::commodity_limit(Faction::TheEmiratesOfHacan), i32::from(Faction::TheEmiratesOfHacan.data().commodities));
}

#[test]
fn following_needs_a_token_or_a_free_secondary_and_something_to_do() {
    let k = kernel_all();
    let s = Scenario::lobby(3).tokens(0, 2, 2, 0).tokens(1, 2, 2, 1).state();
    let env = Answers::default();
    let cx = Cx { core: &s, kernel: &k, env: &env };
    let imperial = card_of(StrategyCardKind::Imperial);
    assert!(!st::secondary_eligible(&cx, Seat(0), imperial), "no strategy token");
    assert!(st::secondary_eligible(&cx, Seat(1), imperial), "one token");
    assert!(!st::secondary_eligible(&cx, Seat(9), imperial), "not a player");
    let free = Answers { free: true, ..Answers::default() };
    let cx = Cx { core: &s, kernel: &k, env: &free };
    assert!(st::secondary_eligible(&cx, Seat(0), imperial), "a free secondary needs no token");
    assert!(st::secondary_is_free(&cx, Seat(0), Some(StrategyCardKind::Imperial)));
    assert!(!st::secondary_is_free(&cx, Seat(0), None), "no card, nothing to be free");
}

#[test]
fn leadership_is_followed_when_affordable_whatever_the_tokens() {
    let k = kernel_all();
    let s = Scenario::lobby(3).tokens(0, 0, 0, 0).state();
    let leadership = card_of(StrategyCardKind::Leadership);
    let yes = Answers::default();
    assert!(st::secondary_eligible(&Cx { core: &s, kernel: &k, env: &yes }, Seat(0), leadership));
    let no = Answers { leadership: false, ..Answers::default() };
    assert!(!st::secondary_eligible(&Cx { core: &s, kernel: &k, env: &no }, Seat(0), leadership));
}

#[test]
fn each_card_kind_needs_its_own_something_to_do() {
    let k = kernel_all();
    let systems = plain_systems();
    let planet = systems[1].data().planets[0];
    let base = || Scenario::lobby(3).map_ring(7).tokens(0, 2, 2, 2);
    let eligible = |s: &CoreState, env: &Answers, kind: StrategyCardKind| st::secondary_eligible(&Cx { core: s, kernel: &k, env }, Seat(0), card_of(kind));
    let env = Answers::default();
    // Diplomacy: an exhausted planet of ours.
    let plain = base().state();
    assert!(!eligible(&plain, &env, StrategyCardKind::Diplomacy));
    let spent = base().control(planet, 0).exhaust(planet).state();
    assert!(eligible(&spent, &env, StrategyCardKind::Diplomacy));
    let ready = base().control(planet, 0).state();
    assert!(!eligible(&ready, &env, StrategyCardKind::Diplomacy), "a ready planet gives nothing to ready");
    // Trade: commodities below the limit.
    let limit = i16::from(Faction::TheArborec.data().commodities.max(Faction::TheEmiratesOfHacan.data().commodities));
    let _ = limit;
    let full = base().commodities(0, i16::from(Scenario::lobby(3).faction(0).data().commodities)).state();
    assert!(!eligible(&full, &env, StrategyCardKind::Trade));
    assert!(eligible(&base().commodities(0, 0).state(), &env, StrategyCardKind::Trade));
    // Warfare: the home system can produce.
    let mut home = base().state();
    home.players_mut().by_seat_mut(Seat(0)).unwrap().home_system = Some(systems[0]);
    assert!(eligible(&home, &env, StrategyCardKind::Warfare));
    assert!(!eligible(&home, &Answers { produce: false, ..Answers::default() }, StrategyCardKind::Warfare));
    assert!(!eligible(&base().state(), &env, StrategyCardKind::Warfare), "no home system on record");
    // Technology: can research and either a substitute, a specialty planet or an affordable payment.
    assert!(!eligible(&plain, &env, StrategyCardKind::Technology));
    assert!(eligible(&plain, &Answers { specialty: true, ..Answers::default() }, StrategyCardKind::Technology));
    assert!(eligible(&plain, &Answers { tech_affordable: true, ..Answers::default() }, StrategyCardKind::Technology));
    assert!(!eligible(&plain, &Answers { specialty: true, research: false, ..Answers::default() }, StrategyCardKind::Technology));
    // Imperial: nothing needed at the old version, a secret to draw from the capped version on.
    let capped = base().rules(35).state();
    assert!(eligible(&capped, &env, StrategyCardKind::Imperial));
    assert!(!eligible(&capped, &Answers { secrets_scored: true, ..Answers::default() }, StrategyCardKind::Imperial));
    let old = base().rules(1).state();
    assert!(eligible(&old, &Answers { secrets_scored: true, ..Answers::default() }, StrategyCardKind::Imperial), "before the cap rule");
}
