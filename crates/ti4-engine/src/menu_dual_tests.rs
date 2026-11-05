//! The typed action menu and closing menu in lockstep with the string's (package D1b2): a whole round of a seeded game on
//! `driver=dual`, every menu the table is asked rendered from `choice::turn` and compared with the string's question.

use ti4_content::ContentStore;

use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
use crate::game::Game;

fn round_of_play(seed: u64) -> (usize, crate::dual::DualReport) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    // Real factions (a table of `generic` seats is outside the codecs).
    let state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "arborec")], ti4_model::content_types::POK);
    let steps = with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
        let mut g = Game::with_seeded_random(state, ContentStore::embedded(), seed);
        let mut steps = 0;
        while g.state.round < 2 && !g.state.finished && steps < 3000 {
            let r = g.step();
            assert!(r.error.is_none(), "{:?}", r.error);
            steps += 1;
        }
        steps
    });
    let r = report();
    let _ = take_report();
    (steps, r)
}

#[test]
fn the_action_menu_and_the_closing_menu_render_from_the_typed_family_exactly_as_the_string_builds_them() {
    let (steps, r) = round_of_play(42);
    assert!(steps > 20, "a round was played: {steps}");
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    let menus = r.windows.get("turn::menu.choice").map_or(0, |w| w.compared);
    let closings = r.windows.get("turn::closing.choice").map_or(0, |w| w.compared);
    assert!(menus >= 3, "the action menu was compared: {:?}", r.windows);
    assert!(closings >= 1, "the closing menu was compared: {:?}", r.windows);
}

#[test]
fn the_hooks_between_the_steps_of_a_turn_end_are_counted_as_lent_by_name() {
    let (_, r) = round_of_play(42);
    for hook in ["technology::end_turn", "legendary::end_turn"] {
        let key = format!("turn::hooks [bail: {hook}]");
        assert!(r.windows.get(&key).is_some_and(|w| w.bailed > 0), "{key}: {:?}", r.windows.keys().filter(|k| k.starts_with("turn::")).collect::<Vec<_>>());
    }
    assert!(r.windows.get("fleet::enforce_everywhere").map_or(0, |w| w.compared) >= 1, "the sweep ran in the core: {:?}", r.windows.keys().collect::<Vec<_>>());
}

/// Golden dual games 4, 10 and 14 (docs/DUAL_REST_RESUME.md, class B): Cabal's Vortex stood in the string's Mapped source and in the core's Faction
/// source (`menu_faction::faction` lists the mapped actions after the plain ones), so the typed menu the dual rendered listed it twice
/// (`options[6]` = `faction|cabal|vortex` against `component|leader|cabalhero`, 26 to 34 menus a game). Neither engine was wrong; the adapter put the
/// string's Mapped ids next to the core's Faction options.
#[test]
fn cabals_vortex_is_listed_once_by_the_typed_menu() {
    use ti4_model::id::{PlayerId, SystemId, TechnologyId};
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let hub = crate::fixtures::plain_hub();
    let (dock_at, target) = (SystemId::new(hub.outer[0].clone()), SystemId::new(hub.centre.clone()));
    let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
    let mut state = crate::fixtures::seated_game(&[("a", "cabal"), ("b", "hacan")], ti4_model::content_types::POK);
    for system in state.board.values_mut() {
        system.units.clear();
        system.planet_units.clear();
    }
    state.phase = ti4_model::state::Phase::Action;
    state.active = Some(a.clone());
    state.player_mut(&a).unwrap().technologies.insert(TechnologyId::new("vtx"));
    crate::fixtures::put(&mut state, &dock_at, "spacedock", &a, 1);
    crate::fixtures::put(&mut state, &target, "cruiser", &b, 1);
    let table = crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new(["pass"])));
    with_mask(AreaMask::STRING.with(Area::Driver, Backend::Dual), || {
        let mut game = Game::with_table(state, ContentStore::embedded(), table).with_galaxy(hub.galaxy.clone());
        let r = game.step();
        assert!(r.error.is_none(), "{:?}", r.error);
    });
    let r = report();
    let _ = take_report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    assert!(r.windows.get("turn::menu.choice").is_some_and(|w| w.compared >= 1), "the action menu was compared: {:?}", r.windows);
}
