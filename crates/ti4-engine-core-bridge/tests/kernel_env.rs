//! KF3: the `KernelEnv` of the core answers what the string engine's views answer (trap T12), over the opening positions of the
//! maps the host knows and a position from the middle of a golden game. The full gate is `board_equivalence`-style over the golden
//! games; this is its smoke test.

use ti4_engine_core_bridge::check_hooks::{hook_positions, hook_variants};
use ti4_engine_core_bridge::check_kernel_env::{check_position, check_variants, EnvReport};
use ti4_engine_core_bridge::workload::template_openings;

#[test]
fn the_kernel_env_agrees_with_the_string_views_on_the_template_openings() {
    let mut report = EnvReport::default();
    let (positions, _) = template_openings();
    for p in positions.iter().take(3) {
        check_position(p, &mut report);
    }
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    // The hook slots no faction module fills yet are reported, not hidden (lane F2 ports them; this list shrinks to empty).
    println!("hook slots with no core function where the string answers: {:?}", report.gaps);
    assert_eq!(report.unmapped, 0);
    assert!(report.answers > 5000, "{} answers", report.answers);
}

/// The leaders whose string function belongs to a dropped faction (Last Bastion, Crimson, Keleres, Deepwrought, Firmament, Obsidian, Ral Nel):
/// the only hook slots the core is allowed to leave empty.
const DROPPED: [&str; 11] = [
    "commander_unlocked bastioncommander",
    "commander_unlocked crimsoncommander",
    "commander_unlocked deepwroughtcommander",
    "commander_unlocked firmamentcommander",
    "commander_unlocked obsidiancommander",
    "leader_action crimsonagent",
    "leader_action deepwroughthero",
    "leader_action firmamenthero",
    "leader_action keleresheroharka",
    "leader_action obsidianhero",
    "leader_action ralnelagent",
];

#[test]
fn no_hook_slot_is_empty_but_for_the_dropped_factions() {
    let mut report = EnvReport::default();
    let (positions, _) = template_openings();
    for p in positions.iter().take(3) {
        check_position(p, &mut report);
    }
    let gaps: Vec<&String> = report.gaps.keys().filter(|k| !DROPPED.contains(&k.as_str())).collect();
    assert!(gaps.is_empty(), "hook slots with no core function: {gaps:?}");
}

/// The 12 commanders and 17 leader actions of the in-scope factions, as the string names them.
const COMMANDERS: [&str; 12] = [
    "arboreccommander", "argentcommander", "cabalcommander", "empyreancommander", "ghostcommander", "mahactcommander", "mentakcommander", "muaatcommander",
    "nekrocommander", "nomadcommander", "saarcommander", "winnucommander",
];
const ACTIONS: [&str; 17] = [
    "arborecagent", "arborechero", "argenthero", "cabalhero", "empyreanhero", "ghosthero", "mahacthero", "muaatagent", "naazhero", "nekroagent", "nekrohero",
    "nomadhero", "saarhero", "titanshero", "winnuhero", "yinhero", "yssarilhero",
];

#[test]
fn the_hook_slots_agree_with_the_string_on_positions_that_reach_every_condition() {
    let mut report = EnvReport::default();
    let positions = hook_positions();
    assert_eq!(positions.len(), 3, "the three tables build");
    for p in &positions {
        let variants = hook_variants(p);
        check_variants(p, variants, &mut report);
    }
    assert!(report.mismatches.is_empty(), "{:#?}", report.mismatches);
    assert_eq!(report.unmapped, 0);
    let gaps: Vec<&String> = report.gaps.keys().filter(|k| !DROPPED.contains(&k.as_str())).collect();
    assert!(gaps.is_empty(), "hook slots with no core function: {gaps:?}");
    // The comparison is worth something only where the string answered both ways.
    let mut one_sided = Vec::new();
    for (method, ids) in [("commander_unlocked", COMMANDERS.as_slice()), ("leader_action", ACTIONS.as_slice())] {
        for id in ids {
            let counts = report.outcomes.get(&format!("{method} {id}")).copied().unwrap_or([0, 0]);
            // Infantry have no plastic limit: Dannel's hero is `true` for every seat (the reinforcements never run out of them).
            let by_construction = *id == "yinhero" && counts[1] > 0;
            if !by_construction && (counts[0] == 0 || counts[1] == 0) {
                one_sided.push(format!("{method} {id}: false {} true {}", counts[0], counts[1]));
            }
        }
    }
    assert!(one_sided.is_empty(), "answered one way only: {one_sided:#?}");
}
