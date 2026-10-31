// Lane G4: abilities of the tactical action run natively against the string's (`include!`d into `combat_live::tests`, which owns the harness).

/// Tungstantus (the Titans commander, carried by a Hacan seat: the Titans seat itself stops at Awaken, lane M6d): a trade good for the production of a tactical action; the whole action is compared, the answer being the use, a decline, or the
/// seat not holding the commander (nothing is offered).
#[test]
fn the_titans_commander_gains_a_trade_good_when_production_is_used_like_the_string() {
    use ti4_model::id::LeaderId;
    use ti4_model::state::LeaderStatus;
    let (hub, origin, target, dock, _other) = dock_hub();
    for (label, status, decline) in [("use", Some(LeaderStatus::Unlocked), false), ("decline", Some(LeaderStatus::Unlocked), true), ("locked", Some(LeaderStatus::Locked), false), ("none", None, false)] {
        for builds in [0, 1] {
            let mut state = dock_table("hacan", "sol", &target, &dock);
            crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
            if let Some(status) = status {
                state.player_mut(&a()).unwrap().leaders.insert(LeaderId::new("titanscommander"), status);
            } else {
                state.player_mut(&a()).unwrap().leaders.remove(&LeaderId::new("titanscommander"));
            }
            let moved = mv(&origin, 0);
            let script: Vec<&str> = if decline { vec![TACTICAL_ACTION_ID, target.as_str(), &moved, "done_moving", "decline"] } else { vec![TACTICAL_ACTION_ID, target.as_str(), &moved] };
            assert_eq!(string_offered(&state, &hub.galaxy, &script, builds, "titanscommander:PRODUCTION_USED"), status == Some(LeaderStatus::Unlocked), "{label}: the string offers the commander only unlocked");
            let diffs = lockstep_buying(state, &hub.galaxy, &script, builds);
            assert!(diffs.is_empty(), "{label} buying {builds}: {diffs:#?}");
        }
    }
}
