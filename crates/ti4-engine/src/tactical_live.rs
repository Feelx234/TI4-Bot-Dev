//! The engine half of the native tactical action (lane M6, `docs/M6_RESUME.md`): the words of the positions the core composer does not run, and
//! the lockstep tests that run the same tactical action twice, once as the string `TacticalWindow` (`Game::step` with a scripted table) and once as
//! the native `Arm::Tactical` through `live_step`, and compare the questions, the answers, the events and the positions.

use ti4_engine_core::kernel::ability::TacticalGap;

/// The prompt of the Minister of Peace's offer to discard its law (`Game::minister_of_peace`).
pub const PEACE_PROMPT: &str = "Minister of Peace: discard to end the active player's turn";

/// The words of a position the native tactical action does not run (`Unknown::UnportedTactical`).
#[must_use]
pub const fn gap_text(gap: TacticalGap) -> &'static str {
    match gap {
        TacticalGap::NoProgress => "the tactical action made no progress (a stage that neither asks nor moves on)",
        TacticalGap::ActivationHook => "a hook of the activation that the native chain does not carry",
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
        // The hooks of the activation carry no display facts of their own.
        TAsk::Hook { .. } | TAsk::Scanlink { .. } => {}
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

/// The routes of one option as `tactical::MoveRoute` serialises them (a false flag and an empty list are left out).
fn route_json(option_id: &str, routes: &[ti4_engine_core::rules::tactical::routes::Route]) -> serde_json::Value {
    let ids = |list: &[System]| list.iter().map(|s| <System as crate::generated::IdStr>::as_str(*s).to_owned()).collect::<Vec<_>>();
    routes
        .iter()
        .enumerate()
        .map(|(n, r)| {
            let mut m = serde_json::Map::new();
            m.insert("path".to_owned(), json!(ids(&r.path)));
            m.insert("answer".to_owned(), json!(if n == 0 { option_id.to_owned() } else { format!("{option_id}|r{n}") }));
            m.insert("default".to_owned(), json!(r.default));
            if r.gd {
                m.insert("gravity_drive".to_owned(), json!(true));
            }
            if r.ionian {
                m.insert("ionian".to_owned(), json!(true));
            }
            if !r.rifts.is_empty() {
                m.insert("rifts".to_owned(), json!(ids(&r.rifts)));
            }
            if r.wormholes != 0 {
                m.insert("wormholes".to_owned(), json!(r.wormholes));
            }
            if !r.anomalies.is_empty() {
                m.insert("anomalies".to_owned(), json!(ids(&r.anomalies)));
            }
            if !r.pickups.is_empty() {
                m.insert("pickups".to_owned(), json!(ids(&r.pickups)));
            }
            serde_json::Value::Object(m)
        })
        .collect::<Vec<_>>()
        .into()
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
    if core.meta().rules_at_least(ti4_model::state::RULES_MOVEMENT_ROUTES) {
        let mut listed = serde_json::Map::new();
        for (i, opt) in choice.opts.iter().enumerate() {
            let COpt::Tactical(TOpt::Move { origin, index, kind, gd, ionian, .. }) = opt else { continue };
            let routes = ti4_engine_core::rules::tactical::routes::ship_routes(core, k, seat, *origin, *index, *kind, (*gd, *ionian));
            if routes.len() > 1 {
                listed.insert(ids[i].clone(), route_json(&ids[i], &routes));
            }
        }
        if !listed.is_empty() {
            rendered.details.insert("routes".to_owned(), listed.into());
        }
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
        /// The native run stopped on a position the core does not run (a bail), worded.
        bail: Option<String>,
        /// The positions of the random streams after the run (the rift dice).
        rng: String,
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
            // `move|origin|i|rN`: a route of the option, which is not an option of the question; the string reads the suffix off the answer.
            if let Some((base, _)) = crate::tactical::route_suffix(&id)
                && let Some(option) = c.options.iter().find(|o| o.id == base)
            {
                let mut routed = option.clone();
                routed.id = id.clone();
                return Ok(routed);
            }
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
        Side { asked, events: game.events.clone(), state: serde_json::to_value(&game.state).expect("serialises"), bail: None, rng: format!("{:?}", game.rng_positions()) }
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
        let mut bail = None;
        for _ in 0..script.len() + 4 {
            if queue.borrow().is_empty() {
                break;
            }
            match live_step(&mut core, Arm::Tactical, &live, &mut asker) {
                Ok(step) if step.finished => break,
                Ok(_) => {
                    if std::env::var_os("M6B_DEBUG").is_some() {
                        eprintln!("after a step: marks {:?} relics {:?} activation {}", core.players().by_seat(seat).map(|p| p.round_marks.clone()), core.players().by_seat(seat).map(|p| p.cards.relics.clone()), core.meta().activation_seq);
                    }
                }
                Err(crate::live::Stop::Live(ti4_engine_core::driver::live::LiveError::Unknown(ti4_engine_core::kernel::ability::Unknown::UnportedTactical(gap)))) => {
                    bail = Some(super::gap_text(gap).to_owned());
                    break;
                }
                Err(e) => panic!("the native step: {e:?}; asked so far: {:?}; last details {:?}; round marks {:?} activation {}", seen.borrow().iter().map(|c| (c.prompt.clone(), c.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>())).collect::<Vec<_>>(), seen.borrow().last().map(|c| c.details.get("routes").cloned()), core.players().by_seat(seat).map(|p| p.round_marks.clone()), core.meta().activation_seq),
            }
        }
        let asked = seen.borrow().clone();
        let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
        let after = materialize::materialize(&core, &cx, state).expect("materializes");
        let rng = format!("{:?}", crate::core_sync::codec::rng::positions(&cx, core.rng()).expect("every stream has a text"));
        Side { asked, events, state: serde_json::to_value(&after).expect("serialises"), bail, rng }
    }

    fn leaves(label: &str, left: &Value, right: &Value) -> Vec<String> {
        // `private:#seen:*` is the string's own bookkeeping of what each seat has been shown (the per-step reconcile), not a rule.
        diff_json(left, right, 40).into_iter().filter(|d| !d.path.contains("#seen:") && !d.path.contains("frontier_tokens")).take(8).map(|d| format!("{label}{} string={} native={}", d.path, d.left, d.right)).collect()
    }

    /// Run both and report every difference of the questions, the events (the native ones must be a prefix of the string's when `prefix`) and the position.
    fn lockstep(state: GameState, galaxy: &Galaxy, script: &[&str], prefix: bool) -> Vec<String> {
        let _guard = crate::dual::tests::serial();
        let rng = crate::rng::GameRng::new(state.rng_seed).positions();
        let s = string_run(&state, galaxy, script);
        let n = native_run(&state, galaxy, &rng, script);
        let mut out = Vec::new();
        if let Some(why) = &n.bail {
            // Bail as failure: a position the native path does not run is a defect of the island, never a silent lend.
            out.push(format!("BAIL: {why}"));
            return out;
        }
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
        let s_events: Vec<String> = s.events.iter().filter(|e| *e != "TURN_BEGAN" && *e != "FRONTIER_TOKENS_PLACED").cloned().collect();
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
            if s.rng != n.rng {
                out.push(format!("rng positions: string {} native {}", s.rng, n.rng));
            }
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

    /// Bail as failure (`TI4_BAIL_IS_FAILURE`): a real tactical action of every faction named by the brief, native from the activation to the
    /// hand-off to the aftermath, with the factions' own abilities of the movement and tactical windows (Mentak, Argent, Ghost, Naalu,
    /// Sardakk, Xxcha, Hacan, Winnu, ...) in play. The same script runs twice: to the end of the hold (state compared) and through the close.
    #[test]
    fn a_tactical_action_of_each_faction_runs_native_without_a_bail_and_as_the_string_does() {
        let (hub, origin, target) = hub();
        let mut failures = Vec::new();
        for faction in ["mentak", "argent", "ghost", "naalu", "sardakk", "xxcha", "hacan", "winnu", "sol", "letnev", "jolnar", "l1z1x", "arborec", "muaat", "nekro", "saar", "yin", "yssaril", "cabal", "empyrean", "naaz", "nomad", "titans", "mahact"] {
            let other = if faction == "letnev" { "sol" } else { "letnev" };
            let mut state = table(&[("a", faction), ("b", other)]);
            crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
            crate::fixtures::put(&mut state, &origin, "infantry", &a(), 2);
            let mv = id(&origin, 0);
            for (script, prefix) in [(vec![TACTICAL_ACTION_ID, target.as_str(), &mv, "load|0", "load|1"], false), (vec![TACTICAL_ACTION_ID, target.as_str(), &mv, "load|0", "load|1", "done_moving"], true)] {
                for d in lockstep(state.clone(), &hub.galaxy, &script, prefix) {
                    failures.push(format!("{faction} (prefix {prefix}): {d}"));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    fn b() -> PlayerId {
        PlayerId::new("b")
    }

    /// Spatial Conduit Cylinders: offered to the active seat after the activation (a unit in the system and one elsewhere); used, the system is
    /// linked to every system with its units, so the ship in the far corner can move to it.
    #[test]
    fn spatial_conduit_cylinders_asks_and_links_in_lockstep() {
        let (hub, origin, target) = hub();
        let far = ti4_model::id::SystemId::new(hub.outer.iter().find(|s| **s != origin.as_str() && **s != target.as_str()).cloned().expect("another outer system"));
        for (answer, route) in [("use", 0), ("decline", 0), ("use", 1)] {
            let mut state = table(&[("a", "jolnar"), ("b", "hacan")]);
            state.player_mut(&a()).unwrap().technologies.insert(ti4_model::TechnologyId::new("scc"));
            crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
            crate::fixtures::put(&mut state, &target, "cruiser", &a(), 1);
            crate::fixtures::put(&mut state, &far, "cruiser", &a(), 1);
            let mv = if route == 0 { id(&origin, 0) } else { format!("{}|r{route}", id(&far, 0)) };
            let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), answer, &mv], false);
            assert!(diffs.is_empty(), "{answer} route {route}: {diffs:#?}");
        }
    }

    /// The Nullification Field of another seat with a ship in the system ends the active seat's turn; the Minister of Peace's owner may discard
    /// the law to do the same. The string runs `advance_turn` inline, the native window opens the turn flow's `End` job (compared on the prefix).
    #[test]
    fn the_hooks_that_end_the_turn_ask_in_lockstep() {
        let (hub, origin, target) = hub();
        for answer in ["use", "decline"] {
            let mut state = table(&[("a", "sol"), ("b", "xxcha")]);
            crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
            crate::fixtures::put(&mut state, &target, "cruiser", &b(), 1);
            {
                let holder = state.player_mut(&b()).unwrap();
                holder.technologies.insert(ti4_model::TechnologyId::new("nf"));
                holder.strategic_tokens = 2;
            }
            let mv = id(&origin, 0);
            let script: Vec<&str> = if answer == "use" { vec![TACTICAL_ACTION_ID, target.as_str(), answer] } else { vec![TACTICAL_ACTION_ID, target.as_str(), answer, &mv] };
            let diffs = lockstep(state, &hub.galaxy, &script, true);
            assert!(diffs.is_empty(), "nullification field {answer}: {diffs:#?}");
        }
        for answer in ["discard", "decline"] {
            let mut state = table(&[("a", "sol"), ("b", "hacan")]);
            crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
            crate::fixtures::put(&mut state, &target, "cruiser", &b(), 1);
            state.laws.insert("minister_peace".to_owned(), "b".to_owned());
            let mv = id(&origin, 0);
            let script: Vec<&str> = if answer == "discard" { vec![TACTICAL_ACTION_ID, target.as_str(), answer] } else { vec![TACTICAL_ACTION_ID, target.as_str(), answer, &mv] };
            let diffs = lockstep(state, &hub.galaxy, &script, true);
            assert!(diffs.is_empty(), "minister of peace {answer}: {diffs:#?}");
        }
    }

    /// The Dominus Orb, held by the active seat while a token of its own pins units elsewhere, is offered after the activation.
    #[test]
    fn the_dominus_orb_is_offered_after_the_activation_in_lockstep() {
        let (hub, origin, target) = hub();
        for answer in ["purge", "decline"] {
            let mut state = table(&[("a", "sol"), ("b", "hacan")]);
            crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
            state.system_mut(&origin).command_tokens.insert(a());
            state.player_mut(&a()).unwrap().relics.push(ti4_model::id::RelicId::new("dominusorb"));
            let mv = id(&origin, 0);
            // Declined, the own token still pins the carrier: nothing to move.
            let script: Vec<&str> = if answer == "purge" { vec![TACTICAL_ACTION_ID, target.as_str(), answer, &mv] } else { vec![TACTICAL_ACTION_ID, target.as_str(), answer] };
            let diffs = lockstep(state, &hub.galaxy, &script, false);
            assert!(diffs.is_empty(), "{answer}: {diffs:#?}");
        }
    }

    /// A ship that exits a gravity rift rolls a die (1-3 loses it and its hold): the native run draws from the same stream as the string, so the
    /// positions of the streams agree as well as the board, for a number of seeds (lost and arrived both occur).
    #[test]
    fn a_ship_exiting_a_gravity_rift_rolls_the_string_dice_in_lockstep() {
        let rift = crate::fixtures::a_system_where("gravity rift");
        let plain = crate::fixtures::plain_systems(2);
        let galaxy = Galaxy::placed(
            ContentStore::embedded(),
            &[(plain[0].as_str(), ti4_model::hex::Hex::new(0, 0)), (rift.as_str(), ti4_model::hex::Hex::new(1, 0)), (plain[1].as_str(), ti4_model::hex::Hex::new(2, 0))],
            POK,
        )
        .expect("a line of three systems");
        let (origin, target) = (ti4_model::id::SystemId::new(&plain[0]), ti4_model::id::SystemId::new(&plain[1]));
        for seed in 0..16_u64 {
            let mut state = table(&[("a", "sol"), ("b", "hacan")]);
            state.rng_seed = seed;
            crate::fixtures::put(&mut state, &origin, "carrier2", &a(), 1);
            crate::fixtures::put(&mut state, &origin, "infantry", &a(), 2);
            let mv = id(&origin, 0);
            let diffs = lockstep(state, &galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv, "load|0", "load|1"], false);
            assert!(diffs.is_empty(), "seed {seed}: {diffs:#?}");
        }
    }

    /// A clone of the position taken while a hook of the activation waits on its question (the question is a nested one: the journal holds the
    /// answers given so far) continues identically: same question after the answer, same position.
    #[test]
    fn a_clone_taken_while_a_hook_waits_continues_identically() {
        use ti4_engine_core::choice::{relic::Opt as ROpt, Opt as COpt, Pick};
        use ti4_engine_core::driver::live::{advance, Live};
        let (hub, origin, target) = hub();
        let mut state = table(&[("a", "sol"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        state.system_mut(&origin).command_tokens.insert(a());
        state.player_mut(&a()).unwrap().relics.push(ti4_model::id::RelicId::new("dominusorb"));
        let _guard = crate::dual::tests::serial();
        let rng = crate::rng::GameRng::new(state.rng_seed).positions();
        let mut cx = Ctx::of(&state).expect("owners");
        let render = Render::new(cx.owners.ids().to_vec());
        let mut core = Codec::load(&mut cx, &state, Sources { galaxy: Some(&hub.galaxy), rng: Some(&rng) }, SliceSet::ALL).expect("the position loads");
        let seat = render.seat("a").expect("seat a");
        open_tactical(&mut core, seat);
        let kernel = crate::live::kernel::kernel_for(POK);
        let env = crate::live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), POK);
        let system = <ti4_engine_core::generated::System as crate::generated::IdStr>::from_id(target.as_str()).expect("a system");
        let (live, _) = advance(&mut core, &env, None).expect("the activation question");
        assert!(matches!(live, Live::Asks(_)));
        let pick = Pick::Opt(COpt::Tactical(TOpt::Activate(system)));
        let (live, _) = advance(&mut core, &env, Some(pick)).expect("the hook's question");
        let Live::Asks(asking) = live else { panic!("the Dominus Orb asks") };
        assert!(asking.nested.is_some() || core.pending().nested.is_some(), "a question asked from inside the step");
        let mut twin = core.clone();
        let purge = Pick::Opt(COpt::Relic(ROpt::Purge));
        let (a_live, _) = advance(&mut core, &env, Some(purge)).expect("the original continues");
        let (b_live, _) = advance(&mut twin, &env, Some(purge)).expect("the clone continues");
        let ask_of = |l: Live| match l {
            Live::Asks(x) => Some(x.choice),
            Live::Done => None,
        };
        assert_eq!(ask_of(a_live), ask_of(b_live), "the same next question");
        let (ja, jb) = (materialize::materialize(&core, &cx, &state).expect("materializes"), materialize::materialize(&twin, &cx, &state).expect("materializes"));
        assert_eq!(serde_json::to_value(&ja).unwrap(), serde_json::to_value(&jb).unwrap(), "the same position");
    }

    /// Mahact's commander lets the seat activate a system that holds its own token; the hook chain returns both tokens and ends the turn.
    /// The Starlancer of a Mahact seat in the system, with the activator's token in its fleet pool, may end the turn too (the activator gains
    /// the token into a pool of its choice).
    #[test]
    fn the_mahact_hooks_of_the_activation_run_in_lockstep() {
        let (hub, origin, target) = hub();
        let mut state = table(&[("a", "mahact"), ("b", "sol")]);
        state.player_mut(&a()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("mahactcommander"), ti4_model::state::LeaderStatus::Unlocked);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        state.system_mut(&target).command_tokens.insert(a());
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()], true);
        assert!(diffs.is_empty(), "commander: {diffs:#?}");
        for answer in ["use", "decline"] {
            let mut state = table(&[("a", "sol"), ("b", "mahact")]);
            crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
            crate::fixtures::put(&mut state, &target, "mahact_mech", &b(), 1);
            state.faction_marks.insert(crate::factions::mahact::fleet_mark(&b()), "a".to_owned());
            let mv = id(&origin, 0);
            let script: Vec<&str> = if answer == "use" { vec![TACTICAL_ACTION_ID, target.as_str(), "use", "tactic_tokens"] } else { vec![TACTICAL_ACTION_ID, target.as_str(), "decline", &mv] };
            let diffs = lockstep(state, &hub.galaxy, &script, true);
            assert!(diffs.is_empty(), "starlancer {answer}: {diffs:#?}");
        }
    }

    /// Scanlink Drone Network: the Titans may explore a planet of the activated system that holds their units.
    #[test]
    fn scanlink_drone_network_explores_in_lockstep() {
        let (hub, origin, _) = hub();
        let system = ti4_model::id::SystemId::new("28");
        let mut state = table(&[("a", "titans"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        crate::fixtures::put_on_planet(&mut state, &system, &ti4_model::id::PlanetId::new("tequran"), "infantry", &a(), 1);
        let diffs = lockstep(state.clone(), &hub.galaxy, &[TACTICAL_ACTION_ID, system.as_str(), "tequran"], true);
        assert!(diffs.is_empty(), "explore: {diffs:#?}");
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, system.as_str(), "decline"], true);
        assert!(diffs.is_empty(), "decline: {diffs:#?}");
    }

    /// E-Res Siphons (automatic, after the activation of a system that holds the holder's ships) and the Sardakk and Winnu marks of the
    /// activation, in lockstep with the string.
    #[test]
    fn the_hooks_of_the_activation_that_do_not_ask_run_as_the_string_runs_them() {
        let (hub, origin, target) = hub();
        let mut state = table(&[("a", "sardakk"), ("b", "jolnar")]);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        crate::fixtures::put(&mut state, &target, "cruiser", &PlayerId::new("b"), 1);
        state.player_mut(&PlayerId::new("b")).unwrap().technologies.insert(ti4_model::TechnologyId::new("ers"));
        let mv = id(&origin, 0);
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv], false);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }
}
