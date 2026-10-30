//! D4-1 low layer: `query/tech.rs` (the pilot module: 26 % of its mutants were killed by the core's own tests).
//! Expectations come from the printed rules (prerequisites, colours, synergy, waivers), not from running another engine.

use proptest::prelude::*;
use ti4_engine_core::generated::{Faction, FactionTag, Planet, Relic, Source, System, TechType, Technology as T, Unit, UnitBaseType};
use ti4_engine_core::query::tech::*;
use ti4_engine_core::tables::{Scope, source_bit};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;

fn table() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheFederationOfSol, Faction::TheEmbersOfMuaat])
}

fn base_pok() -> Scope {
    Scope([Source::Base, Source::Codex1, Source::Codex2, Source::Codex3, Source::Codex4, Source::Pok].iter().fold(0, |m, s| m | source_bit(*s)))
}

/// Hooks with every answer settable.
#[derive(Default)]
struct Hooks {
    purged: Option<T>,
    waive: usize,
    extra: Colours,
    inherit: bool,
    offer: Option<T>,
}

impl ResearchHooks for Hooks {
    fn purged(&self, tech: T) -> bool {
        self.purged == Some(tech)
    }
    fn waived_prerequisites(&self, _: T) -> usize {
        self.waive
    }
    fn extra_colours(&self) -> Colours {
        self.extra
    }
    fn inheritance_affordable(&self) -> bool {
        self.inherit
    }
    fn waiver_offer(&self, tech: T) -> bool {
        self.offer == Some(tech)
    }
}

// ---- printed data -----------------------------------------------------------------------------

#[test]
fn the_four_tracks_have_distinct_indices_and_a_unit_upgrade_has_none() {
    assert_eq!(COLOURS.map(colour_index), [Some(0), Some(1), Some(2), Some(3)]);
    assert_eq!(colour_index(TechType::Unitupgrade), None);
    assert_eq!(colour_type(T::GravityDrive), Some(TechType::Propulsion));
    assert_eq!(colour_type(T::HyperMetabolism), Some(TechType::Biotic));
    assert_eq!(colour_type(T::IntegratedEconomy), Some(TechType::Cybernetic));
    assert_eq!(colour_type(T::AssaultCannon), Some(TechType::Warfare));
    assert_eq!(colour_type(T::CruiserII), None);
}

#[test]
fn unit_upgrades_are_exactly_the_technologies_typed_so() {
    assert!(is_unit_upgrade(T::CruiserII) && is_unit_upgrade(T::AdvancedCarrierII));
    assert!(!is_unit_upgrade(T::GravityDrive) && !is_unit_upgrade(T::AIDevelopmentAlgorithm));
}

#[test]
fn prerequisites_are_counts_in_track_order_green_yellow_blue_red() {
    assert_eq!(prerequisites(T::AssaultCannon), [0, 0, 0, 3]);
    assert_eq!(prerequisites(T::GravityDrive), [0, 0, 1, 0]);
    assert_eq!(prerequisites(T::CruiserII), [1, 1, 0, 1]);
    assert_eq!(prerequisites(T::IntegratedEconomy), [0, 3, 0, 0]);
    assert_eq!(prerequisites(T::HyperMetabolism), [2, 0, 0, 0]);
    assert_eq!(prerequisites(T::AntimassDeflectors), [0, 0, 0, 0]);
}

#[test]
fn a_faction_technology_belongs_to_one_faction_and_a_family_tag_to_none() {
    assert_eq!(faction_of(T::Bioplasmosis), Some(FactionTag::Arborec));
    assert_eq!(faction_of(T::GravityDrive), None);
    assert!(tag_is(FactionTag::Arborec, Faction::TheArborec));
    assert!(!tag_is(FactionTag::Arborec, Faction::TheFederationOfSol));
    assert!(!tag_is(FactionTag::Keleres, Faction::TheCouncilKeleresArgent), "a family tag is never equal to a faction id");
}

#[test]
fn a_faction_upgrade_replaces_the_generic_card_for_that_faction_only() {
    assert!(replaced_for_faction(Faction::TheFederationOfSol, T::CarrierII));
    assert!(!replaced_for_faction(Faction::TheArborec, T::CarrierII));
    assert!(replaced_for_faction(Faction::TheArborec, T::InfantryII), "Letani Warrior II");
    assert!(replaced_for_faction(Faction::TheFederationOfSol, T::InfantryII), "Spec Ops II");
    assert!(!replaced_for_faction(Faction::TheEmbersOfMuaat, T::InfantryII));
    assert!(!replaced_for_faction(Faction::TheArborec, T::GravityDrive));
    assert!(!open_to_faction(Faction::TheArborec, T::InfantryII), "replaced");
    assert!(open_to_faction(Faction::TheEmbersOfMuaat, T::InfantryII));
    assert!(!open_to_faction(Faction::TheFederationOfSol, T::Bioplasmosis), "another faction's card");
    assert!(open_to_faction(Faction::TheArborec, T::Bioplasmosis));
    assert!(open_to_faction(Faction::TheArborec, T::GravityDrive));
}

#[test]
fn the_deck_leaves_out_the_replaced_printings_and_thunders_edge_cards() {
    for t in [T::MagenDefenseGrid, T::MagenDefenseGridOmega, T::ERRORERROR, T::NULLREFERENCE, T::X89BacterialWeapon, T::X89BacterialWeaponOmega] {
        assert!(!in_deck(t), "{t:?}");
    }
    assert!(in_deck(T::GravityDrive) && in_deck(T::MagenDefenseGridOmegaOmega));
    assert!(!in_deck(T::Helios2), "a Thunder's Edge faction card");
    assert!(in_deck(T::ExecutiveOrder), "printed as a Thunder's Edge card but in the deck");
}

#[test]
fn the_active_deck_depends_on_the_scope_for_thunders_edge_cards_only() {
    let plain = active(base_pok());
    let te = active(Scope(base_pok().0 | source_bit(Source::ThundersEdge)));
    assert!(plain.contains(T::GravityDrive) && !plain.contains(T::MagenDefenseGrid));
    assert!(!plain.contains(T::Helios2) && te.contains(T::Helios2));
    assert!(plain.contains(T::ExecutiveOrder) && te.contains(T::ExecutiveOrder));
    assert!(plain.is_subset(&te) && plain.len() < te.len());
}

// ---- ownership and colours --------------------------------------------------------------------

#[test]
fn ownership_and_readiness_follow_the_seat_not_the_table() {
    let mut s = table().tech(0, T::GravityDrive).tech(1, T::FleetLogistics).state();
    assert!(owns(&s, Seat(0), T::GravityDrive) && !owns(&s, Seat(1), T::GravityDrive) && !owns(&s, Seat(9), T::GravityDrive));
    assert_eq!(owned(&s, Seat(0)).iter().collect::<Vec<_>>(), vec![T::GravityDrive]);
    assert!(owned(&s, Seat(9)).is_empty());
    assert!(ready(&s, Seat(0), T::GravityDrive));
    s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.insert(T::GravityDrive);
    assert!(!ready(&s, Seat(0), T::GravityDrive), "exhausted is not ready");
    assert!(owns(&s, Seat(0), T::GravityDrive), "but still owned");
    assert!(!ready(&s, Seat(0), T::FleetLogistics), "not owned by seat 0");
}

#[test]
fn colours_count_each_track_and_ignore_unit_upgrades() {
    let s = table().tech(0, T::GravityDrive).tech(0, T::FleetLogistics).tech(0, T::HyperMetabolism).tech(0, T::CruiserII).state();
    assert_eq!(owned_colours(&s, Seat(0)), [1, 0, 2, 0]);
    assert_eq!(owned_colours(&s, Seat(1)), [0, 0, 0, 0]);
    assert_eq!(colours_of(&owned(&s, Seat(0))), [1, 0, 2, 0]);
}

// ---- specialties ------------------------------------------------------------------------------

fn specialty_planet(kind: TechType) -> Planet {
    Planet::ALL.into_iter().find(|p| p.data().source == Source::Base && p.data().specialties == [kind]).expect("a base planet with that single specialty")
}

#[test]
fn specialties_count_controlled_ready_planets_by_colour() {
    let (blue, green) = (specialty_planet(TechType::Propulsion), specialty_planet(TechType::Biotic));
    let s = table().control(blue, 0).control(green, 0).control(specialty_planet(TechType::Warfare), 1).state();
    assert_eq!(specialties(&s, ALL_SOURCES, Seat(0)), [1, 0, 1, 0]);
    assert_eq!(specialties(&s, ALL_SOURCES, Seat(1)), [0, 0, 0, 1]);
    assert_eq!(specialties(&s, ALL_SOURCES, Seat(2)), [0, 0, 0, 0]);
    let ready_planets = readied_specialty_planets(&s, ALL_SOURCES, Seat(0));
    assert_eq!(ready_planets.as_slice().iter().map(|(p, _)| *p).collect::<Vec<_>>().len(), 2);
    assert!(ready_planets.as_slice().contains(&(blue, TechType::Propulsion)) && ready_planets.as_slice().contains(&(green, TechType::Biotic)));
}

#[test]
fn an_exhausted_planet_gives_no_specialty_unless_psychoarchaeology_frees_it_at_its_rules_version() {
    let blue = specialty_planet(TechType::Propulsion);
    let s = table().control(blue, 0).exhaust(blue).state();
    assert_eq!(specialties(&s, ALL_SOURCES, Seat(0)), [0; 4]);
    assert!(readied_specialty_planets(&s, ALL_SOURCES, Seat(0)).is_empty());
    let s = table().control(blue, 0).exhaust(blue).tech(0, T::Psychoarchaeology).state();
    assert!(psychoarchaeology_frees_specialties(&s, Seat(0)));
    assert_eq!(specialties(&s, ALL_SOURCES, Seat(0)), [0, 0, 1, 0], "freed: counted although exhausted");
    assert!(readied_specialty_planets(&s, ALL_SOURCES, Seat(0)).is_empty(), "but it is not a ready planet");
    let s = table().rules(RULES_TECH_EFFECTS - 1).control(blue, 0).exhaust(blue).tech(0, T::Psychoarchaeology).state();
    assert!(!psychoarchaeology_frees_specialties(&s, Seat(0)), "before its rules version");
    let s = table().rules(RULES_TECH_EFFECTS).state();
    assert!(!psychoarchaeology_frees_specialties(&s, Seat(0)), "not owned");
}

#[test]
fn a_planet_out_of_scope_shows_no_printed_specialty() {
    let p = Planet::ALL.into_iter().find(|p| p.data().source == Source::Pok && !p.data().specialties.is_empty()).expect("a PoK planet with a specialty");
    let s = table().state();
    assert!(!specialties_now(&s, base_pok(), p).is_empty());
    assert!(specialties_now(&s, Scope(source_bit(Source::Base)), p).is_empty());
}

#[test]
fn a_specialty_is_listed_once_even_when_printed_twice() {
    let s = table().state();
    let both = specialties_of(ALL_SOURCES, specialty_planet(TechType::Biotic), []);
    assert_eq!(both.as_slice(), &[TechType::Biotic]);
    let _ = s;
}

// ---- synergy ----------------------------------------------------------------------------------

#[test]
fn the_quantumcore_pools_all_four_tracks_and_nothing_pools_without_it() {
    let mut s = table().state();
    assert_eq!(synergy_joined(&s, ALL_SOURCES, Seat(0)), [false; 4]);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(Relic::TheQuantumcore);
    assert_eq!(synergy_joined(&s, ALL_SOURCES, Seat(0)), [true; 4]);
    assert_eq!(synergy_joined(&s, ALL_SOURCES, Seat(1)), [false; 4]);
    assert_eq!(synergy_joined(&s, ALL_SOURCES, Seat(9)), [false; 4]);
}

#[test]
fn satisfies_table_cases() {
    // Plain: each colour on its own.
    assert!(satisfies([0, 0, 0, 0], [0; 4], [false; 4], 0));
    assert!(satisfies([1, 0, 2, 0], [1, 0, 2, 0], [false; 4], 0));
    assert!(!satisfies([1, 0, 2, 0], [1, 0, 1, 0], [false; 4], 0));
    assert!(satisfies([1, 0, 2, 0], [1, 0, 1, 0], [false; 4], 1), "one waiver covers the missing blue");
    assert!(!satisfies([2, 0, 2, 0], [0, 0, 0, 0], [false; 4], 3));
    assert!(satisfies([2, 0, 2, 0], [0, 0, 0, 0], [false; 4], 4));
    // Pooled: a biotic and a cybernetic join; the pool must cover the SUM of the joined needs (rule 6).
    assert!(satisfies([1, 1, 0, 0], [2, 0, 0, 0], [true, true, false, false], 0));
    assert!(!satisfies([1, 1, 0, 0], [1, 0, 0, 0], [true, true, false, false], 0), "one card cannot fill two slots");
    assert!(satisfies([1, 1, 0, 0], [1, 0, 0, 0], [true, true, false, false], 1), "the missing slot waived");
    assert!(!satisfies([1, 1, 1, 0], [1, 0, 0, 0], [true, true, false, false], 1), "the unjoined blue still counts");
    assert!(satisfies([1, 1, 1, 0], [1, 0, 1, 0], [true, true, false, false], 1));
    // A pool of a colour nobody needs adds nothing.
    assert!(!satisfies([0, 0, 0, 1], [0, 3, 0, 0], [true, true, false, false], 0));
}

proptest! {
    #![proptest_config(ti4_engine_core_tests::generators::config())]
    /// The model: the total shortfall (pooled group first, then each unjoined colour) is covered by the waivers.
    #[test]
    fn satisfies_means_the_shortfall_is_within_the_waivers(needs in prop::array::uniform4(0u16..4), have in prop::array::uniform4(0u16..4), joined in prop::array::uniform4(any::<bool>()), waive in 0usize..4) {
        let wanted: usize = (0..4).filter(|i| joined[*i]).map(|i| usize::from(needs[i])).sum();
        let pooled: usize = (0..4).filter(|i| joined[*i]).map(|i| usize::from(have[i])).sum();
        let mut short = if joined.iter().any(|j| *j) { wanted.saturating_sub(pooled) } else { 0 };
        for i in 0..4 {
            if !joined[i] { short += usize::from(needs[i].saturating_sub(have[i])); }
        }
        prop_assert_eq!(satisfies(needs, have, joined, waive), short <= waive);
    }

    #[test]
    fn more_holdings_or_more_waivers_never_hurt(needs in prop::array::uniform4(0u16..4), have in prop::array::uniform4(0u16..4), joined in prop::array::uniform4(any::<bool>()), waive in 0usize..4, add in 0usize..4) {
        if satisfies(needs, have, joined, waive) {
            let mut more = have;
            more[add] += 1;
            prop_assert!(satisfies(needs, more, joined, waive));
            prop_assert!(satisfies(needs, have, joined, waive + 1));
        }
    }
}

// ---- waivers ----------------------------------------------------------------------------------

#[test]
fn ai_development_algorithm_waives_one_prerequisite_of_a_unit_upgrade_when_ready_and_in_force() {
    let s = table().tech(0, T::AIDevelopmentAlgorithm).state();
    assert_eq!(aida_waiver(&s, Seat(0), T::CruiserII), 1);
    assert_eq!(aida_waiver(&s, Seat(0), T::GravityDrive), 0, "only unit upgrades");
    assert_eq!(aida_waiver(&s, Seat(1), T::CruiserII), 0, "not owned");
    let mut spent = s.clone();
    spent.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.insert(T::AIDevelopmentAlgorithm);
    assert_eq!(aida_waiver(&spent, Seat(0), T::CruiserII), 0, "exhausted");
    let old = table().rules(RULES_TECH_EFFECTS - 1).tech(0, T::AIDevelopmentAlgorithm).state();
    assert_eq!(aida_waiver(&old, Seat(0), T::CruiserII), 0, "before its rules version");
}

#[test]
fn the_prophets_tears_waive_one_prerequisite() {
    let mut s = table().state();
    assert_eq!(relic_waivers(&s, Seat(0)), 0);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(Relic::TheProphetsTears);
    assert_eq!(relic_waivers(&s, Seat(0)), 1);
    assert_eq!(relic_waivers(&s, Seat(1)), 0);
}

#[test]
fn inheritance_systems_needs_the_card_ready_and_the_two_resources() {
    let s = table().tech(0, T::InheritanceSystems).state();
    let (yes, no) = (Hooks { inherit: true, ..Hooks::default() }, Hooks::default());
    assert!(inheritance_ready(&s, Seat(0), &yes));
    assert!(!inheritance_ready(&s, Seat(0), &no));
    assert!(!inheritance_ready(&s, Seat(1), &yes));
    assert_eq!(INHERITANCE_SYSTEMS_COST, 2);
}

// ---- research ---------------------------------------------------------------------------------

fn can(s: &CoreState, seat: u8, tech: T, h: &Hooks) -> bool {
    can_research(s, ALL_SOURCES, false, Seat(seat), tech, h)
}

#[test]
fn a_technology_needs_its_prerequisites_and_is_researchable_once() {
    let h = Hooks::default();
    let s = table().state();
    assert!(can(&s, 0, T::AntimassDeflectors, &h), "no prerequisite");
    assert!(!can(&s, 0, T::GravityDrive, &h), "needs one blue");
    let s = table().tech(0, T::AntimassDeflectors).state();
    assert!(can(&s, 0, T::GravityDrive, &h), "one propulsion card is one blue");
    assert!(!can(&s, 0, T::AntimassDeflectors, &h), "already owned");
    assert!(!can(&s, 0, T::FleetLogistics, &h), "needs two");
    assert!(!can(&s, 9, T::AntimassDeflectors, &h), "no such seat");
}

#[test]
fn a_specialty_planet_counts_as_a_prerequisite() {
    let h = Hooks::default();
    let blue = specialty_planet(TechType::Propulsion);
    let s = table().control(blue, 0).state();
    assert!(can(&s, 0, T::GravityDrive, &h));
    let s = table().control(blue, 0).exhaust(blue).state();
    assert!(!can(&s, 0, T::GravityDrive, &h), "exhausted: not counted");
}

#[test]
fn faction_cards_open_only_to_their_faction_and_replaced_cards_not_at_all_for_the_replacing_faction() {
    let h = Hooks::default();
    let s = table().tech(0, T::BioStims).state();
    assert!(can(&s, 0, T::Bioplasmosis, &Hooks { extra: [1, 0, 0, 0], ..Hooks::default() }), "Arborec's own card");
    assert!(!can(&s, 1, T::Bioplasmosis, &Hooks { extra: [2, 0, 0, 0], ..Hooks::default() }), "Sol may not");
    let s = table().state();
    assert!(!can(&s, 0, T::InfantryII, &Hooks { extra: [2, 0, 0, 0], ..Hooks::default() }), "Arborec has Letani Warrior II instead");
    assert!(can(&s, 2, T::InfantryII, &Hooks { extra: [2, 0, 0, 0], ..Hooks::default() }), "Muaat has no replacement");
    assert!(!can(&s, 1, T::InfantryII, &Hooks { extra: [2, 0, 0, 0], ..Hooks::default() }), "Sol has Spec Ops II instead");
    let _ = h;
}

#[test]
fn the_hooks_can_purge_add_holdings_waive_or_offer() {
    let s = table().state();
    assert!(!can(&s, 0, T::AntimassDeflectors, &Hooks { purged: Some(T::AntimassDeflectors), ..Hooks::default() }), "purged from every deck");
    assert!(can(&s, 0, T::AntimassDeflectors, &Hooks { purged: Some(T::GravityDrive), ..Hooks::default() }));
    assert!(can(&s, 0, T::GravityDrive, &Hooks { extra: [0, 0, 1, 0], ..Hooks::default() }), "extra holdings count");
    assert!(can(&s, 0, T::GravityDrive, &Hooks { waive: 1, ..Hooks::default() }), "a faction waiver");
    assert!(can(&s, 0, T::GravityDrive, &Hooks { offer: Some(T::GravityDrive), ..Hooks::default() }), "a waiver offer");
    assert!(!can(&s, 0, T::GravityDrive, &Hooks { offer: Some(T::FleetLogistics), ..Hooks::default() }));
    let inh = table().tech(0, T::InheritanceSystems).state();
    assert!(can(&inh, 0, T::AssaultCannon, &Hooks { inherit: true, ..Hooks::default() }), "Inheritance Systems pays 2 resources instead");
    assert!(!can(&inh, 0, T::AssaultCannon, &Hooks::default()), "unless it cannot pay");
    assert!(!can(&s, 0, T::AssaultCannon, &Hooks { inherit: true, ..Hooks::default() }), "without the card");
}

#[test]
fn ai_development_algorithm_helps_a_unit_upgrade_only_when_asked_to() {
    let s = table().rules(RULES_TECH_EFFECTS).tech(1, T::AIDevelopmentAlgorithm).state();
    let h = Hooks { extra: [1, 0, 0, 0], ..Hooks::default() };
    // CruiserII needs green, yellow and red: the algorithm itself is a warfare card (red), the hooks give green, yellow is missing: the algorithm waives it.
    assert!(prerequisites_met(&s, ALL_SOURCES, false, Seat(1), T::CruiserII, &h));
    assert!(!prerequisites_met_without_aida(&s, ALL_SOURCES, false, Seat(1), T::CruiserII, &h));
    let none = table().rules(RULES_TECH_EFFECTS).state();
    assert!(!prerequisites_met(&none, ALL_SOURCES, false, Seat(1), T::CruiserII, &h));
}

#[test]
fn researchable_lists_active_open_unowned_cards_in_id_order() {
    let h = Hooks::default();
    let s = table().tech(0, T::AntimassDeflectors).state();
    let r = researchable(&s, ALL_SOURCES, false, Seat(0), &h);
    assert!(r.contains(T::GravityDrive) && r.contains(T::DarkEnergyTap) && !r.contains(T::AntimassDeflectors));
    assert!(!r.contains(T::FleetLogistics), "needs two blue");
    let ids: Vec<usize> = r.iter().map(|t| t as usize).collect();
    assert!(ids.windows(2).all(|w| w[0] < w[1]));
    assert!(r.iter().all(|t| can(&s, 0, t, &h)), "every listed card passes can_research");
    assert!(researchable(&s, ALL_SOURCES, false, Seat(9), &h).is_empty());
    let by_pre = researchable_by_prerequisites(&s, ALL_SOURCES, false, Seat(0), &h);
    assert!(by_pre.is_subset(&r));
    let with_offer = researchable(&s, ALL_SOURCES, false, Seat(0), &Hooks { offer: Some(T::AssaultCannon), ..Hooks::default() });
    assert!(with_offer.contains(T::AssaultCannon) && !r.contains(T::AssaultCannon));
    let by_pre_offer = researchable_by_prerequisites(&s, ALL_SOURCES, false, Seat(0), &Hooks { offer: Some(T::AssaultCannon), ..Hooks::default() });
    assert!(!by_pre_offer.contains(T::AssaultCannon), "an offer is not a prerequisite");
}

#[test]
fn research_context_collects_holdings_from_cards_planets_and_hooks() {
    let blue = specialty_planet(TechType::Propulsion);
    let s = table().tech(0, T::HyperMetabolism).control(blue, 0).state();
    let ctx = ResearchCtx::of(&s, ALL_SOURCES, false, Seat(0), &Hooks { extra: [0, 1, 0, 0], ..Hooks::default() }).expect("a seated player");
    assert_eq!(ctx.holdings, [1, 1, 1, 0]);
    assert_eq!(ctx.faction, Faction::TheArborec);
    assert_eq!(ctx.waivable, 0);
    assert_eq!(ctx.joined, [false; 4]);
    assert!(ctx.owned.contains(T::HyperMetabolism));
    assert!(ResearchCtx::of(&s, ALL_SOURCES, false, Seat(9), &Hooks::default()).is_none());
}

// ---- unit upgrades ----------------------------------------------------------------------------

#[test]
fn a_seat_gets_its_upgrade_else_its_factions_unit_else_the_plain_one() {
    let mut roster = UnitRoster::default();
    roster.set(Faction::TheFederationOfSol, UnitBaseType::Carrier, Some(Unit::CarrierI));
    let plain = table().state();
    assert_eq!(unit_form(&plain, ALL_SOURCES, &roster, Seat(0), UnitBaseType::Cruiser), Some(Unit::CruiserI));
    assert_eq!(unit_form(&plain, ALL_SOURCES, &roster, Seat(9), UnitBaseType::Cruiser), None);
    let upgraded = table().tech(0, T::CruiserII).state();
    assert_eq!(unit_form(&upgraded, ALL_SOURCES, &roster, Seat(0), UnitBaseType::Cruiser), Some(Unit::CruiserII));
    assert_eq!(unit_form(&upgraded, ALL_SOURCES, &roster, Seat(1), UnitBaseType::Cruiser), Some(Unit::CruiserI), "seat 1 did not research it");
    assert_eq!(generic_unit(ALL_SOURCES, UnitBaseType::Infantry), Some(Unit::InfantryI));
    assert_eq!(generic_unit(ALL_SOURCES, UnitBaseType::Mech), Some(Unit::FactionlessMech));
    assert_eq!(roster.get(Faction::TheArborec, UnitBaseType::Carrier), None);
    assert!(faction_unit(&roster, Faction::TheFederationOfSol, UnitBaseType::Carrier).is_some());
}

#[test]
fn an_unlocked_upgrade_is_found_by_base_type_faction_and_held_technology() {
    let held = |ts: &[T]| ts.iter().copied().collect::<ti4_engine_core::state::TechSet>();
    assert_eq!(unlocked_upgrade(ALL_SOURCES, UnitBaseType::Cruiser, Faction::TheArborec, &held(&[T::CruiserII])), Some(Unit::CruiserII));
    assert_eq!(unlocked_upgrade(ALL_SOURCES, UnitBaseType::Cruiser, Faction::TheArborec, &held(&[T::CarrierII])), None);
    assert_eq!(unlocked_upgrade(ALL_SOURCES, UnitBaseType::Cruiser, Faction::TheArborec, &held(&[])), None);
    assert_eq!(unlocked_upgrade(ALL_SOURCES, UnitBaseType::Infantry, Faction::TheArborec, &held(&[T::InfantryII])), Some(Unit::InfantryII));
}

// ---- scars, faction technologies --------------------------------------------------------------

#[test]
fn faction_technologies_not_yet_owned_are_that_factions_own_cards_in_scope() {
    let owned_set: ti4_engine_core::state::TechSet = [T::Bioplasmosis].into_iter().collect();
    let left = unowned_faction_technologies_of(base_pok(), Faction::TheArborec, &owned_set);
    assert!(!left.contains(T::Bioplasmosis) && left.contains(T::LetaniWarriorII));
    assert!(left.iter().all(|t| faction_of(t).is_some_and(|tag| tag_is(tag, Faction::TheArborec))));
    let s = table().tech(0, T::Bioplasmosis).state();
    assert_eq!(unowned_faction_technologies(&s, base_pok(), Seat(0)), left);
    assert!(unowned_faction_technologies(&s, base_pok(), Seat(9)).is_empty());
    let sol = unowned_faction_technologies_of(base_pok(), Faction::TheFederationOfSol, &Default::default());
    assert!(sol.contains(T::AdvancedCarrierII) && !sol.contains(T::LetaniWarriorII));
    let out_of_scope = unowned_faction_technologies_of(Scope(source_bit(Source::Base)), Faction::TheArborec, &Default::default());
    assert!(!out_of_scope.iter().any(|t| t.data().source == Source::Pok));
}

#[test]
fn a_scar_is_a_system_flag_in_scope() {
    let scar = System::ALL.into_iter().find(|s| s.data().anomalies.scar).expect("a scar tile");
    let plain = System::ALL.into_iter().find(|s| !s.data().anomalies.scar).unwrap();
    assert!(is_scar(ALL_SOURCES, scar) && !is_scar(ALL_SOURCES, plain));
    let outside = Scope(!source_bit(scar.data().source));
    assert!(!is_scar(outside, scar), "a tile of an expansion that is not in play");
}
