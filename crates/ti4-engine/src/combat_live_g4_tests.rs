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

/// Stymie (the Arborec promissory note, held by seat `b`): after seat `a` moves ships into a system holding a unit of `b`, `b` may place a command token of `a`'s from its reinforcements in a
/// non-home system. The whole action is compared: the window, the system question (several systems on the map, so asked), the placement and the staged event.
#[test]
fn stymie_places_a_command_token_of_the_mover_after_a_move_into_the_holders_units_like_the_string() {
    // A hub of systems that are not home systems (the default ring's ids are the first tiles of the corpus, which are).
    let (system, planet) = crate::fixtures::a_placed_planet();
    let content = ContentStore::embedded();
    let mut ids = vec![system.as_str().to_owned()];
    ids.extend(crate::fixtures::plain_systems(80).into_iter().filter(|id| id != system.as_str() && !ti4_content::galaxy::is_home_system(content, id, POK)).take(6));
    let hub = crate::fixtures::hub_from(&ids);
    let origin = SystemId::new(hub.outer[0].clone());
    let target = system;
    let window = "promissory:sol:stymie:SHIP_MOVED:after";
    for (label, pick) in [("use", Some(0_usize)), ("use the second system", Some(1)), ("decline", None), ("no note", Some(99))] {
        let mut state = table(&[("a", "hacan"), ("b", "sol")]);
        // The string places a frontier token on every planetless system when it is given the map: the native run starts from the position after.
        for id in &ids {
            if ti4_content::galaxy::all_systems(content, POK).iter().any(|(sid, sys)| *sid == id.as_str() && sys.planets().is_empty()) {
                state.frontier_tokens.insert(SystemId::new(id.clone()));
            }
        }
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        crate::fixtures::put_on_planet(&mut state, &target, &planet, "infantry", &b(), 1);
        if pick != Some(99) {
            state.promissory_notes.insert("stymie:arborec".to_owned(), b());
        }
        // The systems the string would offer the holder, in its order.
        let moved = mv(&origin, 0);
        let probe = string_run(&state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &moved, window], 0);
        let question = probe.asked.iter().find(|q| q.prompt.starts_with("Stymie:"));
        let mut script: Vec<String> = vec![TACTICAL_ACTION_ID.to_owned(), target.to_string(), moved.clone()];
        match pick {
            Some(99) => assert!(question.is_none() && !probe.asked.iter().any(|q| q.options.iter().any(|o| o.id == window)), "{label}: no note, no window"),
            None => script.push("decline".to_owned()),
            Some(n) => {
                let q = question.unwrap_or_else(|| panic!("{label}: the string asks where: {:?}", probe.asked.iter().map(|q| q.prompt.clone()).collect::<Vec<_>>()));
                script.push(window.to_owned());
                script.push(q.options[n].id.clone());
            }
        }
        let script: Vec<&str> = script.iter().map(String::as_str).collect();
        let diffs = lockstep_buying(state.clone(), &hub.galaxy, &script, 0);
        assert!(diffs.is_empty(), "{label}: {diffs:#?}");
        if let Some(n @ 0..=1) = pick {
            let tokens = |s: &Side| s.state["board"].as_object().map_or(0, |b| b.values().filter(|sys| sys["command_tokens"].as_array().is_some_and(|t| t.iter().any(|x| x == "a"))).count());
            let after = string_run(&state, &hub.galaxy, &script, 0);
            let before = string_run(&state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &moved, "decline"], 0);
            assert_eq!(tokens(&after), tokens(&before) + 1, "{label}: one more token of the mover is on the map ({n})");
        }
    }
}
