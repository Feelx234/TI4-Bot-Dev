//! The engine half of the native invasion (lane M3, `docs/M3_RESUME.md`): the texts the core composer needs from outside
//! (`driver::windows::invasion` has no text of its own) and the lockstep tests that run the same invasion twice, once as the string
//! `InvasionWindow` and once as the native `Arm::Invasion` through `live_step`, and compare the questions, the answers and the positions.

use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::state::Code;

use crate::generated::IdStr;
use crate::render::Render;

/// Fill the invasion texts of `table`. `state_code` mints a text of the position (`InvasionStep.kind`, a reroll kind, the dice groups: the state's
/// text table); the texts of events and questions are the render table's.
pub fn fill_codes(table: &mut AbilityTable, r: &Render, state_code: &mut dyn FnMut(&str) -> Code) {
    let c = &mut table.codes.invasion;
    c.ground = state_code("ground");
    c.bombardment = state_code("bombardment");
    c.ground_round = state_code("ground_round");
    c.space_cannon_defense = state_code("space_cannon_defense");
    c.bombardment_cause = r.code("bombardment");
    c.space_cannon_cause = r.code("space_cannon_defense");
    c.planets = r.code("planets");
    c.dunlain_reaper = r.code("dunlain_reaper");
    for (slot, text) in c.phases.iter_mut().zip(["bombardment", "custodians", "landing", "ground_battle", "planet_result", "next_defender", "control", "complete"]) {
        *slot = state_code(text);
    }
    for (i, slot) in c.names.iter_mut().enumerate() {
        *slot = state_code(&format!("combat value {}", i + 1));
    }
    for unit in ti4_engine_core::generated::Unit::ALL {
        c.units[unit as usize] = state_code(IdStr::as_str(unit));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use ti4_engine_core::driver::live::{is_open, Env};
    use ti4_engine_core::driver::session::Arm;
    use ti4_engine_core::driver::windows::invasion::{open_at_commit, open_invasion, InvasionMachine};
    use ti4_engine_core::state::{NoteHold, Slice, WindowKind};
    use ti4_model::content_types::POK;
    use ti4_model::id::{PlayerId, SystemId};

    use super::*;
    use crate::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Table, Window as _};
    use crate::core_sync::codec::rng::TextSeeds;
    use crate::core_sync::{with_core, Footprint, Sources};
    use crate::dice::Dice;
    use crate::invasion::InvasionWindow;
    use crate::live::{live_step, Cx, Scripted, Stop};
    use crate::rng::GameRng;
    use ti4_model::state::GameState;

    /// Records every question and answers with `pick` (the first option by default).
    struct Recorder {
        asked: Rc<RefCell<Vec<Choice>>>,
        answers: Rc<RefCell<Vec<String>>>,
        pick: Box<dyn FnMut(&Choice) -> usize>,
    }

    impl Decider for Recorder {
        fn choose(&mut self, c: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            let i = (self.pick)(c).min(c.options.len() - 1);
            self.asked.borrow_mut().push(c.clone());
            self.answers.borrow_mut().push(c.options[i].id.clone());
            Ok(c.options[i].clone())
        }
    }

    /// What one run left: the questions in order, the answers, the position.
    struct Run {
        asked: Vec<Choice>,
        answers: Vec<String>,
        state: GameState,
    }

    fn armed_resolver(state: &GameState, players: Vec<PlayerId>) -> crate::timing::Resolver {
        let mut resolver = crate::timing::Resolver::new(players, Some(PlayerId::new("a")), crate::choice::Table::default());
        crate::reactions::arm(&mut resolver, state);
        resolver
    }

    /// The string run: the aftermath's opening (`active_invasion`, `INVASION_BEGAN`), the window, every question answered by `pick`.
    fn string_run(mut state: GameState, seed: u64, system: &SystemId, invader: &str, pick: Box<dyn FnMut(&Choice) -> usize>) -> Run {
        let asked = Rc::new(RefCell::new(Vec::new()));
        let answers = Rc::new(RefCell::new(Vec::new()));
        let mut table = Table::with_default(Box::new(Recorder { asked: asked.clone(), answers: answers.clone(), pick }));
        let mut dice = Dice::new();
        let mut rng = GameRng::new(seed);
        let content = ti4_content::ContentStore::embedded();
        let order: Vec<PlayerId> = state.players.iter().map(|p| p.id.clone()).collect();
        let mut resolver = armed_resolver(&state, order);
        let mut sequence = crate::event::EventSequence::new();
        let invader = PlayerId::new(invader);
        state.active_invasion = Some(ti4_model::state::ActiveInvasion {
            system: system.clone(),
            invader: invader.clone(),
            seq: u64::from(state.activation_seq),
            phase: "bombardment".to_owned(),
            planet: None,
            defender: None,
            ground_round: 0,
            last_step: None,
        });
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
        };
        let mut payload = std::collections::BTreeMap::new();
        payload.insert("player".to_owned(), serde_json::Value::String(invader.to_string()));
        payload.insert("system".to_owned(), serde_json::Value::String(system.to_string()));
        let _ = ctx.emit(&mut state, "INVASION_BEGAN", payload);
        let notes = crate::combat::note_holdings(&state);
        let (dice_ref, rng_ref) = (&mut *ctx.dice, &mut *ctx.rng);
        let mut window = InvasionWindow::new_with_notes(&mut state, content, POK, dice_ref, rng_ref, &invader, system, notes);
        let mut guard = 0;
        while !window.is_done() {
            window.drive(&mut state, &mut ctx).expect("the string run is legal");
            while window.take_scoring_occurrence().is_some() {}
            window.settle(&mut state, &mut ctx);
            guard += 1;
            assert!(guard < 400, "the string invasion does not end");
        }
        state.active_invasion = None;
        drop(ctx);
        let (asked, answers) = (asked.borrow().clone(), answers.borrow().clone());
        Run { asked, answers, state }
    }

    const FP: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger, Slice::Strategy, Slice::Trade, Slice::Explore, Slice::Decks, Slice::Objectives],
        &[Slice::Meta, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger, Slice::Objectives],
    );

    /// The native run of the same invasion: the arm `Arm::Invasion` through `live_step`, answered by `answers` in order.
    fn live_run(mut gs: GameState, seed: u64, system: &SystemId, invader: &str, answers: &[String], hero: bool) -> Result<(Vec<Choice>, GameState, usize), Stop> {
        let (mut cx, r) = crate::dual_window::tables_for_tests(&gs).expect("a position the codecs carry");
        let rng_before = GameRng::new(seed).positions();
        let seeds: Rc<dyn ti4_engine_core::state::SeedSource> = Rc::new(TextSeeds::of(&cx));
        // The texts of the position the composer needs, minted in the state's table before the lend.
        let mut minted: Vec<(String, Code)> = Vec::new();
        let mut mint = |t: &str| -> Code {
            let c = cx.texts.intern(Slice::Transient, t).expect("room for the text");
            minted.push((t.to_owned(), c));
            c
        };
        let mut table = ti4_engine_core::kernel::stage::AbilityTable::default();
        fill_codes(&mut table, &r, &mut mint);
        let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str()).expect("a system");
        let seat = r.seat(invader).expect("a seat");
        let src = Sources { galaxy: None, rng: Some(&rng_before) };
        let mut asked = Vec::new();
        let mut steps = 0;
        let mut outcome: Result<(), Stop> = Ok(());
        let r_ref = &r;
        with_core(&mut gs, &mut cx, src, FP, |core, _| {
            let kernel = crate::live::kernel::kernel_for(POK);
            let mut env: Env<'_> = crate::live::env::env_for(r_ref, core, kernel, seeds.clone());
            env.table.codes.invasion = table.codes.invasion;
            if hero {
                open_at_commit(core, seat, sys, NoteHold::new());
            } else {
                open_invasion(core, seat, sys, NoteHold::new());
            }
            let live = Cx::new(r_ref, &env);
            let mut asker = Scripted::new(answers.iter().cloned());
            loop {
                steps += 1;
                if steps > 400 {
                    outcome = Err(Stop::Blocked);
                    break;
                }
                match live_step(core, Arm::Invasion, &live, &mut asker) {
                    Ok(step) => {
                        asked.extend(step.asked);
                        // The event scoring window of a pause: nobody has anything to score here, so it closes at once.
                        core.pending_mut().close(WindowKind::EventScoring);
                        if !is_open::<InvasionMachine>(core) {
                            break;
                        }
                    }
                    Err(stop) => {
                        asked.extend(asker.seen.clone());
                        outcome = Err(stop);
                        break;
                    }
                }
            }
        })
        .expect("lendable");
        outcome.map(|()| (asked, gs, steps))
    }

    fn assert_same_questions(live: &[Choice], string: &[Choice]) {
        assert_eq!(live.len(), string.len(), "live {:?}\nstring {:?}", live.iter().map(|c| &c.prompt).collect::<Vec<_>>(), string.iter().map(|c| &c.prompt).collect::<Vec<_>>());
        for (i, (l, s)) in live.iter().zip(string).enumerate() {
            assert_eq!(l.player, s.player, "question {i}");
            assert_eq!(l.prompt, s.prompt, "question {i}");
            assert_eq!(l.options, s.options, "question {i}: {}", s.prompt);
            assert_eq!(l.context, s.context, "question {i}: {}", s.prompt);
        }
    }

    fn game(seats: &[(&str, &str)], seed: u64) -> (GameState, SystemId, ti4_model::id::PlanetId) {
        let mut state = crate::fixtures::seated_dealt(seats, POK);
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        state.rng_seed = seed;
        let (system, planet) = crate::fixtures::a_placed_planet();
        (state, system, planet)
    }

    /// A conquest of a planet another player holds (no first claim, no exploration): the questions, the answers and the position agree with the
    /// string window's, for several factions of the invader and several seeds.
    #[test]
    fn a_conquest_runs_natively_with_the_questions_and_the_position_of_the_string_window() {
        let _guard = crate::dual::tests::serial();
        let mut compared = 0;
        for (seed, invader) in (1..=6_u64).zip(["hacan", "sardakk", "mentak", "letnev", "sol", "naalu"]) {
            let (mut state, system, planet) = game(&[("a", invader), ("b", "jolnar")], seed);
            let a = PlayerId::new("a");
            let b = PlayerId::new("b");
            crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
            crate::fixtures::put(&mut state, &system, "infantry", &a, 6);
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 2);
            let string = string_run(state.clone(), seed, &system, "a", Box::new(|_| 0));
            let (asked, gs, _) = match live_run(state.clone(), seed, &system, "a", &string.answers, false) {
                Ok(r) => r,
                Err(stop) => panic!("{invader} seed {seed}: the native run stopped: {stop:?}"),
            };
            assert_same_questions(&asked, &string.asked);
            for (id, label) in [(&a, "a"), (&b, "b")] {
                let (l, s) = (gs.player(id).unwrap(), string.state.player(id).unwrap());
                assert_eq!((l.victory_points, l.trade_goods), (s.victory_points, s.trade_goods), "{invader} {label}");
            }
            assert_eq!(gs.system_state(&system).planet_units, string.state.system_state(&system).planet_units, "{invader} seed {seed}: the units on the planets");
            assert_eq!(gs.system_state(&system).planet_control, string.state.system_state(&system).planet_control, "{invader} seed {seed}: the control");
            compared += 1;
        }
        assert_eq!(compared, 6);
    }
}
