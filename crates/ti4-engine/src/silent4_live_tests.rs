//! Lane G4: the last loud gap rows turned into native rules, run natively through `live::live_step` against the string bodies they replace, in the turn-flow harness
//! of lane B (`turn_live_tests`): the questions as rendered, the answers, the events and the position must agree.

use ti4_model::state::GameState;

use super::turn_live_tests::{action_table, assert_same, based, pid, reconcile_both, Run};

/// The turn prelude of the string against the native prelude. A card move an ability stages (`ACTION_CARD_TAKEN`) is announced by the string's step before it stops at the action menu;
/// the native side leaves the staged `cards:staged:*` row in the position for the host's next step (the rows are `hooks_cards::StagedCardEvent`s, announced by
/// `reactions::announce_staged_card_events`): the rows are taken out of the native position and counted against the announced events of the string.
fn prelude_both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    let (mut string, mut native) = super::turn_live_tests::prelude_both(state, ids);
    let rows: Vec<String> = native.state["faction_marks"].as_object_mut().map(|marks| {
        let keys: Vec<String> = marks.keys().filter(|k| k.starts_with("cards:staged:")).cloned().collect();
        keys.iter().filter_map(|k| marks.remove(k)).filter_map(|v| v.as_str().map(str::to_owned)).collect()
    }).unwrap_or_default();
    let announced = string.events.iter().filter(|e| *e == "ACTION_CARD_TAKEN").count();
    assert_eq!(rows.iter().filter(|r| r.starts_with("taken|")).count(), announced, "{rows:?}: each card taken is staged by the native side and announced by the string");
    string.events.retain(|e| e != "ACTION_CARD_TAKEN");
    (string, native)
}

// -- Propagation (Nekro) ----------------------------------------------------------------------------------------------------------

/// A table where seat `a` (Nekro) has had `pending` researches replaced by command tokens that the game has not announced yet.
fn propagation_table(pending: u32) -> GameState {
    let mut state = action_table(&[("a", "nekro"), ("b", "hacan"), ("c", "sol")]);
    state.faction_marks.insert("nekro:propagation:a".to_owned(), pending.to_string());
    based(state)
}

#[test]
fn a_replaced_research_gives_three_command_tokens_into_the_pools_of_the_nekro_like_the_string() {
    for (pending, answers) in [
        (1_u32, vec!["tactic_tokens", "fleet_tokens", "strategic_tokens"]),
        (1, vec!["tactic_tokens", "tactic_tokens", "tactic_tokens"]),
        (2, vec!["fleet_tokens", "fleet_tokens", "fleet_tokens", "strategic_tokens", "tactic_tokens", "tactic_tokens"]),
    ] {
        let state = propagation_table(pending);
        let (string, native) = reconcile_both(&state, &answers);
        assert_same(&string, &native);
        assert!(string.events.iter().any(|e| e == "PROPAGATION_RESEARCH"), "{:?}", string.events);
        assert_eq!(string.questions.len(), 3 * pending as usize, "three pool questions per replaced research");
        assert!(string.state["faction_marks"].get("nekro:propagation:a").is_none(), "the pending count is settled");
        let seat = |run: &super::turn_live_tests::Run| run.state["players"][0].clone();
        let (before, after) = (seat(&super::turn_live_tests::string_run(&state, &[], |_| {})), seat(&string));
        let total = |p: &serde_json::Value| ["tactic_tokens", "fleet_tokens", "strategic_tokens"].iter().map(|k| p[k].as_u64().unwrap_or(0)).sum::<u64>();
        assert_eq!(total(&after) - total(&before), 3 * u64::from(pending), "{answers:?}");
    }
    // Nothing owed: nothing is announced.
    let (string, native) = reconcile_both(&propagation_table(0), &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().all(|e| e != "PROPAGATION_RESEARCH"));
}

// -- Spy Net (Yssaril) ------------------------------------------------------------------------------------------------------------

/// A table where seat `a` (Hacan) holds the Yssaril's Spy Net, seat `b` is the Yssaril holding `hand`, and `a` holds `own` action cards.
fn spy_table(hand: &[&str], own: &[&str]) -> GameState {
    use ti4_model::id::ActionCardId;
    let mut state = action_table(&[("a", "hacan"), ("b", "yssaril"), ("c", "sol")]);
    state.promissory_notes.insert("spynet:yssaril".to_owned(), pid("a"));
    state.player_mut(&pid("b")).unwrap().action_cards = hand.iter().map(|c| ActionCardId::new(*c)).collect();
    state.player_mut(&pid("a")).unwrap().action_cards = own.iter().map(|c| ActionCardId::new(*c)).collect();
    based(state)
}

#[test]
fn the_holder_of_spy_net_takes_a_card_of_the_yssaril_hand_at_the_start_of_the_turn_like_the_string() {
    const WINDOW: &str = "promissory:hacan:spynet:TURN_BEGAN:after";
    // Several printed cards (a question), one (taken unasked), a duplicate pair (one option), a decline of the window.
    for (hand, answers) in [
        (vec!["sabo1", "insub", "reverse_engineer"], vec![WINDOW, "insub"]),
        (vec!["sabo1", "insub", "reverse_engineer"], vec![WINDOW, "sabo1"]),
        (vec!["sabo1"], vec![WINDOW]),
        (vec!["sabo1", "sabo2"], vec![WINDOW]),
        (vec!["sabo1", "insub"], vec!["decline"]),
    ] {
        let state = spy_table(&hand, &[]);
        let (string, native) = prelude_both(&state, &answers);
        assert_same(&string, &native);
        let taken = answers.first() == Some(&WINDOW);
        let hand_of = |run: &super::turn_live_tests::Run, seat: usize| run.state["players"][seat]["action_cards"].as_array().map_or(0, Vec::len);
        assert_eq!(hand_of(&string, 0), usize::from(taken), "{hand:?} {answers:?}: one card is taken only by a use");
        assert_eq!(string.state["promissory_notes"].get("spynet:yssaril").is_none_or(|holder| holder != "a"), taken, "{hand:?} {answers:?}: the note goes home only after a use");
    }
    // Nothing to look at: the window is not offered.
    let (string, native) = prelude_both(&spy_table(&[], &[]), &[]);
    assert_same(&string, &native);
}

#[test]
fn spy_net_over_the_hand_limit_asks_the_discard_like_the_string() {
    const WINDOW: &str = "promissory:hacan:spynet:TURN_BEGAN:after";
    let full = ["coup", "dh1", "industrial_initiative", "investments", "reverse_engineer", "sabo1", "sabo2"];
    let state = spy_table(&["insub", "sabo1"], &full);
    let (string, native) = prelude_both(&state, &[WINDOW, "insub", "0"]);
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.prompt.starts_with("over the hand limit")), "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
}

// -- Iconoclast (the Naalu Thunder's Edge mech) ------------------------------------------------------------------------------------

/// A table where seat `a` (Naalu) controls its two home planets, seat `b` has gained a relic the game has not announced yet; `mechs_out` of the Naalu mechs stand elsewhere.
fn relic_table(gainer: &str, mechs_out: usize) -> GameState {
    use ti4_content::ContentStore;
    let state = action_table(&[("a", "naalu"), ("b", "hacan"), ("c", "sol")]);
    // The first look records the starting set; a relic gained after it is announced.
    let mut game = crate::game::Game::with_table(state, ContentStore::embedded(), crate::choice::Table::with_default(Box::new(crate::choice::FirstOption)));
    game.prepared_turn_seq = Some(game.state.turn_seq);
    let _ = game.step();
    let mut state = game.state.clone();
    state.player_mut(&pid(gainer)).unwrap().relics.push(ti4_model::id::RelicId::new("dynamiscore"));
    if mechs_out > 0 {
        crate::fixtures::put(&mut state, &ti4_model::id::SystemId::new(crate::seating::MECATOL), "naalu_mech_te", &pid("a"), mechs_out);
    }
    state
}

#[test]
fn another_players_relic_lets_the_naalu_deploy_the_thunders_edge_mech_like_the_string() {
    const WINDOW: &str = "unit:naalu:naalu_mech_te:RELIC_GAINED:after";
    let home_planets = |state: &GameState| -> Vec<String> {
        let home = state.player(&pid("a")).unwrap().home_system.clone().unwrap();
        state.system_state(&home).planet_control.iter().filter(|(_, owner)| **owner == pid("a")).map(|(planet, _)| format!("{home}|{planet}")).collect()
    };
    let probe = relic_table("b", 0);
    let spots = home_planets(&probe);
    assert!(spots.len() >= 2, "the Naalu home has several planets: {spots:?}");
    for (label, gainer, out, answers) in [
        ("use, the first planet", "b", 0, vec![WINDOW.to_owned(), spots[0].clone()]),
        ("use, the second planet", "b", 0, vec![WINDOW.to_owned(), spots[1].clone()]),
        ("decline", "b", 0, vec!["decline".to_owned()]),
        ("the Naalu gain it themselves", "a", 0, vec![]),
        ("no mech in the box", "b", 4, vec![WINDOW.to_owned()]),
    ] {
        let state = relic_table(gainer, out);
        let answers: Vec<&str> = answers.iter().map(String::as_str).collect();
        // The string game plays under the Thunder's Edge sources, as the native kernel does (the printing of the mech the Naalu place).
        let string = super::turn_live_tests::string_run(&state, &answers, |g| {
            g.sources = ti4_model::content_types::DEFAULT;
            g.prepared_turn_seq = Some(g.state.turn_seq);
            let _ = g.step();
        });
        let (_, native) = reconcile_both(&state, &answers);
        let mut string = string;
        if string.questions.last().is_some_and(|q| q.context.as_ref().is_some_and(|c| c.subtype == "action_menu")) {
            string.questions.pop();
        }
        assert_same(&string, &native);
        let offered = string.questions.iter().any(|q| q.options.iter().any(|o| o.id == WINDOW));
        assert_eq!(offered, gainer == "b", "{label}: the window is offered for another player's relic only");
        let mechs = |run: &super::turn_live_tests::Run| run.state["board"].as_object().map_or(0, |b| b.values().map(|s| s["planet_units"].as_object().map_or(0, |pu| pu.values().filter_map(|u| u.as_array()).flatten().filter(|u| u["type_id"] == "naalu_mech_te" && u["owner"] == "a").count())).sum::<usize>());
        let before = mechs(&super::turn_live_tests::string_run(&state, &[], |_| {}));
        let expected = usize::from(label.starts_with("use"));
        assert_eq!(mechs(&string) - before, expected, "{label}");
    }
}

// -- M'aban (the Naalu commander): the standing permission to look at the promissory notes of the neighbours ---------------------------

#[test]
fn the_core_hook_may_view_hand_answers_like_the_strings_for_the_maban_permission() {
    use crate::core_sync::{Codec, Ctx, Sources};
    use crate::factions::hooks_cards::RevealKind as SKind;
    use ti4_engine_core::kernel::hooks::RevealKind as CKind;
    use ti4_engine_core::state::SliceSet;
    use ti4_model::state::LeaderStatus;
    let content = ti4_content::ContentStore::embedded();
    // A hub whose centre holds the Naalu's units and whose first ring system holds the Hacan's: neighbours; the Sol has no unit on the board and is not.
    let hub = crate::fixtures::plain_hub();
    let (centre, near) = (ti4_model::id::SystemId::new(hub.centre.clone()), ti4_model::id::SystemId::new(hub.outer[0].clone()));
    for (label, status, neighbours) in [("unlocked, neighbours", Some(LeaderStatus::Unlocked), true), ("locked", Some(LeaderStatus::Locked), true), ("unlocked, no neighbour units", Some(LeaderStatus::Unlocked), false), ("no commander", None, true)] {
        let mut state = action_table(&[("a", "naalu"), ("b", "hacan"), ("c", "sol")]);
        for system in state.board.values_mut() {
            system.units.clear();
            system.planet_units.clear();
        }
        crate::fixtures::put(&mut state, &centre, "cruiser", &pid("a"), 1);
        if neighbours {
            crate::fixtures::put(&mut state, &near, "cruiser", &pid("b"), 1);
        }
        match status {
            Some(status) => {
                state.player_mut(&pid("a")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("naalucommander"), status);
            }
            None => {
                state.player_mut(&pid("a")).unwrap().leaders.remove(&ti4_model::id::LeaderId::new("naalucommander"));
            }
        }
        let state = based(state);
        let mut cx = Ctx::of(&state).expect("a table the codecs carry");
        let positions = crate::game::Game::with_table(state.clone(), ti4_content::ContentStore::embedded(), crate::choice::Table::with_default(Box::new(crate::choice::FirstOption))).rng_positions();
        let core = Codec::load(&mut cx, &state, Sources { galaxy: Some(&hub.galaxy), rng: Some(&positions) }, SliceSet::ALL).expect("the position loads");
        let kernel = crate::live::kernel::kernel_for(ti4_model::content_types::DEFAULT);
        for (viewer, owner) in [("a", "b"), ("a", "c"), ("b", "a"), ("a", "a")] {
            for (skind, ckind) in [(SKind::PromissoryNotes, CKind::PromissoryNotes), (SKind::ActionCards, CKind::ActionCards), (SKind::SecretObjectives, CKind::SecretObjectives)] {
                let string = crate::factions::hooks_cards::may_view_hand(&state, content, ti4_model::content_types::DEFAULT, Some(&hub.galaxy), &pid(viewer), &pid(owner), skind);
                let seat = |name: &str| ti4_engine_core::Seat(u8::try_from(["a", "b", "c"].iter().position(|n| *n == name).unwrap()).unwrap());
                let native = kernel.hooks.cards.may_view_hand(&core, kernel, seat(viewer), seat(owner), ckind);
                assert_eq!(string, native, "{label}: {viewer} looks at {owner}'s {skind:?}");
                if label == "unlocked, neighbours" && viewer == "a" && owner == "b" && skind == SKind::PromissoryNotes {
                    assert!(string, "the permission is given in the case that is meant to give it");
                }
            }
        }
    }
}

// -- Jae Mir Kan (the Mahact agent): the waiver the follower takes in place of a strategy token -------------------------------------

#[test]
fn the_core_hook_secondary_waived_pays_jae_mir_kan_like_the_string() {
    use crate::core_sync::{materialize, Codec, Ctx, Sources};
    use ti4_engine_core::state::SliceSet;
    use ti4_model::id::{LeaderId, SystemId, TechnologyId};
    use ti4_model::state::LeaderStatus;
    let content = ti4_content::ContentStore::embedded();
    let hub = crate::fixtures::plain_hub();
    let system = SystemId::new(hub.outer[1].clone());
    let agent = LeaderId::new(crate::factions::mahact::AGENT);
    for (label, watching, status, token) in [
        ("ready", false, LeaderStatus::Readied, true),
        ("ready, the Nomad watches", true, LeaderStatus::Readied, true),
        ("exhausted already", false, LeaderStatus::Exhausted, true),
        ("no token there", false, LeaderStatus::Readied, false),
    ] {
        let mut state = action_table(&[("a", "sol"), ("b", "mahact"), ("c", "nomad")]);
        state.player_mut(&pid("b")).unwrap().leaders.insert(agent.clone(), LeaderStatus::Readied);
        if token {
            state.system_mut(&system).command_tokens.insert(pid("a"));
        }
        if watching {
            state.player_mut(&pid("c")).unwrap().technologies.insert(TechnologyId::new("tcs"));
        }
        let state = based(state);
        // The waiver is offered while the agent is ready; the follower takes it after it was exhausted by something else only in the exhausted case.
        let offered = crate::factions::hooks_strategy::secondary_waivers(&state, content, &pid("b"), &pid("a"), None);
        let mut state = state;
        if status == LeaderStatus::Exhausted {
            state.player_mut(&pid("b")).unwrap().leaders.insert(agent.clone(), LeaderStatus::Exhausted);
        }
        let (index, waiver) = offered.iter().find(|(_, w)| w.id == format!("{}{}", crate::factions::mahact::AGENT_WAIVER_PREFIX, system)).map_or((0, String::new()), |(i, w)| (*i, w.id.clone()));
        if !token {
            assert!(waiver.is_empty(), "{label}: no token, no waiver offered");
        }
        let waiver = if waiver.is_empty() { format!("{}{}", crate::factions::mahact::AGENT_WAIVER_PREFIX, system) } else { waiver };
        // The string pays.
        let mut string = state.clone();
        crate::factions::hooks_strategy::secondary_waived(&mut string, content, &pid("b"), &pid("a"), index, &waiver);
        // The core hook pays.
        let mut cx = Ctx::of(&state).expect("a table the codecs carry");
        let positions = crate::game::Game::with_table(state.clone(), ti4_content::ContentStore::embedded(), crate::choice::Table::with_default(Box::new(crate::choice::FirstOption))).rng_positions();
        let mut core = Codec::load(&mut cx, &state, Sources { galaxy: Some(&hub.galaxy), rng: Some(&positions) }, SliceSet::ALL).expect("the position loads");
        let kernel = crate::live::kernel::kernel_for(ti4_model::content_types::DEFAULT);
        let mut asker = ti4_engine_core::kernel::FirstOption;
        let seat = |name: &str| ti4_engine_core::Seat(u8::try_from(["a", "b", "c"].iter().position(|n| *n == name).unwrap()).unwrap());
        let system_index = <ti4_engine_core::generated::System as crate::generated::IdStr>::from_id(system.as_str()).unwrap().index();
        {
            let mut hook_cx = ti4_engine_core::kernel::Cx::new(&mut core, kernel, &mut asker);
            kernel.hooks.strategy.secondary_waived(&mut hook_cx, seat("b"), seat("a"), ti4_engine_core::hooks::Waiver { module: ti4_engine_core::generated::Faction::TheMahactGeneSorcerers, code: u16::try_from(system_index).unwrap() });
        }
        let native = materialize::materialize(&core, &cx, &state).expect("materializes");
        let staged_core: Vec<String> = core.pending().staged.iter().map(|e| format!("{:?}", e.kind)).collect();
        let mut string_json = serde_json::to_value(&string).unwrap();
        let mut staged_string = Vec::new();
        if let Some(marks) = string_json.get_mut("faction_marks").and_then(serde_json::Value::as_object_mut) {
            let keys: Vec<String> = marks.keys().filter(|k| k.starts_with("private:#staged:event:")).cloned().collect();
            for key in keys {
                let text = marks.remove(&key).and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
                staged_string.push(serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|e| e["type"].as_str().map(str::to_owned)).unwrap_or_default());
            }
        }
        let native_json = serde_json::to_value(&native).unwrap();
        let diffs = crate::dual::diff_json(&string_json, &native_json, 12);
        assert!(diffs.is_empty(), "{label}: {diffs:?}");
        assert_eq!(staged_string.iter().map(|s| s.replace('_', "").to_lowercase()).collect::<Vec<_>>(), staged_core.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>(), "{label}: the Nomad's event is staged alike");
        let remains = string.system_state(&system).command_tokens.contains(&pid("a"));
        assert_eq!(remains, token && status != LeaderStatus::Readied, "{label}: the token leaves only when the agent could be exhausted");
        assert_eq!(staged_core.len(), usize::from(watching && token && status == LeaderStatus::Readied), "{label}: the Nomad is told once, when the agent is exhausted");
    }
}
