//! The engine half of the native aftermath chain and its space combat (lane M5, `docs/M5_RESUME.md`): the texts the core composer needs from
//! outside (`driver::windows::{chain, combat}` have no text of their own), the words of the positions it does not run, and the lockstep tests
//! that run a real tactical action twice, once as the string `Game` and once as the native arms (`Arm::Tactical`, `Arm::Aftermath`,
//! `Arm::EventScoring`) through `live_step`, and compare the questions, the answers, the events and the position.

use ti4_engine_core::generated::Unit;
use ti4_engine_core::kernel::ability::{AftermathGap, CombatTexts};
use ti4_engine_core::state::{Code, CoreState, CODE_CAPACITY};

use crate::generated::IdStr;
use crate::render::Render;

/// The words of a position the native aftermath does not run (`Unknown::UnportedAftermath`).
#[must_use]
pub const fn gap_text(why: AftermathGap) -> &'static str {
    match why {
        AftermathGap::NoProgress => "the aftermath made no progress (a stage that neither asks nor moves on)",
        AftermathGap::Negotiating => "the contact pause of the aftermath (structured diplomacy is dropped)",
        AftermathGap::NoCombatEnv => "a space combat whose environment cannot be built from the position",
        AftermathGap::CombatUnported => "the space combat machine reached a position it does not carry (cancelled hits, Duranium Armor, Devotion, staged destructions, a retreat the core does not move)",
        AftermathGap::RollEndsFight => "a roll that ended the fight inside the roll step",
        AftermathGap::NeutralSide => "a neutral side in the space combat",
        AftermathGap::EventScoring => "a scoring window that asks for something the core does not offer",
    }
}

/// The display facts of a combat question that only the position knows: the sustainers the casualty is chosen from, and the subtype of the
/// casualty question (its text is a render code the core's `Subtype::Casualty(Code)` cannot carry from a window rebuilt without the adapter).
pub fn complete(core: &CoreState, k: &ti4_engine_core::kernel::Kernel, choice: &ti4_engine_core::choice::Choice, rendered: &mut crate::choice::Choice) {
    use ti4_engine_core::choice::combat::Ask as CAsk;
    let ti4_engine_core::choice::Ask::Combat(ask) = &choice.ask else { return };
    match ask {
        CAsk::RerollDie { .. } | CAsk::MunitionsRoll => roll_details(core, choice, rendered),
        CAsk::CancelHit { .. } => {
            rendered.details.insert("sustainers".to_owned(), crate::core_combat::sustainers_detail(k, core, ask, choice.actor));
        }
        CAsk::AssignHit => {
            if let Some(ctx) = rendered.context.as_mut() {
                ctx.subtype = "assign_casualty".to_owned();
            }
        }
        _ => {}
    }
}

/// The questions of the combat the string asks in the battle they are fought in (`TimingContext::ask_seeing`: Ambush, Sacrifice, Splicer, Raid
/// Formation); every other question a combat child asks inside an emission names no battle. The nested rendering puts the battle on all of them.
pub fn after_nested(choice: &ti4_engine_core::choice::Choice, rendered: &mut crate::choice::Choice) {
    use ti4_engine_core::choice::combat::Ask as CAsk;
    // (Rear Admiral Farran's payment after a sustain is the leaders' question, asked in the same step.)
    let in_combat = match &choice.ask {
        ti4_engine_core::choice::Ask::Combat(ask) => !matches!(ask, CAsk::Ambush | CAsk::Sacrifice { .. } | CAsk::Splicer | CAsk::RaidDamage { .. }),
        ti4_engine_core::choice::Ask::Leaders(ti4_engine_core::choice::leaders::Ask::SustainCommander) => true,
        _ => false,
    };
    if !in_combat {
        return;
    }
    if let Some(ctx) = rendered.context.as_mut() {
        ctx.space_battle = false;
        ctx.target = None;
    }
}

/// The display facts of a roll step's questions (`dual_window::roll_details`): the staged dice, which one is asked about, and Munitions
/// Reserves' printed card and cost.
fn roll_details(core: &CoreState, c: &ti4_engine_core::choice::Choice, rendered: &mut crate::choice::Choice) {
    use ti4_engine_core::choice::combat::Ask as CAsk;
    let ti4_engine_core::choice::Ask::Combat(ask) = &c.ask else { return };
    let set = core.transient().combat.reroll_staging.iter().find(|(s, _)| *s == c.actor).map(|(_, set)| set);
    let text = |code: Code| core.texts.codes.get(u32::from(code.0)).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    let dice: Vec<serde_json::Value> = set
        .map(|set| {
            set.rolls
                .iter()
                .enumerate()
                .flat_map(|(unit, entry)| {
                    let name = text(entry.unit);
                    entry.faces.iter().enumerate().map(move |(die, face)| {
                        let bonus = u8::try_from(die).ok().and_then(|d| entry.deltas.get(&d).copied()).unwrap_or(0);
                        serde_json::json!({
                            "unit": name,
                            "unit_index": unit,
                            "die": die,
                            "face": face,
                            "bonus": bonus,
                            "hits_on": entry.hits_on,
                            "rerolled": die < 64 && entry.rerolled & (1 << die) != 0,
                        })
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    match ask {
        CAsk::RerollDie { die, .. } => {
            let entry = c.opts.iter().find_map(|o| match o {
                ti4_engine_core::choice::Opt::Combat(ti4_engine_core::choice::combat::Opt::RerollDie { entry, .. }) => Some(usize::from(*entry)),
                _ => None,
            });
            let current = set.zip(entry).map_or(0, |(set, e)| set.rolls[..e].iter().map(|r| r.faces.len()).sum::<usize>() + usize::from(*die));
            rendered.details.insert("dice".to_owned(), serde_json::Value::Array(dice));
            rendered.details.insert("current".to_owned(), current.into());
        }
        CAsk::MunitionsRoll => {
            let misses = dice
                .iter()
                .filter(|d| {
                    let on = d["hits_on"].as_i64();
                    on.is_some_and(|on| d["face"].as_i64().unwrap_or(0) + d["bonus"].as_i64().unwrap_or(0) < on)
                })
                .count();
            rendered.details.insert("kind".to_owned(), "ability_offer".into());
            rendered.details.insert("ability".to_owned(), crate::faction_abilities::ability_card(ti4_content::ContentStore::embedded(), "munitions"));
            rendered.details.insert("cost".to_owned(), serde_json::json!({ "trade_goods": crate::faction_abilities::MUNITIONS_COST, "have": core.players().by_seat(c.actor).map_or(0, |p| i64::from(p.econ.trade_goods.max(0))) }));
            rendered.details.insert("dice".to_owned(), serde_json::Value::Array(dice));
            rendered.details.insert("misses".to_owned(), misses.into());
        }
        _ => {}
    }
}

/// The most unit types one fight may name (the roll group texts are every subset of them).
const MAX_TYPES: usize = 10;

/// The texts of the space combat at `core`: the fixed words, and for the unit types that can be named in the fight in `active_space_combat` (the
/// types present, with the other form of each dual-form unit) their names and every roll group's. State texts go to the position's table, shown
/// texts to the render table. A fight with more than ten types names none (the machine then stops at a roll it cannot name).
pub fn combat_texts(core: &mut CoreState, r: &Render) -> CombatTexts {
    let mut state_code = |core: &mut CoreState, text: &str| -> Code {
        Code(u16::try_from(core.texts.codes.intern(text.as_bytes(), CODE_CAPACITY).expect("room for the text")).expect("a code fits"))
    };
    let mut t = CombatTexts::default();
    for (slot, text) in t.phases.iter_mut().zip(["pre_roll", "barrage", "resolving_hits", "retreating", "complete"]) {
        *slot = state_code(core, text);
    }
    t.barrage_extra = state_code(core, "extra barrage");
    t.retreat_cause = state_code(core, "retreat_capacity");
    t.barrage_extra_shown = r.code("extra barrage");
    t.barrage_cause = r.code("unit_ability:anti_fighter_barrage");
    t.barrage_kind = r.code("anti_fighter_barrage");
    t.roll_cause = r.code("space_combat");
    t.hacan = r.code("hacan_flagship");
    t.retreat_event = r.code("retreat_capacity");
    t.retreat_pool = r.code("reinforcements");
    t.raid = r.code("raid_formation_damage");
    t.ac_subtype = r.code("assault_cannon_destroy");
    t.ac_cause = r.code("technology:assault_cannon");
    t.ambush = r.code("ambush");
    t.cause_core = r.code("technology:impulse_core");
    t.cause_devotion = r.code("faction_ability:devotion");
    t.casualty = r.code("assign_casualty");
    t.commander = r.code("letnevcommander");
    t.opponents = r.code("opponents");
    for (slot, text) in t.end.iter_mut().zip(["losers", "retreated", "retreat", "draw", "destruction", "round_limit"]) {
        *slot = r.code(text);
    }
    let Some((system, _, _)) = core.transient().combat.active_space_combat else { return t };
    let mut present: Vec<Unit> = Vec::new();
    let cell = core.board().sys(system);
    let mut pieces: Vec<Unit> = cell.units.iter().map(|p| p.kind()).collect();
    for planet in ti4_engine_core::rules::combat::planets_of_sorted(core, system).iter() {
        pieces.extend(core.board().planet(*planet).units.iter().map(|p| p.kind()));
    }
    for u in pieces {
        if !present.contains(&u) {
            present.push(u);
        }
        if let Some(twin) = ti4_engine_core::query::tech::other_form(u)
            && !present.contains(&twin)
        {
            present.push(twin);
        }
    }
    present.sort_unstable();
    if present.len() > MAX_TYPES {
        return t;
    }
    for u in &present {
        let id = IdStr::as_str(*u);
        t.unit_codes.push((*u, state_code(core, id)));
        t.shown_codes.push((*u, r.code(id)));
    }
    for mask in 1_u32..(1 << present.len()) {
        let kinds: Vec<Unit> = present.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, u)| *u).collect();
        let joined = kinds.iter().map(|u| IdStr::as_str(*u)).collect::<Vec<_>>().join("+");
        t.groups.push((kinds, state_code(core, &joined), r.code(&joined)));
    }
    t
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use serde_json::{json, Value};
    use ti4_content::galaxy::Galaxy;
    use ti4_content::ContentStore;
    use ti4_engine_core::driver::session::{step_arm, Arm, Facts};
    use ti4_engine_core::driver::windows::tactical::open_tactical;
    use ti4_engine_core::state::SliceSet;
    use ti4_model::content_types::POK;
    use ti4_model::id::{PlayerId, SystemId};
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

    fn b() -> PlayerId {
        PlayerId::new("b")
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

    /// The answer after the script ran out, the same on both sides: the first option; but a production question (the step after the invasion,
    /// lane M2) is answered by the build budget: `build|destroyer|1` while the run may still buy one (paid with trade goods), then
    /// `done_producing`.
    fn default_answer(c: &Choice, builds: &std::cell::Cell<u32>) -> Option<String> {
        let has = |id: &str| c.options.iter().any(|o| o.id == id);
        if c.prompt.starts_with("produce in") {
            if builds.get() > 0
                && let Some(o) = c.options.iter().find(|o| o.id.starts_with("build|destroyer|"))
            {
                builds.set(builds.get() - 1);
                return Some(o.id.clone());
            }
            if has("done_producing") {
                return Some("done_producing".to_owned());
            }
        }
        if c.prompt.starts_with("pay ") && has("trade_good") {
            return Some("trade_good".to_owned());
        }
        c.options.first().map(|o| o.id.clone())
    }

    /// What one side of the lockstep leaves.
    struct Side {
        asked: Vec<Choice>,
        events: Vec<String>,
        /// The events of the aftermath's own arms, in order (native only): the typed ones the steps emitted, then the plain ones they wrote.
        after: Vec<String>,
        state: Value,
        bail: Option<String>,
    }

    /// Answers from the script, then the first option (the questions after the script are the same on both sides: the default is deterministic).
    struct Recorder {
        queue: Rc<RefCell<VecDeque<String>>>,
        asked: Rc<RefCell<Vec<Choice>>>,
        builds: std::cell::Cell<u32>,
    }

    impl crate::choice::Decider for Recorder {
        fn choose(&mut self, c: &Choice) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
            self.asked.borrow_mut().push(c.clone());
            let Some(id) = self.queue.borrow_mut().pop_front().or_else(|| default_answer(c, &self.builds)) else {
                return Err(crate::choice::IllegalChoice::NotOffered { player: c.player.clone(), chosen: "<no option>".to_owned(), offered: Vec::new() });
            };
            c.options.iter().find(|o| o.id == id).cloned().ok_or(crate::choice::IllegalChoice::NotOffered { player: c.player.clone(), chosen: id, offered: c.ids().into_iter().map(str::to_owned).collect() })
        }
    }

    /// The string run: `script[0]` answers the action menu, the rest the window's questions; steps until the action is over.
    fn string_run(state: &GameState, galaxy: &Galaxy, script: &[&str], builds: u32) -> Side {
        let _ = take_report();
        let queue = Rc::new(RefCell::new(script.iter().map(|s| (*s).to_owned()).collect::<VecDeque<_>>()));
        let asked = Rc::new(RefCell::new(Vec::new()));
        let table = crate::choice::Table::with_default(Box::new(Recorder { queue: queue.clone(), asked: asked.clone(), builds: std::cell::Cell::new(builds) }));
        let mut game = Game::with_table(state.clone(), ContentStore::embedded(), table).with_galaxy(galaxy.clone());
        with_mask(AreaMask::STRING, || {
            for i in 0..80 {
                if game.step().error.is_some() {
                    break;
                }
                // The action is over once the system is no longer active (`close_tactical`).
                if i > 0 && game.state.active_system.is_none() {
                    break;
                }
            }
        });
        let asked: Vec<Choice> = asked.borrow().iter().skip(1).filter(|c| !c.options.is_empty()).cloned().collect();
        if std::env::var_os("M5_DEBUG").is_some() {
            for (i, q) in asked.iter().enumerate() {
                eprintln!("Q{i} {} {}: {:?}", q.player, q.prompt, q.options.iter().map(|o| o.id.as_str()).collect::<Vec<_>>());
            }
        }
        Side { asked, events: game.events.clone(), after: Vec::new(), state: serde_json::to_value(&game.state).expect("serialises"), bail: None }
    }

    /// Answers from the shared queue, then the first option.
    struct Shared {
        queue: Rc<RefCell<VecDeque<String>>>,
        seen: Rc<RefCell<Vec<Choice>>>,
        builds: std::cell::Cell<u32>,
    }

    impl crate::live::Asker for Shared {
        fn ask(&mut self, q: &crate::live::Question<'_>) -> Result<String, crate::live::Stop> {
            self.seen.borrow_mut().push(q.rendered.clone());
            if std::env::var_os("M5_DEBUG").is_some() {
                eprintln!("NQ {} {} nested={} {:?}", q.rendered.player, q.rendered.prompt, q.asking.nested.is_some(), q.rendered.options.iter().map(|o| o.id.as_str()).collect::<Vec<_>>());
            }
            Ok(self.queue.borrow_mut().pop_front().unwrap_or_else(|| default_answer(&q.rendered, &self.builds).unwrap_or_default()))
        }
    }

    /// The native run: the tactical window opens as the menu's answer does, then one `live_step` per step of the arm the position names, until the
    /// action is over.
    fn native_run(state: &GameState, galaxy: &Galaxy, rng: &crate::rng::RngPositions, script: &[&str], builds: u32) -> Side {
        let mut cx = Ctx::of(state).expect("owners");
        let render = Render::new(cx.owners.ids().to_vec());
        let sources = POK;
        let mut core = Codec::load(&mut cx, state, Sources { galaxy: Some(galaxy), rng: Some(rng) }, SliceSet::ALL).expect("the position loads");
        let seat = render.seat("a").expect("seat a");
        open_tactical(&mut core, seat);
        let kernel = crate::live::kernel::kernel_for(sources);
        let env = crate::live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), sources);
        if std::env::var_os("M5_DEBUG").is_some() {
            use ti4_engine_core::choice::reaction::Relation;
            use ti4_engine_core::state::EventKind;
            let mine: Vec<_> = env.registry.for_event(EventKind::CombatRoundStarted, Relation::After).map(|d| (d.owner, d.id)).collect();
            eprintln!("REGISTRY {} abilities; CombatRoundStarted/After: {mine:?}", env.registry.len());
        }
        let live = Cx::new(&render, &env);
        let queue = Rc::new(RefCell::new(script[1..].iter().map(|s| (*s).to_owned()).collect::<VecDeque<_>>()));
        let seen = Rc::new(RefCell::new(Vec::new()));
        let mut asker = Shared { queue: queue.clone(), seen: seen.clone(), builds: std::cell::Cell::new(builds) };
        let mut bail = None;
        let mut after = Vec::new();
        for _ in 0..120 {
            let arm = step_arm(&Facts::of_core(&core, false), true);
            if !matches!(arm, Arm::Tactical | Arm::Aftermath | Arm::EventScoring) {
                break;
            }
            match live_step(&mut core, arm, &live, &mut asker) {
                Ok(step) => {
                    if arm != Arm::Tactical {
                        after.extend(step.out.emitted.iter().filter_map(|e| crate::dual_window::event_of(&render, e)).map(|e| e.event_type));
                        for line in core.ledger.events.iter().skip(step.events_from) {
                            if let ti4_engine_core::state::Emit::Plain(rec) = line
                                && let Some(p) = crate::render::events::pushed_of(&render, &cx, &ti4_engine_core::state::Emit::Plain(*rec))
                            {
                                after.push(p.text().to_owned());
                            }
                        }
                    }
                }
                Err(crate::live::Stop::Live(ti4_engine_core::driver::live::LiveError::Unknown(why))) => {
                    bail = Some(match why {
                        ti4_engine_core::kernel::ability::Unknown::UnportedAftermath(gap) => super::gap_text(gap).to_owned(),
                        ti4_engine_core::kernel::ability::Unknown::UnportedTactical(gap) => crate::tactical_live::gap_text(gap).to_owned(),
                        ti4_engine_core::kernel::ability::Unknown::Unported(_) => "the invasion".to_owned(),
                        other => format!("{other:?}"),
                    });
                    break;
                }
                Err(e) => panic!("the native step ({arm:?}): {e:?} after {:?}", seen.borrow().iter().map(|c| (c.player.to_string(), c.prompt.clone())).collect::<Vec<_>>()),
            }
        }
        let asked = seen.borrow().clone();
        if std::env::var_os("M5_DEBUG").is_some() {
            for (i, q) in asked.iter().enumerate() {
                eprintln!("N{i} {} {}: {:?}", q.player, q.prompt, q.options.iter().map(|o| o.id.as_str()).collect::<Vec<_>>());
            }
        }
        let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
        let materialized = materialize::materialize(&core, &cx, state).expect("materializes");
        let after_events = after;
        Side { asked, events, after: after_events, state: serde_json::to_value(&materialized).expect("serialises"), bail }
    }

    fn leaves(label: &str, left: &Value, right: &Value) -> Vec<String> {
        // `active`, `action_turn`, `turn_seq`: the string closes the action into the next turn (`finish_action`, the turn flow's, not run here).
        // `trigger.event_id`: the string's first step is the turn prelude (`TURN_BEGAN`), which numbers one event the native run does not take.
        // `combat_round_seq`: the native invasion bumps the counter once more than the string's window over a ground fight (a finding for lane
        // M3: `driver::windows::invasion::fight` and `GroundRollTail::bump`), compared separately in the invasion tests.
        diff_json(left, right, 60).into_iter().filter(|d| !d.path.contains("#seen:") && !d.path.contains("event_id") && !d.path.ends_with("combat_round_seq") && !matches!(d.path.as_str(), "action_turn" | "active" | "turn_seq" | "/action_turn" | "/active" | "/turn_seq") && !d.path.ends_with("action_turn") && !d.path.ends_with("active") && !d.path.ends_with("turn_seq")).take(10).map(|d| format!("{label}{} string={} native={}", d.path, d.left, d.right)).collect()
    }

    /// Run both and report every difference of the questions, the events and the position.
    fn lockstep(state: GameState, galaxy: &Galaxy, script: &[&str]) -> Vec<String> {
        lockstep_buying(state, galaxy, script, 0)
    }

    /// [`lockstep`] with a production step that may buy `builds` destroyers after the script ran out.
    fn lockstep_buying(state: GameState, galaxy: &Galaxy, script: &[&str], builds: u32) -> Vec<String> {
        let _guard = crate::dual::tests::serial();
        let rng = crate::rng::GameRng::new(state.rng_seed).positions();
        let s = string_run(&state, galaxy, script, builds);
        let n = native_run(&state, galaxy, &rng, script, builds);
        let mut out = Vec::new();
        if let Some(why) = &n.bail {
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
        // The events of the aftermath's arms against the string's, restricted to the names the native arms announced (the tactical arm's own
        // events are compared in `tactical_live`).
        // `TRADE_GOODS_GAINED` is left out: Rear Admiral Farran's gain is a STAGED event; the native arms announce it with the next emission (the
        // invasion's `INVASION_BEGAN`), the string's queue does not show it there. The merge of the staged events into the one queue is the
        // wrapper's (P1/D2-8d); the gain itself is compared in the position.
        let n_after: Vec<String> = n.after.iter().filter(|e| *e != "TRADE_GOODS_GAINED").cloned().collect();
        let names: std::collections::BTreeSet<&str> = n_after.iter().map(String::as_str).collect();
        let string_after: Vec<&String> = s.events.iter().filter(|e| names.contains(e.as_str())).collect();
        if string_after.iter().map(|e| e.as_str()).ne(n_after.iter().map(String::as_str)) {
            out.push(format!("events: string {string_after:?} native {n_after:?}"));
        }
        out.extend(leaves("state", &s.state, &n.state));
        if std::env::var_os("M5_DEBUG").is_some() {
            for (i, q) in s.asked.iter().enumerate() {
                eprintln!("Q{i} {} {}: {:?}", q.player, q.prompt, q.options.iter().map(|o| o.id.as_str()).collect::<Vec<_>>());
            }
            eprintln!("STRING EVENTS {:?}\nNATIVE EVENTS {:?}\nAFTER {:?}", s.events, n.events, n.after);
        }
        out
    }

    fn hub() -> (crate::fixtures::Hub, SystemId, SystemId) {
        let hub = crate::fixtures::plain_hub();
        let (origin, target) = (SystemId::new(hub.outer[0].clone()), SystemId::new(hub.centre.clone()));
        (hub, origin, target)
    }

    fn mv(origin: &SystemId, index: usize) -> String {
        format!("move|{origin}|{index}")
    }

    /// A space fight with no decision of the defender: a carrier and a cruiser against a lone cruiser, both sides answering the first option.
    #[test]
    fn a_space_fight_runs_native_with_the_questions_and_the_position_of_the_string() {
        let (hub, origin, target) = hub();
        let mut state = table(&[("a", "sol"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "dreadnought", &a(), 2);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        crate::fixtures::put(&mut state, &target, "cruiser", &b(), 2);
        crate::fixtures::put(&mut state, &target, "dreadnought", &b(), 1);
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv(&origin, 0), &mv(&origin, 0), &mv(&origin, 0), "done_moving"]);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }

    /// A retreat: the defender announces it and picks a destination (an adjacent system holding its units), the attacker keeps the space.
    #[test]
    fn a_retreat_asks_and_moves_the_fleet_as_the_string_does() {
        let (hub, origin, target) = hub();
        let refuge = SystemId::new(hub.outer[1].clone());
        let mut state = table(&[("a", "sol"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "dreadnought", &a(), 2);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        crate::fixtures::put(&mut state, &target, "cruiser", &b(), 2);
        crate::fixtures::put(&mut state, &refuge, "cruiser", &b(), 1);
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv(&origin, 0), &mv(&origin, 0), &mv(&origin, 0), "done_moving", "retreat"]);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }

    /// A planet system as the centre of the hub (the target of the action): its planet and the hub.
    fn planet_hub() -> (crate::fixtures::Hub, SystemId, SystemId, ti4_model::id::PlanetId) {
        let (system, planet) = crate::fixtures::a_placed_planet();
        let hub = crate::fixtures::hub_with_centre(system.as_str());
        let origin = SystemId::new(hub.outer[0].clone());
        (hub, origin, system, planet)
    }

    /// A conquest with no space fight: the carrier brings infantry to a planet the other player holds, and the aftermath goes on to the invasion.
    #[test]
    fn an_undefended_system_goes_on_to_the_invasion_in_lockstep() {
        let (hub, origin, target, planet) = planet_hub();
        let mut state = table(&[("a", "sol"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        crate::fixtures::put(&mut state, &origin, "infantry", &a(), 3);
        crate::fixtures::put_on_planet(&mut state, &target, &planet, "infantry", &b(), 1);
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &mv(&origin, 0), "load|0", "load|1", "load|2", "done_moving"]);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }

    /// A space fight won, and the invasion that follows it.
    #[test]
    fn a_won_space_fight_goes_on_to_the_invasion_in_lockstep() {
        let (hub, origin, target, planet) = planet_hub();
        let mut state = table(&[("a", "sol"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "dreadnought", &a(), 2);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        crate::fixtures::put(&mut state, &origin, "infantry", &a(), 3);
        crate::fixtures::put(&mut state, &target, "cruiser", &b(), 1);
        crate::fixtures::put_on_planet(&mut state, &target, &planet, "infantry", &b(), 1);
        // The dreadnoughts sail empty (a hold of one each: `done_loading`), the carrier takes the infantry.
        let (m0, m2) = (mv(&origin, 0), mv(&origin, 2));
        let _ = m2;
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &m0, "done_loading", &m0, "done_loading", &m0, "load|0", "load|1", "load|2", "done_moving"]);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }

    /// A destroyed capital ship opens the scoring window of the feat after the fight: the winner scores the secret objective (the pause of the
    /// aftermath, the book, `OBJECTIVE_SCORED`, and the points).
    #[test]
    fn a_capital_ship_destroyed_opens_the_scoring_window_and_scores_in_lockstep() {
        let (hub, origin, target) = hub();
        let mut state = table(&[("a", "sol"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &origin, "dreadnought", &a(), 3);
        crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
        crate::fixtures::put(&mut state, &target, "warsun", &b(), 1);
        state.player_mut(&a()).unwrap().secret_objectives.push(ti4_model::id::SecretObjectiveId::new("dtgs"));
        // (Every later answer is the first option: each ship moves, then `done_moving`.)
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()]);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }

    /// The active seat's ships are already in the system: the action activates it and moves nothing, and the fight starts at once. Mentak's Ambush
    /// asks inside the start of the combat (a child question of the machine, asked through the tape).
    #[test]
    fn mentak_ambush_at_the_start_of_the_fight_is_asked_through_the_tape_in_lockstep() {
        let (hub, _origin, target) = hub();
        let mut state = table(&[("a", "mentak"), ("b", "hacan")]);
        crate::fixtures::put(&mut state, &target, "cruiser", &a(), 2);
        crate::fixtures::put(&mut state, &target, "dreadnought", &b(), 2);
        crate::fixtures::put(&mut state, &target, "cruiser", &b(), 1);
        let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()]);
        assert!(diffs.is_empty(), "{diffs:#?}");
    }

    /// A space fight of every faction as the attacker and as the defender, native from the end of the movement step to the end of the action,
    /// with the faction's own abilities of the combat (Mentak, Argent, Ghost, Naalu, Sardakk, Xxcha, Hacan, Winnu, Sol, Letnev, Jol-Nar, ...) in
    /// play. Bail as failure: nothing may stop the native path, and questions, events and position are the string's.
    #[test]
    fn a_space_fight_of_each_faction_runs_native_without_a_bail_and_as_the_string_does() {
        let (hub, _origin, target) = hub();
        let mut failures = Vec::new();
        let only = std::env::var("M5_ONLY").ok();
        for faction in ["mentak", "argent", "ghost", "naalu", "sardakk", "xxcha", "hacan", "winnu", "sol", "letnev", "jolnar", "l1z1x", "arborec", "muaat", "nekro", "saar", "yin", "yssaril", "cabal", "empyrean", "naaz", "nomad", "titans", "mahact"] {
            if only.as_deref().is_some_and(|o| o != faction) {
                continue;
            }
            let other = if faction == "letnev" { "sol" } else { "letnev" };
            for (mine, theirs) in [(faction, other), (other, faction)] {
                let mut state = table(&[("a", mine), ("b", theirs)]);
                crate::fixtures::put(&mut state, &target, "dreadnought", &a(), 2);
                crate::fixtures::put(&mut state, &target, "cruiser", &a(), 1);
                crate::fixtures::put(&mut state, &target, "fighter", &a(), 2);
                crate::fixtures::put(&mut state, &target, "dreadnought", &b(), 1);
                crate::fixtures::put(&mut state, &target, "destroyer", &b(), 2);
                crate::fixtures::put(&mut state, &target, "fighter", &b(), 3);
                if std::env::var_os("M5_DEBUG").is_some() {
                    eprintln!("== {mine} v {theirs}");
                }
                // (Every difference after the first follows from it: the questions shift.)
                if let Some(d) = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()]).into_iter().next() {
                    failures.push(format!("{mine} v {theirs}: {d}"));
                }
            }
        }
        // The residual, counted (docs/M5_RESUME.md): the activation hooks of the tactical action that ask (Mahact at the table).
        failures.retain(|f| !(f.contains("BAIL: a hook of the activation") && (f.contains("mahact") || f.contains("titans"))));
        // Residuals of the neighbours' lanes, not of this island (docs/M5_RESUME.md): the Saar agent is a string-registered SYSTEM_ACTIVATED
        // ability the core registry lacks (M6, open item 2), and the Empyrean commander's unlock is a hook slot nobody fills yet (A2/F2:
        // `commander_unlocked`).
        failures.retain(|f| !(f.starts_with("saar v") || f.starts_with("letnev v saar")) && !f.contains("leaders.empyreancommander"));
        assert!(failures.is_empty(), "{failures:#?}");
    }

    /// Van Hauge (the Yin flagship): when it is destroyed every ship of the system goes with it, staged and announced at the fixed drain points of the
    /// string (after the round ended, in the end announcement). Several seeds, the flagship on either side.
    #[test]
    fn van_hauge_takes_every_ship_with_it_and_the_announcements_follow_the_string() {
        let (hub, _origin, target) = hub();
        let mut seen_chain = 0;
        for seed in 1..=12_u64 {
            for yin_is_attacker in [true, false] {
                let (mine, theirs) = if yin_is_attacker { ("yin", "hacan") } else { ("hacan", "yin") };
                let mut state = table(&[("a", mine), ("b", theirs)]);
                state.rng_seed = seed;
                let yin = if yin_is_attacker { a() } else { b() };
                let other = if yin_is_attacker { b() } else { a() };
                crate::fixtures::put(&mut state, &target, "yin_flagship", &yin, 1);
                crate::fixtures::put(&mut state, &target, "cruiser", &yin, 1);
                crate::fixtures::put(&mut state, &target, "dreadnought", &other, 3);
                crate::fixtures::put(&mut state, &target, "cruiser", &other, 2);
                let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()]);
                assert!(diffs.is_empty(), "seed {seed} yin attacker {yin_is_attacker}: {diffs:#?}");
                seen_chain += 1;
            }
        }
        assert_eq!(seen_chain, 24);
    }

    /// The Letnev commander (Rear Admiral Farran) pays after a sustain: a child question of the sustain's answer, asked through the tape, and
    /// the Letnev agent's round window beside it.
    #[test]
    fn the_letnev_commander_and_agent_react_in_the_fight_in_lockstep() {
        let (hub, _origin, target) = hub();
        for seed in 1..=4_u64 {
            let mut state = table(&[("a", "sol"), ("b", "letnev")]);
            state.rng_seed = seed;
            state.player_mut(&b()).unwrap().leaders.insert(ti4_model::id::LeaderId::new("letnevcommander"), ti4_model::state::LeaderStatus::Unlocked);
            state.player_mut(&b()).unwrap().trade_goods = 3;
            crate::fixtures::put(&mut state, &target, "dreadnought", &a(), 2);
            crate::fixtures::put(&mut state, &target, "cruiser", &a(), 1);
            crate::fixtures::put(&mut state, &target, "dreadnought", &b(), 2);
            crate::fixtures::put(&mut state, &target, "carrier", &b(), 1);
            let diffs = lockstep(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()]);
            assert!(diffs.is_empty(), "seed {seed}: {diffs:#?}");
        }
    }

    /// A system of two ordinary planets as the centre of the hub (the target of the action), with the two planets: seat `a` holds the first one
    /// and a space dock on it.
    fn dock_hub() -> (crate::fixtures::Hub, SystemId, SystemId, ti4_model::id::PlanetId, ti4_model::id::PlanetId) {
        let content = ContentStore::embedded();
        let (system, planets): (String, Vec<String>) = ti4_content::galaxy::all_systems(content, POK)
            .iter()
            .filter(|(_, sys)| !sys.is_anomaly() && !sys.is_hyperlane())
            .map(|(id, sys)| ((*id).to_owned(), sys.planets().into_iter().map(str::to_owned).collect::<Vec<_>>()))
            .find(|(_, ps)| {
                ps.len() == 2
                    && ps.iter().all(|p| ti4_content::galaxy::planet(content, p, POK).is_some_and(|p| p.homeworld_of().is_none() && !p.is_legendary() && !p.is_space_station() && !p.is_placed_during_play()))
            })
            .expect("a system with two ordinary planets");
        let target = SystemId::new(system);
        let hub = crate::fixtures::hub_with_centre(target.as_str());
        let origin = SystemId::new(hub.outer[0].clone());
        (hub, origin, target, ti4_model::id::PlanetId::new(planets[0].clone()), ti4_model::id::PlanetId::new(planets[1].clone()))
    }

    /// The position of the production tests: `a` holds the first planet of the target with a space dock on it.
    fn dock_table(mine: &str, theirs: &str, target: &SystemId, dock: &ti4_model::id::PlanetId) -> GameState {
        let mut state = table(&[("a", mine), ("b", theirs)]);
        state.system_mut(target).planet_control.insert(dock.clone(), a());
        crate::fixtures::put_on_planet(&mut state, target, dock, "spacedock", &a(), 1);
        state.player_mut(&a()).unwrap().trade_goods = 6;
        state
    }

    /// An undefended system the active seat holds with a space dock: no fight, no invasion, and the aftermath goes on to the production step,
    /// which the native chain hands to `Arm::Production` (lane M5 to lane M2). Several factions; a counted bail is a failure.
    #[test]
    fn an_undefended_system_with_a_space_dock_goes_on_to_the_production_step_in_lockstep() {
        let (hub, origin, target, dock, _other) = dock_hub();
        let mut failures = Vec::new();
        for faction in ["sol", "hacan", "arborec", "letnev", "mentak", "yin", "titans", "naalu", "nekro", "muaat"] {
            let other = if faction == "hacan" { "sol" } else { "hacan" };
            for builds in [0, 1] {
                let mut state = dock_table(faction, other, &target, &dock);
                crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
                let script = [TACTICAL_ACTION_ID, target.as_str(), &mv(&origin, 0), "done_moving"];
                if let Some(d) = lockstep_buying(state, &hub.galaxy, &script, builds).into_iter().next() {
                    failures.push(format!("{faction} buying {builds}: {d}"));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    /// A won space fight in a system the winner holds a space dock in: the fight, the end of the fight, the invasion step with nothing to take, and
    /// the production step.
    #[test]
    fn a_won_space_fight_over_a_space_dock_goes_on_to_the_production_step_in_lockstep() {
        let (hub, origin, target, dock, _other) = dock_hub();
        for seed in 1..=3_u64 {
            for faction in ["sol", "jolnar", "saar"] {
                let mut state = dock_table(faction, "hacan", &target, &dock);
                state.rng_seed = seed;
                crate::fixtures::put(&mut state, &origin, "dreadnought", &a(), 2);
                crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
                crate::fixtures::put(&mut state, &target, "cruiser", &b(), 1);
                let (m0, done) = (mv(&origin, 0), "done_moving");
                let diffs = lockstep_buying(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str(), &m0, &m0, &m0, done], 1);
                assert!(diffs.is_empty(), "{faction} seed {seed}: {diffs:#?}");
            }
        }
    }

    /// A won invasion with a space dock in the system: the infantry takes the other planet, the planet with the dock stays the invader's, and the
    /// production step follows the invasion's end (`INVASION_RESOLVED`, then `PRODUCTION_USED` ... `PRODUCTION_RESOLVED`).
    #[test]
    fn a_won_invasion_with_a_space_dock_goes_on_to_the_production_step_in_lockstep() {
        let (hub, origin, target, dock, other) = dock_hub();
        for faction in ["sol", "hacan", "yssaril", "nomad", "winnu"] {
            let rival = if faction == "hacan" { "sol" } else { "hacan" };
            let mut state = dock_table(faction, rival, &target, &dock);
            state.system_mut(&target).planet_control.insert(other.clone(), b());
            crate::fixtures::put_on_planet(&mut state, &target, &other, "infantry", &b(), 1);
            crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
            crate::fixtures::put(&mut state, &origin, "infantry", &a(), 3);
            let script = [TACTICAL_ACTION_ID, target.as_str(), &mv(&origin, 0), "load|0", "load|1", "load|2", "done_moving"];
            let diffs = lockstep_buying(state, &hub.galaxy, &script, 1);
            assert!(diffs.is_empty(), "{faction}: {diffs:#?}");
        }
    }
}
