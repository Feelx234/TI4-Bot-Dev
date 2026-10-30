//! The typed action menu and closing menu in lockstep with the string's (package D1b2): a whole round of a seeded game on
//! `driver=dual`, every menu the table is asked rendered from `choice::turn` and compared with the string's question.

use ti4_content::ContentStore;
use ti4_model::id::PlayerId;

use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
use crate::game::Game;

fn round_of_play(seed: u64) -> (usize, crate::dual::DualReport) {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    let players = [PlayerId::new("a"), PlayerId::new("b"), PlayerId::new("c")];
    let state = crate::setup::start_game(ContentStore::embedded(), &players, ti4_model::content_types::POK, None).unwrap();
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
    assert!(steps > 50, "a round was played: {steps}");
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences.first());
    let menus = r.windows.get("turn::menu.choice").map_or(0, |w| w.compared);
    let closings = r.windows.get("turn::closing.choice").map_or(0, |w| w.compared);
    assert!(menus >= 6, "the action menu was compared: {:?}", r.windows);
    assert!(closings >= 1, "the closing menu was compared: {:?}", r.windows);
}
