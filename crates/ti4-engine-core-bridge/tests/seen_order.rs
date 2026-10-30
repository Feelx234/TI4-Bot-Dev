//! Enum order = text order (docs/todo/D3-bots.md trap T6, D3-1 item 5): the string bot iterates `BTreeMap<..Id, _>` in
//! the order of the id type; the typed view iterates enum order. Every id table the bot iterates must agree.

use ti4_engine::generated::IdStr;
use ti4_engine_core::generated::{Faction, Planet, PublicObjective, SecretObjective, StrategyCard, System, Technology, Unit};

/// The texts of an enum in enum order must be the texts sorted by the order of the string id type `S`.
fn same_order<T: IdStr, S: Ord + Clone>(make: impl Fn(&str) -> S, text: impl Fn(&S) -> String) {
    let in_enum_order: Vec<String> = (0..T::IDS.len()).map(|i| T::nth(i).unwrap().as_str().to_owned()).collect();
    let mut by_string_order: Vec<S> = in_enum_order.iter().map(|t| make(t)).collect();
    by_string_order.sort();
    let sorted: Vec<String> = by_string_order.iter().map(&text).collect();
    assert_eq!(in_enum_order, sorted);
}

#[test]
fn enum_order_is_the_order_of_the_string_ids() {
    same_order::<Planet, _>(|t: &str| ti4_model::id::PlanetId::new(t), |p| p.as_str().to_owned());
    same_order::<System, _>(|t: &str| ti4_model::id::SystemId::new(t), |p| p.as_str().to_owned());
    same_order::<Technology, _>(|t: &str| ti4_model::id::TechnologyId::new(t), |p| p.as_str().to_owned());
    same_order::<StrategyCard, _>(|t: &str| ti4_model::id::StrategyCardId::new(t), |p| p.as_str().to_owned());
    same_order::<Faction, _>(|t: &str| ti4_model::id::FactionId::new(t), |p| p.as_str().to_owned());
    same_order::<PublicObjective, _>(|t: &str| ti4_model::id::ObjectiveId::new(t), |p| p.as_str().to_owned());
    same_order::<SecretObjective, _>(|t: &str| ti4_model::id::SecretObjectiveId::new(t), |p| p.as_str().to_owned());
    same_order::<Unit, _>(|t: &str| ti4_model::id::UnitTypeId::new(t), |p| p.as_str().to_owned());
}

#[test]
fn the_enum_tables_are_in_byte_order_of_their_text() {
    // The text order the BTreeMaps of the string state use for plain `String` keys.
    fn check<T: IdStr>() {
        let ids: Vec<&str> = (0..T::IDS.len()).map(|i| T::nth(i).unwrap().as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }
    check::<Planet>();
    check::<Technology>();
    check::<StrategyCard>();
    check::<Faction>();
    check::<PublicObjective>();
    check::<SecretObjective>();
    check::<Unit>();
}
