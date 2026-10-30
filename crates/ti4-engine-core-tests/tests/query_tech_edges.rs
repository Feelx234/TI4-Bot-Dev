//! D4 lane A2: the edges of `query::tech` the first tests left to mutation survivors: the default hooks, the synergy of a breakthrough
//! (Thunder's Edge), the exact boundaries of the prerequisite arithmetic, Publicize Weapon Schematics and the Research Team laws.

use ti4_engine_core::generated::{Agenda, Breakthrough, Faction, Planet, Source, Technology as T};
use ti4_engine_core::keys::Outcome;
use ti4_engine_core::query::tech::*;
use ti4_engine_core::tables::{source_bit, Scope};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;

fn table() -> Scenario {
    Scenario::new(&[Faction::TheArborec, Faction::TheFederationOfSol, Faction::TheEmbersOfMuaat])
}

#[test]
fn the_default_hooks_say_nothing_special() {
    covers!("query::tech::NoHooks");
    let h = NoHooks;
    for t in T::ALL {
        assert!(!h.purged(t), "{t:?}");
        assert_eq!(h.waived_prerequisites(t), 0);
        assert!(!h.waiver_offer(t));
    }
    assert_eq!(h.extra_colours(), [0; 4]);
    assert!(!h.inheritance_affordable());
    // With them a seat researches exactly what its own cards and planets allow: nothing it owns, and a card with no prerequisites if it has none.
    let s = table().tech(0, T::NeuralMotivator).state();
    let list = researchable(&s, ALL_SOURCES, false, Seat(0), &h);
    assert!(!list.contains(T::NeuralMotivator), "owned");
    assert!(list.iter().all(|t| prerequisites_met(&s, ALL_SOURCES, false, Seat(0), t, &h) && open_to_faction(Faction::TheArborec, t)));
}

#[test]
fn a_breakthrough_pools_its_two_colours_only_when_its_source_is_in_play() {
    covers!("query::tech::synergy_joined");
    let (mut s, _) = table().build();
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.breakthrough = Some(Breakthrough::Psychospore);
    let te = Scope(source_bit(Source::ThundersEdge) | source_bit(Source::Base));
    // Psychospore: biotic and warfare.
    assert_eq!(synergy_joined(&s, te, Seat(0)), [true, false, false, true], "green and red are one for the Arborec");
    assert_eq!(synergy_joined(&s, Scope(source_bit(Source::Base)), Seat(0)), [false; 4], "Thunder's Edge is not in the game");
    assert_eq!(synergy_joined(&s, te, Seat(1)), [false; 4], "another seat has no breakthrough");
    assert_eq!(synergy_joined(&s, te, Seat(9)), [false; 4], "no such seat");
    s.players_mut().by_seat_mut(Seat(1)).unwrap().cards.relics.push(ti4_engine_core::generated::Relic::TheQuantumcore);
    assert_eq!(synergy_joined(&s, te, Seat(1)), [true; 4]);
}

#[test]
fn the_prerequisite_arithmetic_is_exact_at_each_boundary() {
    covers!("query::tech::satisfies");
    // Holdings exactly equal to the needs: met without a waiver; one short: not.
    assert!(satisfies([2, 1, 0, 0], [2, 1, 0, 0], [false; 4], 0));
    assert!(!satisfies([2, 1, 0, 0], [2, 0, 0, 0], [false; 4], 0));
    assert!(satisfies([2, 1, 0, 0], [2, 0, 0, 0], [false; 4], 1), "exactly one short, exactly one waiver");
    // More than needed is fine, and the surplus of one colour never covers another.
    assert!(satisfies([1, 0, 0, 0], [3, 0, 0, 0], [false; 4], 0));
    assert!(!satisfies([1, 1, 0, 0], [3, 0, 0, 0], [false; 4], 0));
    // A colour with no need never matters, a pool with no need of its colours is not consulted.
    assert!(satisfies([0; 4], [0; 4], [true; 4], 0));
    assert!(satisfies([0, 0, 1, 0], [0, 0, 1, 0], [true, true, false, false], 0), "the joined pair needs nothing: the blue is checked alone");
    // Pooled holdings exactly equal to the pooled need pass without waivers; one short needs exactly one.
    assert!(satisfies([2, 1, 0, 0], [0, 3, 0, 0], [true, true, false, false], 0));
    assert!(!satisfies([2, 1, 0, 0], [0, 2, 0, 0], [true, true, false, false], 0));
    assert!(satisfies([2, 1, 0, 0], [0, 2, 0, 0], [true, true, false, false], 1));
    // The same waiver cannot cover a pool shortfall and an unjoined colour.
    assert!(!satisfies([2, 0, 1, 0], [1, 0, 0, 0], [true, true, false, false], 1));
    assert!(satisfies([2, 0, 1, 0], [1, 0, 0, 0], [true, true, false, false], 2));
}

#[test]
fn publicize_weapon_schematics_waives_the_war_sun_prerequisites_once_somebody_owns_it() {
    covers!("query::tech::war_sun_prerequisites_waived");
    let hooks = NoHooks;
    let law = |s: &mut CoreState| {
        s.agenda_mut().laws.insert(Agenda::PublicizeWeaponSchematics, Outcome::For);
    };
    // Law in play, nobody owns the war sun: no waiver.
    let (mut s, _) = table().build();
    law(&mut s);
    assert!(!war_sun_prerequisites_waived(&s, false, T::WarSun));
    assert!(!can_research(&s, ALL_SOURCES, false, Seat(0), T::WarSun, &hooks));
    // The Federation of Sol's seat owns it: everybody may research it without the four slots.
    let (mut s, _) = table().tech(1, T::WarSun).build();
    assert!(!war_sun_prerequisites_waived(&s, false, T::WarSun), "no law yet");
    law(&mut s);
    assert!(war_sun_prerequisites_waived(&s, false, T::WarSun));
    assert!(!war_sun_prerequisites_waived(&s, true, T::WarSun), "blanked laws do not apply");
    assert!(!war_sun_prerequisites_waived(&s, false, T::CruiserII), "only the war sun");
    assert!(prerequisites_met(&s, ALL_SOURCES, false, Seat(0), T::WarSun, &hooks), "waived for the seat that owns nothing");
    assert!(!prerequisites_met(&s, ALL_SOURCES, true, Seat(0), T::WarSun, &hooks), "blanked: the printed prerequisites apply again");
    let ctx = ResearchCtx::of(&s, ALL_SOURCES, false, Seat(0), &hooks).unwrap();
    assert!(ctx.war_sun_waived);
    assert!(!ResearchCtx::of(&s, ALL_SOURCES, true, Seat(0), &hooks).unwrap().war_sun_waived);
}

#[test]
fn research_team_laws_and_the_prophets_tears_add_to_the_waivers_of_the_seat() {
    covers!("query::tech::ResearchCtx");
    let blue = Planet::ALL.iter().copied().find(|p| !p.data().specialties.is_empty()).expect("a specialty planet");
    let (mut s, _) = table().control(blue, 0).build();
    s.agenda_mut().laws.insert(Agenda::ResearchTeamBiotic, Outcome::Planet(blue));
    s.agenda_mut().laws.insert(Agenda::ResearchTeamWarfare, Outcome::Planet(blue));
    let ctx = |s: &CoreState, seat: u8, blanked: bool| ResearchCtx::of(s, ALL_SOURCES, blanked, Seat(seat), &NoHooks).unwrap().waivable;
    assert_eq!(ctx(&s, 0, false), 2, "two laws on a planet we control: one waiver each");
    assert_eq!(ctx(&s, 1, false), 0, "a seat that does not control the planet");
    assert_eq!(ctx(&s, 0, true), 0, "blanked");
    s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(ti4_engine_core::generated::Relic::TheProphetsTears);
    assert_eq!(ctx(&s, 0, false), 3, "plus the Prophet's Tears");
    ti4_engine_core::kernel::fx::exhaust_planet(&mut s, blue);
    assert_eq!(ctx(&s, 0, false), 1, "an exhausted planet gives no Research Team waiver");
    // A waiver turns a technology one prerequisite short into a researchable one.
    let (mut short, _) = table().build();
    assert!(!prerequisites_met(&short, ALL_SOURCES, false, Seat(0), T::HyperMetabolism, &NoHooks) || prerequisites(T::HyperMetabolism).iter().all(|n| *n == 0));
    short.players_mut().by_seat_mut(Seat(0)).unwrap().cards.relics.push(ti4_engine_core::generated::Relic::TheProphetsTears);
    let need: u16 = prerequisites(T::HyperMetabolism).iter().sum();
    assert_eq!(prerequisites_met(&short, ALL_SOURCES, false, Seat(0), T::HyperMetabolism, &NoHooks), need <= 1, "one slot waived");
}
