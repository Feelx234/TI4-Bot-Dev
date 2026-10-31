//! The engine half of the native tactical action (lane M6, `docs/M6_RESUME.md`): the words of the positions the core composer does not run, and
//! the lockstep tests that run the same tactical action twice, once as the string `TacticalWindow` (`Game::step` with a scripted table) and once as
//! the native `Arm::Tactical` through `live_step`, and compare the questions, the answers, the events and the positions.

use ti4_engine_core::kernel::ability::TacticalGap;

/// The words of a position the native tactical action does not run (`Unknown::UnportedTactical`).
#[must_use]
pub const fn gap_text(gap: TacticalGap) -> &'static str {
    match gap {
        TacticalGap::NoProgress => "the tactical action made no progress (a stage that neither asks nor moves on)",
        TacticalGap::ActivationHook => "a hook of the activation (Spatial Conduit Cylinders, Scanlink Drone Network, E-Res Siphons, Nullification Field, Mahact, Minister of Peace, the Dominus Orb, Ceasefire)",
        TacticalGap::NovaSeed => "Nova Seed replaced a tile while the movement step closed",
        TacticalGap::CannonHits => "a hit of the space cannon offense that needs a question",
        TacticalGap::ExplorationAtTheEnd => "the Crown of Emphidia or Dark Energy Tap explores at the end of the action (no native planet exploration window yet)",
        TacticalGap::WarfareSecondary => "Thunder's Edge Warfare's follower window waits for the end of the free tactical action",
        TacticalGap::LeaderStrategy => "a hero's strategy primary waits for the end of the free tactical action",
        TacticalGap::SsruuLends => "a Yssaril seat could copy an agent that listens to the end of the action (Ssruu)",
        TacticalGap::NoGalaxy => "the map slice holds no galaxy",
    }
}

use serde_json::json;
use ti4_engine_core::choice::tactical::{Ask as TAsk, Opt as TOpt};
use ti4_engine_core::choice::{Ask, Choice as CoreChoice, Opt as COpt};
use ti4_engine_core::driver::windows::tactical::fleet_limit;
use ti4_engine_core::generated::{Unit, System};
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::fleet::{self, Arrival};
use ti4_engine_core::query::reach::{scope_of, SeatFleet, SeatReach};
use ti4_engine_core::state::{CoreState, TStage, Window as OpenWindow, WindowKind};

use crate::choice::Choice;
use crate::preview::{Delta, Preview, Quantity};

/// The display facts of a tactical question that only the position knows (`Choice.details` and the option previews), computed from the core
/// alone: the string builds them in `activation_options_with`, `preview_moves` and `Game::tactical_choice`.
pub fn complete(core: &CoreState, k: &Kernel, choice: &CoreChoice, rendered: &mut Choice) {
    let Ask::Tactical(ask) = choice.ask else { return };
    match ask {
        TAsk::Activate { seat } => {
            let tokens = core.players().by_seat(seat).map_or(0, |p| i64::from(p.econ.tactic_tokens));
            let spend = Preview::certain(vec![Delta::new(Quantity::TacticTokens, tokens, tokens - 1)]);
            for option in &mut rendered.options {
                option.preview = Some(spend.clone());
            }
        }
        TAsk::Move { seat } => move_details(core, k, seat, choice, rendered),
        TAsk::Load { seat, .. } => {
            let Some(OpenWindow::Tactical(t)) = core.pending().window(WindowKind::Tactical) else { return };
            let TStage::Loading { path, .. } = &t.stage else { return };
            let exits = rifts_on(core, k, seat, path.as_slice());
            if exits > 0 {
                rendered.details.insert("rift_exits".to_owned(), exits.into());
            }
        }
        // The string's removal question carries no context (`render::tactical`).
        TAsk::Remove { .. } => rendered.context = None,
    }
}

/// The gravity rifts a route exits for `seat` (`Game::route_rift_exits`): none when the ship ignores anomalies or rifts.
fn rifts_on(core: &CoreState, k: &Kernel, seat: ti4_engine_core::Seat, path: &[System]) -> usize {
    let Some(active) = core.meta().active_system else { return 0 };
    let Some(reach) = SeatReach::new(core, k, seat, true) else { return 0 };
    let rules = reach.rules_for(core, k, active);
    if rules.anomalies_ignored || rules.rifts_ignored {
        0
    } else {
        rules.rifts_exited(path)
    }
}

/// `tactical::preview_moves` and the `movable` count of `movement_options`.
fn move_details(core: &CoreState, k: &Kernel, seat: ti4_engine_core::Seat, choice: &CoreChoice, rendered: &mut Choice) {
    let Some(active) = core.meta().active_system else { return };
    let scope = scope_of(k);
    let limit = i64::from(fleet_limit(core, k, seat));
    let half = |u: Unit| k.hooks.combat.fighter_fleet_weight_halves(core, k, seat, u);
    let before = fleet::standing(core, scope, seat, active, limit, None, &half);
    let Some(reach) = SeatReach::new(core, k, seat, true) else { return };
    let rules = reach.rules_for(core, k, active);
    // `movable`: how many hulls each option stands for.
    let mut hulls = serde_json::Map::new();
    let hull = |origin: System, index: u16| ti4_engine_core::query::board::space_units(core, origin).iter().filter(|p| p.owner() == seat).nth(usize::from(index)).copied();
    let mut keys: Vec<((Unit, bool, System, bool, bool), String)> = Vec::new();
    let ids: Vec<String> = rendered.options.iter().map(|o| o.id.clone()).collect();
    for (i, opt) in choice.opts.iter().enumerate() {
        if let COpt::Tactical(TOpt::Move { origin, index, kind, damaged, gd, ionian, .. }) = opt {
            let _ = index;
            keys.push(((*kind, *damaged, *origin, *gd, *ionian), ids[i].clone()));
        }
    }
    for found in SeatFleet::new(core, k, seat).movable(&rules) {
        let Some(piece) = hull(found.origin, found.index) else { continue };
        let key = (found.kind, piece.sustained(), found.origin, found.gravity_drive, found.ionian);
        if let Some((_, id)) = keys.iter().find(|(kk, _)| *kk == key) {
            let n = hulls.get(id).and_then(serde_json::Value::as_u64).unwrap_or(0);
            hulls.insert(id.clone(), json!(n + 1));
        }
    }
    if !hulls.is_empty() {
        rendered.details.insert("movable".to_owned(), hulls.into());
    }
    let mut rift_map = serde_json::Map::new();
    for (i, opt) in choice.opts.iter().enumerate() {
        let COpt::Tactical(TOpt::Move { origin, kind, gd, ionian, .. }) = opt else { continue };
        let after = fleet::standing(core, scope, seat, active, limit, Some(Arrival { kind: *kind, count: 1, in_space: true }), &half);
        let exits = ti4_engine_core::query::reach::effective_move_value(core, k, seat, *origin, *kind, (*gd, *ionian))
            .and_then(|value| rules.path_from_ship(*origin, value, Some(*kind)))
            .map_or(0, |path| if rules.anomalies_ignored || rules.rifts_ignored { 0 } else { rules.rifts_exited(&path) });
        let preview = if exits > 0 {
            rift_map.insert(ids[i].clone(), json!(exits));
            Preview::unknown("gravity-rift survival is unresolved")
        } else {
            Preview::certain(vec![
                Delta::new(Quantity::FleetSupplyHeadroom, before.fleet_headroom(), after.fleet_headroom()),
                Delta::new(Quantity::CapacityFree, before.capacity_free(), after.capacity_free()),
            ])
        };
        rendered.options[i].preview = Some(preview);
    }
    let mut supply = json!({
        "used": before.fleet_charged,
        "limit": before.fleet_limit,
        "ships": before.ships,
        "capacity": before.transport,
        "carried": before.carried,
        "fighters": before.fighters,
        "fighter_support": before.fighter_support,
        "fighters_upgraded": before.upgraded,
        "fighters_half": before.half_weight,
    });
    let armada = ti4_engine_core::rules::abilities::armada_bonus(core, seat);
    if armada > 0 {
        supply["armada"] = json!(armada);
    }
    if !rift_map.is_empty() {
        rendered.details.insert("rift_exits".to_owned(), rift_map.into());
    }
    rendered.details.insert("fleet_supply".to_owned(), supply);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use serde_json::{json, Value};
    use ti4_content::galaxy::Galaxy;
    use ti4_content::ContentStore;
    use ti4_engine_core::driver::session::Arm;
    use ti4_engine_core::driver::windows::tactical::open_tactical;
    use ti4_engine_core::state::SliceSet;
    use ti4_model::content_types::POK;
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use crate::choice::Choice;
    use crate::core_sync::codec::rng::TextSeeds;
    use crate::core_sync::{materialize, Codec, Ctx, Sources};
    use crate::dual::{diff_json, take_report, with_mask, AreaMask};
    use crate::game::{Game, TACTICAL_ACTION_ID};
    use crate::live::{live_step, Cx};
    use crate::render::Render;

    fn a() -> PlayerId {
        PlayerId::new("a")
    }

    /// A seated table of real factions (notes dealt, no units anywhere), in the action phase with `a` to act.
    fn table(factions: &[(&str, &str)]) -> GameState {
        let mut state = crate::fixtures::seated_game(factions, POK);
        crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
        for system in state.board.values_mut() {
            system.units.clear();
            system.planet_units.clear();
        }
        state.phase = ti4_model::state::Phase::Action;
        state.active = Some(a());
        state
    }

    /// What one side of the lockstep leaves: the questions asked, in order (the string's `legal_options` before each step), the events, the position.
    struct Side {
        asked: Vec<Choice>,
        events: Vec<String>,
        state: Value,
    }

    /// Records every question the string table is asked and answers from the script; a spent script refuses (the step then fails and the
    /// position is what the string restores).
    struct Recorder {
        queue: Rc<RefCell<VecDeque<String>>>,
        asked: Rc<RefCell<Vec<Choice>>>,
    }

    impl crate::choice::Decider for Recorder {
        fn choose(&mut self, c: &Choice) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
            self.asked.borrow_mut().push(c.clone());
            // A spent script takes the first option (the questions after the script are not compared).
            let Some(id) = self.queue.borrow_mut().pop_front().or_else(|| c.options.first().map(|o| o.id.clone())) else {
                return Err(crate::choice::IllegalChoice::NotOffered { player: c.player.clone(), chosen: "<no option>".to_owned(), offered: Vec::new() });
            };
            c.options.iter().find(|o| o.id == id).cloned().ok_or(crate::choice::IllegalChoice::NotOffered { player: c.player.clone(), chosen: id, offered: c.ids().into_iter().map(str::to_owned).collect() })
        }
    }

    /// The string run: `script[0]` answers the action menu (it opens the window), the rest the window's questions, all of them (a question asked
    /// from inside a step included).
    fn string_run(state: &GameState, galaxy: &Galaxy, script: &[&str]) -> Side {
        let _ = take_report();
        let queue = Rc::new(RefCell::new(script.iter().map(|s| (*s).to_owned()).collect::<VecDeque<_>>()));
        let asked = Rc::new(RefCell::new(Vec::new()));
        let table = crate::choice::Table::with_default(Box::new(Recorder { queue: queue.clone(), asked: asked.clone() }));
        let mut game = Game::with_table(state.clone(), ContentStore::embedded(), table).with_galaxy(galaxy.clone());
        with_mask(AreaMask::STRING, || {
            for _ in 0..script.len() + 4 {
                let spent = queue.borrow().is_empty();
                if spent || game.step().error.is_some() {
                    break;
                }
            }
        });
        // The first question is the action menu's.
        let asked = asked.borrow().iter().skip(1).filter(|c| !c.options.is_empty()).cloned().collect::<Vec<_>>();
        let asked = asked.into_iter().take(script.len() - 1).collect();
        Side { asked, events: game.events.clone(), state: serde_json::to_value(&game.state).expect("serialises") }
    }

    /// Answers from one shared queue: the questions of a step (stage and nested) take what the script has next.
    struct Shared {
        queue: Rc<RefCell<VecDeque<String>>>,
        seen: Rc<RefCell<Vec<Choice>>>,
    }

    impl crate::live::Asker for Shared {
        fn ask(&mut self, q: &crate::live::Question<'_>) -> Result<String, crate::live::Stop> {
            self.seen.borrow_mut().push(q.rendered.clone());
            self.queue.borrow_mut().pop_front().ok_or(crate::live::Stop::Exhausted)
        }
    }

    /// The native run of the same script: the window opens as the menu's answer does, then one `live_step` per question until the script is spent.
    fn native_run(state: &GameState, galaxy: &Galaxy, rng: &crate::rng::RngPositions, script: &[&str]) -> Side {
        let mut cx = Ctx::of(state).expect("owners");
        let render = Render::new(cx.owners.ids().to_vec());
        let sources = POK;
        let mut core = Codec::load(&mut cx, state, Sources { galaxy: Some(galaxy), rng: Some(rng) }, SliceSet::ALL).expect("the position loads");
        let seat = render.seat("a").expect("seat a");
        open_tactical(&mut core, seat);
        let kernel = crate::live::kernel::kernel_for(sources);
        let env = crate::live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), sources);
        let live = Cx::new(&render, &env);
        let queue = Rc::new(RefCell::new(script[1..].iter().map(|s| (*s).to_owned()).collect::<VecDeque<_>>()));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut asker = Shared { queue: queue.clone(), seen: seen.clone() };
        for _ in 0..script.len() + 4 {
            if queue.borrow().is_empty() {
                break;
            }
            match live_step(&mut core, Arm::Tactical, &live, &mut asker) {
                Ok(step) if step.finished => break,
                Ok(_) => {}
                Err(e) => panic!("the native step: {e:?}"),
            }
        }
        let asked = seen.borrow().clone();
        let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
        let after = materialize::materialize(&core, &cx, state).expect("materializes");
        Side { asked, events, state: serde_json::to_value(&after).expect("serialises") }
    }

    fn leaves(label: &str, left: &Value, right: &Value) -> Vec<String> {
        diff_json(left, right, 8).into_iter().map(|d| format!("{label}{} string={} native={}", d.path, d.left, d.right)).collect()
    }

    /// Run both and report every difference of the questions, the events (the native ones must be a prefix of the string's when `prefix`) and the position.
    fn lockstep(state: GameState, galaxy: &Galaxy, script: &[&str], prefix: bool) -> Vec<String> {
        let _guard = crate::dual::tests::serial();
        let rng = crate::rng::GameRng::new(state.rng_seed).positions();
        let s = string_run(&state, galaxy, script);
        let n = native_run(&state, galaxy, &rng, script);
        let mut out = Vec::new();
        if s.asked.len() != n.asked.len() {
            out.push(format!("questions: string {} native {}", s.asked.len(), n.asked.len()));
        }
        for (i, (l, r)) in s.asked.iter().zip(&n.asked).enumerate() {
            for (what, lv, rv) in [
                ("player", json!(l.player), json!(r.player)),
                ("prompt", json!(l.prompt), json!(r.prompt)),
                ("options", json!(l.options), json!(r.options)),
                ("details", json!(l.details), json!(r.details)),
                ("context", json!(l.context), json!(r.context)),
            ] {
                out.extend(leaves(&format!("question {i} {what}"), &lv, &rv));
            }
        }
        // The string's first step is the turn prelude (`TURN_BEGAN`), which the native run does not take.
        let s_events: Vec<String> = s.events.iter().filter(|e| *e != "TURN_BEGAN").cloned().collect();
        let left: &[String] = if prefix { &s_events[..n.events.len().min(s_events.len())] } else { &s_events };
        if left != n.events.as_slice() {
            out.push(format!("events: string {:?} native {:?}", s.events, n.events));
        }
        for (i, (l, r)) in s.asked.iter().zip(&n.asked).enumerate() {
            if l.options != r.options {
                out.push(format!("question {i}: the options differ in a preview"));
            }
        }
        if !prefix {
            out.extend(leaves("state", &s.state, &n.state));
        }
        out
    }

    fn hub() -> (crate::fixtures::Hub, ti4_model::id::SystemId, ti4_model::id::SystemId) {
        let hub = crate::fixtures::plain_hub();
        let (origin, target) = (ti4_model::id::SystemId::new(hub.outer[0].clone()), ti4_model::id::SystemId::new(hub.centre.clone()));
        (hub, origin, target)
    }

    #[test]
    fn an_activation_and_a_move_ask_what_the_string_asks_and_leave_the_same_position() {
        let (hub, origin, target) = hub();
        let mut state = table(&[("a", "sol"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        let mv = format!("move|{origin}|0");
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv], false);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }

    fn id(origin: &ti4_model::id::SystemId, index: usize) -> String {
        format!("move|{origin}|{index}")
    }

    #[test]
    fn a_carrier_loads_two_infantry_and_sails_in_lockstep() {
        let (hub, origin, target) = hub();
        let mut state = table(&[("a", "sol"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        crate::fixtures::put(&mut state, &origin, "infantry", &a(), 2);
        let mv = id(&origin, 0);
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv, "load|0", "load|1"], false);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }

    #[test]
    fn done_loading_closes_the_hold_early_in_lockstep() {
        let (hub, origin, target) = hub();
        let mut state = table(&[("a", "sol"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        crate::fixtures::put(&mut state, &origin, "infantry", &a(), 2);
        let mv = id(&origin, 0);
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv, "load|0", "done_loading"], false);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }

    #[test]
    fn finishing_the_movement_step_closes_it_and_hands_over_what_the_string_does_first() {
        let (hub, origin, target) = hub();
        let mut state = table(&[("a", "sol"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        let mv = id(&origin, 0);
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv, "done_moving"], true);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }

    #[test]
    fn the_enforcement_of_the_head_asks_what_the_string_asks() {
        let (hub, origin, target) = hub();
        let mut state = table(&[("a", "sol"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        crate::fixtures::put(&mut state, &target, "cruiser", &a(), 3);
        state.player_mut(&a()).unwrap().fleet_tokens = 1;
        let mv = id(&origin, 0);
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv, "done_moving", "remove|0", "remove|0"], true);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }
}
