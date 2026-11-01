//! Lockstep tests of the research window's waiver questions (lane G5; `strategy_cards::resolve_research`): the Cabal's Riftmeld and Yin's
//! Brother Omar. Each plays the Technology ability on a real seated game under the `Dual` strategy backend (the string and the core windows
//! compared question by question, byte for byte, and position by position) and reads the string position afterwards.

use ti4_content::ContentStore;
use ti4_model::content_types::POK;
use ti4_model::id::{LeaderId, PlayerId, TechnologyId, UnitTypeId};
use ti4_model::state::{GameState, LeaderStatus};

use crate::choice::{Scripted, Table};
use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};

const LABEL_P: &str = "strategy_cards::technology_primary";
const LABEL_S: &str = "strategy_cards::technology_secondary";
const CABAL: [(&str, &str); 3] = [("a", "cabal"), ("b", "hacan"), ("c", "sol")];
const YIN: [(&str, &str); 3] = [("a", "yin"), ("b", "hacan"), ("c", "sol")];

fn a() -> PlayerId {
    PlayerId::new("a")
}

fn content() -> &'static ContentStore {
    ContentStore::embedded()
}

/// The Technology ability of `a` under the Dual backend: `(compared, bailed)` of its window and the string position afterwards.
fn play(factions: &[(&str, &str)], setup: impl FnOnce(&mut GameState), answers: &[&str], primary: bool) -> ((u64, u64), GameState) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    let mut after = None;
    with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
        let mut state = crate::fixtures::seated_game(factions, POK);
        crate::promissory::deal(&mut state, content(), POK);
        setup(&mut state);
        let mut table = Table::with_default(Box::new(Scripted::new(answers.iter().map(|x| (*x).to_owned()))));
        if primary {
            crate::strategy_cards::primary(&mut state, content(), POK, None, &mut table, &a(), "pok7technology").unwrap();
        } else {
            crate::strategy_cards::secondary(&mut state, content(), POK, None, &mut table, &a(), "pok7technology").unwrap();
        }
        after = Some(state);
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let counts = crate::dual::thread_window_counts(if primary { LABEL_P } else { LABEL_S });
    let _ = take_report();
    (counts, after.expect("played"))
}

fn owns(state: &GameState, who: &str, tech: &str) -> bool {
    state.player(&PlayerId::new(who)).unwrap().technologies.contains(&TechnologyId::new(tech))
}

fn captured(state: &GameState) -> Vec<(String, String)> {
    state.player(&a()).unwrap().captured_units.iter().map(|(owner, unit)| (owner.to_string(), unit.to_string())).collect()
}

/// The Cabal holds the captured models and has trade goods to pay a research with.
fn cabal(models: &'static [(&'static str, &'static str)], goods: i32) -> impl Fn(&mut GameState) + Clone {
    move |s: &mut GameState| {
        s.player_mut(&a()).unwrap().trade_goods = goods.into();
        s.player_mut(&a()).unwrap().captured_units = models.iter().map(|(owner, unit)| (PlayerId::new(*owner), UnitTypeId::new(*unit))).collect();
    }
}

/// A technology `a` can research only through a waiver, and one it researches by its prerequisites.
fn waiver_only_and_plain(setup: &impl Fn(&mut GameState)) -> (TechnologyId, TechnologyId) {
    let mut probe = crate::fixtures::seated_game(&CABAL, POK);
    setup(&mut probe);
    let plain = crate::technology::researchable_by_prerequisites(&probe, content(), POK, &a());
    let walled = crate::technology::researchable(&probe, content(), POK, &a()).into_iter().find(|t| !plain.contains(t) && t.as_str() == "cr2").expect("Cruiser II opens by the waiver alone");
    (walled, plain.into_iter().next().expect("a technology by prerequisites"))
}

#[test]
fn riftmeld_returns_a_captured_cruiser_to_research_cruiser_ii_in_lockstep() {
    let setup = cabal(&[("b", "cruiser"), ("c", "carrier")], 6);
    let (walled, _) = waiver_only_and_plain(&setup);
    // The primary: the free research asks the waiver and the payment; the paid research follows and is declined.
    let (counts, after) = play(&CABAL, setup.clone(), &[walled.as_str(), "waiver|0", "captured|b|cruiser", "decline"], true);
    assert_eq!(counts, (1, 0), "compared, not declined");
    assert!(owns(&after, "a", walled.as_str()), "the technology is gained");
    assert_eq!(captured(&after), vec![("c".to_owned(), "carrier".to_owned())], "the captured cruiser went home, the carrier stays");
}

#[test]
fn a_declined_waiver_or_payment_of_the_free_research_offers_the_rest_without_the_technology() {
    let setup = cabal(&[("b", "cruiser")], 6);
    let (walled, plain) = waiver_only_and_plain(&setup);
    for answers in [
        [walled.as_str(), "decline", plain.as_str(), "decline"],
        [walled.as_str(), "waiver|0", "decline", plain.as_str()],
    ] {
        let (counts, after) = play(&CABAL, setup.clone(), &answers, true);
        assert_eq!(counts, (1, 0), "{answers:?}");
        assert!(!owns(&after, "a", walled.as_str()), "{answers:?}: nothing was researched by the waiver");
        assert!(owns(&after, "a", plain.as_str()), "{answers:?}: the other technology is");
        assert_eq!(captured(&after).len(), 1, "{answers:?}: the model stays captured");
    }
}

/// The answers of a secondary's bill paid with the four trade goods, one face at a time.
const FOUR_GOODS: [&str; 4] = ["trade_good"; 4];

fn with_bill<'a>(tech: &'a str, rest: &[&'a str]) -> Vec<&'a str> {
    std::iter::once(tech).chain(FOUR_GOODS).chain(rest.iter().copied()).collect()
}

#[test]
fn the_paid_research_pays_then_asks_the_waiver_and_a_decline_gives_the_bill_back() {
    let setup = cabal(&[("b", "cruiser")], 4);
    let (walled, _) = waiver_only_and_plain(&setup);
    // The secondary: the bill is paid before the waiver is asked, as the string does; the waiver is accepted.
    let (counts, after) = play(&CABAL, setup.clone(), &with_bill(walled.as_str(), &["waiver|0", "captured|b|cruiser"]), false);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", walled.as_str()));
    assert!(captured(&after).is_empty());
    assert_eq!(after.player(&a()).unwrap().trade_goods, 0, "the four resources were paid");
    // Declined at the waiver or at its payment: the position from before the bill is restored, the technology is no longer offered, the
    // research is declined at last.
    for rest in [&["decline", "decline"][..], &["waiver|0", "decline", "decline"][..]] {
        let (counts, after) = play(&CABAL, setup.clone(), &with_bill(walled.as_str(), rest), false);
        assert_eq!(counts, (1, 0), "{rest:?}");
        assert!(!owns(&after, "a", walled.as_str()), "{rest:?}");
        assert_eq!(after.player(&a()).unwrap().trade_goods, 4, "{rest:?}: the bill was given back");
        assert_eq!(captured(&after).len(), 1, "{rest:?}");
    }
}

/// Yin's commander in the hands of `a` (a seat of any faction), the technology owned by another seat and an infantry standing in the space
/// area and on a planet of the home system.
fn omar(s: &mut GameState) {
    s.player_mut(&a()).unwrap().leaders.insert(LeaderId::new("yincommander"), LeaderStatus::Unlocked);
    s.player_mut(&a()).unwrap().trade_goods = 6;
    s.player_mut(&PlayerId::new("b")).unwrap().technologies.insert(TechnologyId::new("cr2"));
    let home = s.board.iter().find(|(_, b)| b.units.iter().any(|u| u.owner == a())).map(|(id, b)| (id.clone(), b.planet_units.keys().next().cloned()));
    let (system, planet) = home.expect("the seat has ships at home");
    crate::fixtures::put(s, &system, "infantry", &a(), 1);
    if let Some(planet) = planet {
        crate::fixtures::put_on_planet(s, &system, &planet, "infantry", &a(), 1);
    }
}

fn payment_ids(state: &GameState, index: usize) -> Vec<String> {
    let offers = crate::technology::research_waiver_offers(state, content(), &a(), &TechnologyId::new("cr2"));
    offers.into_iter().find(|(i, _)| *i == index).map(|(_, w)| w.payments.into_iter().map(|p| p.id).collect()).unwrap_or_default()
}

#[test]
fn brother_omar_returns_an_infantry_from_space_or_from_a_planet_in_lockstep() {
    let mut probe = crate::fixtures::seated_game(&YIN, POK);
    omar(&mut probe);
    let payments = payment_ids(&probe, 0);
    assert!(payments.iter().any(|p| p.starts_with("space|")) && payments.iter().any(|p| p.starts_with("ground|")), "{payments:?}");
    for pay in payments {
        let (counts, after) = play(&YIN, omar, &["cr2", "waiver|0", &pay, "decline"], true);
        assert_eq!(counts, (1, 0), "{pay}");
        assert!(owns(&after, "a", "cr2"), "{pay}");
    }
}

/// Two modules, two waivers: the Cabal seat that also holds Yin's commander (an alliance) is offered Riftmeld and Brother Omar; the string
/// asks one per module, the answer `waiver|1` is the second.
#[test]
fn two_modules_offer_a_waiver_each_and_the_second_one_is_paid() {
    let both = |s: &mut GameState| {
        omar(s);
        s.player_mut(&a()).unwrap().captured_units = vec![(PlayerId::new("b"), UnitTypeId::new("cruiser"))];
    };
    let mut probe = crate::fixtures::seated_game(&CABAL, POK);
    both(&mut probe);
    let offers = crate::technology::research_waiver_offers(&probe, content(), &a(), &TechnologyId::new("cr2"));
    assert_eq!(offers.len(), 2, "{offers:?}");
    let pay = payment_ids(&probe, 1).into_iter().next().expect("an infantry to return");
    let (counts, after) = play(&CABAL, both, &["cr2", "waiver|1", &pay, "decline"], true);
    assert_eq!(counts, (1, 0));
    assert!(owns(&after, "a", "cr2"));
    assert_eq!(captured(&after).len(), 1, "the captured cruiser stays: the infantry paid");
}
