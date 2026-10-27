//! The movement area runs on `AreaMask` + `dual!` (docs/ENGINE_CORE_STATE.md 3.3): zero differences
//! on real positions, and an injected difference is caught.

use ti4_content::ContentStore;
use ti4_engine::choice::Observed;
use ti4_engine::dual::{self, Area, AreaMask, Backend, Category};
use ti4_engine_core_bridge::workload::template_openings;

/// The report is process-wide.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn movement_on_dual_has_no_differences_on_template_openings() {
    let _guard = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let content = ContentStore::embedded();
    let (positions, _) = template_openings();
    assert!(!positions.is_empty());
    let _ = dual::take_report();
    let mask = AreaMask::STRING.with(Area::Movement, Backend::Dual);
    let answered = dual::with_mask(mask, || {
        let mut n = 0;
        for p in positions.iter().take(12) {
            let seen = Observed::new(&p.state, content, p.sources, Some(&p.galaxy));
            for player in &p.state.players {
                for dest in p.galaxy.system_ids_holding_things().into_iter().take(6) {
                    n += seen.movable_into(&player.id, &ti4_model::id::SystemId::new(dest)).len();
                    let _ = ti4_engine::transactions::are_neighbours(&p.state, &p.galaxy, &player.id, &p.state.players[0].id);
                }
            }
        }
        n
    });
    let report = dual::take_report();
    assert!(report.total_calls() > 100, "{report}");
    assert_eq!(report.total_differences(), 0, "{report}");
    let _ = answered;
}

#[test]
fn an_injected_difference_in_the_movement_path_is_caught() {
    let _guard = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let _ = dual::take_report();
    let before = dual::fail_fast();
    dual::set_fail_fast(false);
    let mask = AreaMask::STRING.with(Area::Movement, Backend::Dual);
    let shown = dual::with_mask(mask, || {
        // The core side answers wrongly: the call returns the string answer and reports the value.
        ti4_engine::dual_typed!(Area::Movement, "injected", Some(2_i64), 1_i64)
    });
    dual::set_fail_fast(before);
    assert_eq!(shown, 1);
    let report = dual::take_report();
    assert_eq!(report.differing(Category::Value), 1, "{report}");
    assert_eq!(report.differences[0].area, Area::Movement);
}
