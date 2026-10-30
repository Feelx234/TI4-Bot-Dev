//! D4-2c: `rules::abilities` (the faction abilities the core knows without a module), against the factions that print them.

use ti4_engine_core::generated::{Ability, StrategyCardKind};
use ti4_engine_core::kernel::hooks::CombatCtx;
use ti4_engine_core::rules::abilities::{self as ab, Cx, Env};
use ti4_engine_core::state::NoteKey;
use ti4_engine_core_tests::prelude::*;

/// The string side's answers, all neutral unless a field says otherwise.
#[derive(Default)]
struct Neutral {
    combat: i64,
    supply_plus: i32,
    tokens_plus: i32,
    waived: usize,
    substitutes: bool,
    trades: bool,
    ignores: bool,
    free: bool,
}

impl Env for Neutral {
    fn combat_hook(&self, _: Seat, _: CombatCtx) -> i64 {
        self.combat
    }
    fn fleet_supply_hook(&self, _: Seat, limit: i32) -> i32 {
        limit + self.supply_plus
    }
    fn status_tokens_hook(&self, _: Seat, count: i32) -> i32 {
        count + self.tokens_plus
    }
    fn waived_hook(&self, _: Seat, _: Technology) -> usize {
        self.waived
    }
    fn substitutes_primary_hook(&self, _: Seat, _: Option<StrategyCardKind>) -> bool {
        self.substitutes
    }
    fn trades_action_cards_hook(&self, _: Seat) -> bool {
        self.trades
    }
    fn ignores_neighbours_hook(&self, _: Seat) -> bool {
        self.ignores
    }
    fn secondary_is_free_hook(&self, _: Seat, _: Option<StrategyCardKind>) -> bool {
        self.free
    }
}

fn with(ability: Ability) -> Faction {
    LOBBY.into_iter().find(|f| f.data().abilities.contains(&ability)).unwrap_or_else(|| panic!("a lobby faction with {ability:?}"))
}

fn without(ability: Ability) -> Faction {
    LOBBY.into_iter().find(|f| !f.data().abilities.contains(&ability)).unwrap()
}

fn two(a: Faction, b: Faction) -> CoreState {
    Scenario::new(&[a, b]).state()
}

#[test]
fn a_seat_has_the_abilities_its_faction_prints_and_a_stranger_none() {
    let s = two(with(Ability::Armada), without(Ability::Armada));
    assert!(ab::has(&s, Seat(0), Ability::Armada) && !ab::has(&s, Seat(1), Ability::Armada));
    assert_eq!(ab::of_player(&s, Seat(0)), with(Ability::Armada).data().abilities);
    assert!(ab::of_player(&s, Seat(9)).is_empty() && !ab::has(&s, Seat(9), Ability::Armada));
}

#[test]
fn fragile_costs_a_point_on_every_combat_roll_unrelenting_gives_one_and_the_string_hook_adds() {
    let env = Neutral::default();
    let s = two(with(Ability::Fragile), with(Ability::Unrelenting));
    let cx = Cx { core: &s, env: &env };
    assert_eq!(ab::combat_modifier(&cx, Seat(0), CombatCtx::Space), -1);
    assert_eq!(ab::combat_modifier(&cx, Seat(1), CombatCtx::Ground), 1);
    let plain = two(without(Ability::Fragile), without(Ability::Unrelenting));
    assert_eq!(ab::combat_modifier(&Cx { core: &plain, env: &env }, Seat(0), CombatCtx::Space), 0);
    let hooked = Neutral { combat: 2, ..Neutral::default() };
    assert_eq!(ab::combat_modifier(&Cx { core: &s, env: &hooked }, Seat(0), CombatCtx::Space), 1, "-1 and the hook's +2");
}

#[test]
fn armada_adds_two_to_the_fleet_pool_before_the_hook_sees_it() {
    let env = Neutral { supply_plus: 10, ..Neutral::default() };
    let s = two(with(Ability::Armada), without(Ability::Armada));
    assert_eq!(ab::armada_bonus(&s, Seat(0)), 2);
    assert_eq!(ab::armada_bonus(&s, Seat(1)), 0);
    assert_eq!(ab::fleet_supply(&Cx { core: &s, env: &Neutral::default() }, Seat(0), 5), 7);
    assert_eq!(ab::fleet_supply(&Cx { core: &s, env: &env }, Seat(1), 5), 15, "the hook adds to the base");
    assert_eq!(ab::fleet_supply(&Cx { core: &s, env: &env }, Seat(0), 5), 17);
}

#[test]
fn status_phase_tokens_count_versatile_hyper_metabolism_and_borrowed_cybernetic_enhancements() {
    let env = Neutral::default();
    let a = with(Ability::Versatile);
    let b = without(Ability::Versatile);
    let s = two(a, b);
    let base = |s: &CoreState, who: u8| ab::status_tokens(&Cx { core: s, env: &env }, Seat(who), 2);
    assert_eq!(base(&s, 0), 3, "versatile");
    assert_eq!(base(&s, 1), 2);
    let hyper = Scenario::new(&[a, b]).tech(1, Technology::HyperMetabolism).state();
    assert_eq!(base(&hyper, 1), 3, "Hyper Metabolism");
    let mut lent = s.clone();
    let key = NoteKey { note: PromissoryNote::CyberneticEnhancements, faction: a };
    lent.cards_mut().promissory.edit().insert(key, Seat(1));
    assert_eq!(base(&lent, 1), 3, "another faction's Cybernetic Enhancements held");
    let own = {
        let mut s2 = s.clone();
        s2.cards_mut().promissory.edit().insert(NoteKey { note: PromissoryNote::CyberneticEnhancements, faction: b }, Seat(1));
        s2
    };
    assert_eq!(base(&own, 1), 2, "one's own copy does not count");
    let hooked = Neutral { tokens_plus: 1, ..Neutral::default() };
    assert_eq!(ab::status_tokens(&Cx { core: &s, env: &hooked }, Seat(1), 2), 3);
}

#[test]
fn analytical_waives_one_prerequisite_of_a_technology_that_is_not_a_unit_upgrade() {
    let env = Neutral::default();
    let s = two(with(Ability::Analytical), without(Ability::Analytical));
    let cx = Cx { core: &s, env: &env };
    assert_eq!(ab::waived_prerequisites(&cx, Seat(0), Technology::GravityDrive), 1);
    assert_eq!(ab::waived_prerequisites(&cx, Seat(0), Technology::CruiserII), 0, "not for an upgrade");
    assert_eq!(ab::waived_prerequisites(&cx, Seat(1), Technology::GravityDrive), 0);
    let hooked = Neutral { waived: 2, ..Neutral::default() };
    assert_eq!(ab::waived_prerequisites(&Cx { core: &s, env: &hooked }, Seat(0), Technology::GravityDrive), 3);
}

#[test]
fn the_printed_abilities_that_change_a_strategy_card_or_a_transaction() {
    let env = Neutral::default();
    let s = two(with(Ability::Brilliant), with(Ability::MastersOfTrade));
    let cx = Cx { core: &s, env: &env };
    assert!(ab::substitutes_primary(&cx, Seat(0), Some(StrategyCardKind::Technology)));
    assert!(!ab::substitutes_primary(&cx, Seat(0), Some(StrategyCardKind::Trade)) && !ab::substitutes_primary(&cx, Seat(0), None));
    assert!(ab::secondary_is_free(&cx, Seat(1), Some(StrategyCardKind::Trade)));
    assert!(!ab::secondary_is_free(&cx, Seat(1), Some(StrategyCardKind::Warfare)) && !ab::secondary_is_free(&cx, Seat(1), None));
    let hooks = Neutral { substitutes: true, trades: true, ignores: true, free: true, ..Neutral::default() };
    let hc = Cx { core: &s, env: &hooks };
    assert!(ab::substitutes_primary(&hc, Seat(1), None) && ab::trades_action_cards(&hc, Seat(1)) && ab::ignores_neighbours(&hc, Seat(1)) && ab::secondary_is_free(&hc, Seat(0), Some(StrategyCardKind::Leadership)));
    let t = two(with(Ability::Arbiters), with(Ability::GuildShips));
    let tc = Cx { core: &t, env: &env };
    assert!(ab::trades_action_cards(&tc, Seat(0)) && !ab::trades_action_cards(&tc, Seat(1)));
    assert!(ab::ignores_neighbours(&tc, Seat(1)) && !ab::ignores_neighbours(&tc, Seat(0)));
}
