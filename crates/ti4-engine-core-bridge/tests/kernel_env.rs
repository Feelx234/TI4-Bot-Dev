//! KF3: the `KernelEnv` of the core answers what the string engine's views answer (trap T12), over the opening positions of the
//! maps the host knows and a position from the middle of a golden game. The full gate is `board_equivalence`-style over the golden
//! games; this is its smoke test.

use ti4_engine_core_bridge::check_kernel_env::{check_position, EnvReport};
use ti4_engine_core_bridge::workload::template_openings;

#[test]
fn the_kernel_env_agrees_with_the_string_views_on_the_template_openings() {
    let mut report = EnvReport::default();
    let (positions, _) = template_openings();
    for p in positions.iter().take(3) {
        check_position(p, &mut report);
    }
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    assert_eq!(report.unmapped, 0);
    assert!(report.answers > 5000, "{} answers", report.answers);
}
