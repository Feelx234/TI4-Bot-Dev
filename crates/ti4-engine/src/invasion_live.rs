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
    c.minister = r.code("minister_exploration");
    for (slot, text) in c.phases.iter_mut().zip(["bombardment", "custodians", "landing", "ground_battle", "planet_result", "next_defender", "control", "complete"]) {
        *slot = state_code(text);
    }
    for (i, slot) in c.names.iter_mut().enumerate() {
        *slot = state_code(&format!("combat value {}", i + 1));
    }
    for unit in ti4_engine_core::generated::Unit::ALL {
        c.units[unit as usize] = state_code(IdStr::as_str(unit));
        c.shown_units[unit as usize] = r.code(IdStr::as_str(unit));
    }
}

/// The words of a position the native invasion does not run (`Unknown::Unported`).
#[must_use]
pub const fn unported_text(why: ti4_engine_core::kernel::ability::Unported) -> &'static str {
    use ti4_engine_core::kernel::ability::Unported as U;
    match why {
        U::NoProgress => "the invasion made no progress (a stage that neither asks nor moves on)",
        U::Slumberstate => "Slumberstate Computing (a Thunder's Edge breakthrough)",
        U::RollCannotName => "a pool or unit the ground roll cannot name",
        U::IntegratedEconomy => "Integrated Economy (a triggered production nested in the control gain)",
        U::AssimilateRoster => "an Assimilate seat whose unit forms the core cannot name",
        U::Exploration(why) => {
            use ti4_engine_core::state::EWhy as W;
            match why {
                W::NoTexts => "the exploration of a first claim, in a table with no texts for its log row",
                W::NoLink => "the exploration of a first claim, in a table with no link to a child window or no unit forms",
                W::LinkedAsks => "the exploration of a first claim, a card whose child window of another area asks a question (the discard of a draw over the hand limit, the secret objective's hand limit)",
                W::Freelancers => "the exploration of a first claim: Freelancers (a unit produced with influence)",
                W::Terragenesis => "the exploration of a first claim by the Titans (Terragenesis is not a rule of the core table)",
                W::Replenish => "the exploration of a first claim: a replenishment the core does not follow (two seats of one faction)",
                W::Draw => "the exploration of a first claim: a draw the core does not carry",
            }
        }
        U::BombardmentDiceMismatch => "the bombardment dice do not match the plan",
        U::X89 => "X-89 Bacterial Weapon in a ground round",
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
        /// The scoring occurrences the invasion queued, in order: (occurrence, combat).
        scoring: Vec<(u64, bool)>,
        /// The events the invasion announced: (name, payload), in order.
        events: Vec<(String, std::collections::BTreeMap<String, serde_json::Value>)>,
    }

    fn armed_resolver(state: &GameState, players: Vec<PlayerId>) -> crate::timing::Resolver {
        let mut resolver = crate::timing::Resolver::new(players, Some(PlayerId::new("a")), crate::choice::Table::default());
        crate::reactions::arm(&mut resolver, state);
        resolver
    }

    /// The string run: the aftermath's opening (`active_invasion`, `INVASION_BEGAN`), the window, every question answered by `pick`.
    fn string_run(mut state: GameState, seed: u64, system: &SystemId, invader: &str, hero: bool, pick: Box<dyn FnMut(&Choice) -> usize>) -> Run {
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
        let notes = crate::combat::note_holdings(&state);
        let mut window = if hero {
            InvasionWindow::at_commit_step(&mut state, &invader, system)
        } else {
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
            let mut payload = std::collections::BTreeMap::new();
            payload.insert("player".to_owned(), serde_json::Value::String(invader.to_string()));
            payload.insert("system".to_owned(), serde_json::Value::String(system.to_string()));
            let mut ctx = crate::choice::Resolving {
                content,
                sources: POK,
                dice: &mut dice,
                rng: &mut rng,
                table: &mut table,
                timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
            };
            let _ = ctx.emit(&mut state, "INVASION_BEGAN", payload);
            InvasionWindow::new_with_notes(&mut state, content, POK, ctx.dice, ctx.rng, &invader, system, notes)
        };
        let mut ctx = crate::choice::Resolving {
            content,
            sources: POK,
            dice: &mut dice,
            rng: &mut rng,
            table: &mut table,
            timing: Some(crate::choice::TimingHandle { resolver: &mut resolver, sequence: &mut sequence, galaxy: None }),
        };
        let mut guard = 0;
        let mut scoring: Vec<(u64, bool)> = Vec::new();
        while !window.is_done() {
            window.drive(&mut state, &mut ctx).expect("the string run is legal");
            while let Some((o, combat)) = window.take_scoring_occurrence() {
                scoring.push((o.0, combat));
            }
            window.settle(&mut state, &mut ctx);
            guard += 1;
            assert!(guard < 400, "the string invasion does not end");
        }
        if std::env::var_os("M3_DEBUG").is_some() {
            eprintln!("STRING DICE {:?}", ctx.dice.history().iter().map(|r| (r.reason.clone(), r.hits_on, r.faces.clone())).collect::<Vec<_>>());
        }
        state.active_invasion = None;
        drop(ctx);
        let (asked, answers) = (asked.borrow().clone(), answers.borrow().clone());
        let events = resolver.applied_events().iter().map(|e| (e.event_type.clone(), e.payload.clone())).collect();
        Run { asked, answers, state, scoring, events }
    }

    /// What the native run of an invasion leaves: the questions, the position, the dice (seed, ok) and the events with their payloads.
    type LiveRun = (Vec<Choice>, GameState, Vec<(u64, bool)>, Vec<(String, std::collections::BTreeMap<String, serde_json::Value>)>);

    const FP: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger, Slice::Strategy, Slice::Trade, Slice::Explore, Slice::Decks, Slice::Objectives],
        &[Slice::Meta, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger, Slice::Objectives, Slice::Decks, Slice::Explore, Slice::Agenda],
    );

    /// The native run of the same invasion: the arm `Arm::Invasion` through `live_step`, answered by `answers` in order.
    fn live_run(mut gs: GameState, seed: u64, system: &SystemId, invader: &str, answers: &[String], hero: bool) -> Result<LiveRun, (Stop, Vec<Choice>)> {
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
        let mut native_dice: Vec<(Option<u8>, Vec<u8>)> = Vec::new();
        let mut steps = 0;
        let mut windows: Vec<(u64, bool)> = Vec::new();
        let mut events: Vec<(String, std::collections::BTreeMap<String, serde_json::Value>)> = Vec::new();
        let mut outcome: Result<(), Stop> = Ok(());
        let mut seen_all: Vec<Choice> = Vec::new();
        let r_ref = &r;
        with_core(&mut gs, &mut cx, src, FP, |core, _| {
            let kernel = crate::live::kernel::kernel_for(POK);
            let mut env: Env<'_> = crate::live::env::env_with_plays(r_ref, core, kernel, seeds.clone(), POK);
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
                        for e in &step.out.emitted {
                            if let Some(ev) = crate::dual_window::event_of(r_ref, e) {
                                events.push((ev.event_type, ev.payload));
                            }
                        }
                        // The event scoring window of a pause: nobody has anything to score here, so it closes at once.
                        for w in &step.out.opened {
                            if let ti4_engine_core::state::Window::EventScoring(sc) = w {
                                windows.push((sc.occurrence.unwrap_or(0), sc.one_per_player));
                            }
                        }
                        core.pending_mut().close(WindowKind::EventScoring);
                        if !is_open::<InvasionMachine>(core) {
                            native_dice = core.ledger().dice.iter().map(|d| (d.hits_on, d.faces.iter().copied().collect())).collect();
                            break;
                        }
                    }
                    Err(stop) => {
                        native_dice = core.ledger().dice.iter().map(|d| (d.hits_on, d.faces.iter().copied().collect())).collect();
                        seen_all = asker.seen.clone();
                        outcome = Err(stop);
                        break;
                    }
                }
            }
        })
        .expect("lendable");
        if std::env::var_os("M3_DEBUG").is_some() {
            eprintln!("NATIVE DICE {native_dice:?}");
        }
        match outcome {
            Ok(()) => Ok((asked, gs, windows, events)),
            Err(stop) => Err((stop, if seen_all.is_empty() { asked } else { seen_all })),
        }
    }

    include!("invasion_live_g3_tests.rs");
    include!("invasion_live_g4_tests.rs");

    fn assert_same_questions(live: &[Choice], string: &[Choice]) {
        assert_eq!(live.len(), string.len(), "live {:?}\nstring {:?}", live.iter().map(|c| &c.prompt).collect::<Vec<_>>(), string.iter().map(|c| &c.prompt).collect::<Vec<_>>());
        for (i, (l, s)) in live.iter().zip(string).enumerate() {
            assert_eq!(l.player, s.player, "question {i}");
            assert_eq!(l.prompt, s.prompt, "question {i}");
            assert_eq!(l.options, s.options, "question {i}: {}", s.prompt);
            assert_eq!(l.context, s.context, "question {i}: {}", s.prompt);
            assert_eq!(l.details, s.details, "question {i}: {}", s.prompt);
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

    /// Pick the first option whose id starts with one of `prefer` (in the order given), else the first option.
    fn prefer(prefer: &'static [&'static str]) -> Box<dyn FnMut(&Choice) -> usize> {
        Box::new(move |c| prefer.iter().find_map(|p| c.options.iter().position(|o| o.id.starts_with(p))).unwrap_or(0))
    }

    /// Run `state` both ways and compare the questions, the position of the planets, the points and the control.
    fn check(label: &str, state: &GameState, seed: u64, system: &SystemId, hero: bool, pick: Box<dyn FnMut(&Choice) -> usize>) -> usize {
        check_ignoring(label, state, seed, system, hero, pick, &[])
    }

    /// [`check`] with the events named in `ignore` left out of the comparison of the announced events (the native run lists the top-level emissions only: what an
    /// ability announced inside a window is not among them).
    #[allow(clippy::too_many_arguments, reason = "the position, the run and the events left out")]
    fn check_ignoring(label: &str, state: &GameState, seed: u64, system: &SystemId, hero: bool, pick: Box<dyn FnMut(&Choice) -> usize>, ignore: &[&str]) -> usize {
        let string = string_run(state.clone(), seed, system, "a", hero, pick);
        let mut padded = string.answers.clone();
        padded.extend(std::iter::repeat_n("fight".to_owned(), 6));
        crate::multiask_dual::remember_string_questions(string.asked.clone());
        let (asked, gs, windows, events) = match live_run(state.clone(), seed, system, "a", &padded, hero) {
            Ok(r) => r,
            Err((stop, seen)) => panic!(
                "{label}: the native run stopped: {stop:?}\nnative {:?}\nstring {:?}",
                seen.iter().map(|c| (&c.prompt, c.options.iter().map(|o| o.id.as_str()).collect::<Vec<_>>())).collect::<Vec<_>>(),
                string.asked.iter().map(|c| (&c.prompt, c.options.iter().map(|o| o.id.as_str()).collect::<Vec<_>>())).collect::<Vec<_>>()
            ),
        };
        if std::env::var_os("M3_DEBUG").is_some() {
            eprintln!("QUESTIONS {label}: {:?}", string.asked.iter().map(|c| (&c.prompt, c.options.iter().map(|o| o.id.as_str()).collect::<Vec<_>>())).collect::<Vec<_>>());
        }
        let (string_asked, asked_aligned) = crate::multiask_dual::align(&string.asked, &string.answers, &asked);
        assert_same_questions(&asked_aligned, &string_asked);
        // The announcement of a card declared in a window of the invasion (`ACTION_CARD_PLAYED`, `ACTION_CARD_DISCARDED`) is compared by the card's
        // effect on the position below (the hand, the structures), not as an event of the invasion. The events named in `ignore` (a native run lists
        // top-level emissions only) are left out as well: both filters apply.
        let kept = |all: &[(String, std::collections::BTreeMap<String, serde_json::Value>)]| {
            all.iter().filter(|(name, _)| !ignore.contains(&name.as_str()) && name != "ACTION_CARD_PLAYED" && name != "ACTION_CARD_DISCARDED").cloned().collect::<Vec<_>>()
        };
        assert_eq!(kept(&events), kept(&string.events), "{label}: the events the invasion announced");
        assert_eq!(windows, string.scoring, "{label}: the scoring windows the invasion pauses for");
        assert_eq!(gs.feat_occurrence_seq, string.state.feat_occurrence_seq, "{label}: the feat occurrences");
        assert!(gs.active_invasion.is_none() && string.state.active_invasion.is_none());
        // The numbered round counter: the native fight bumped it once more than the string's (lane M5's finding).
        assert_eq!(gs.combat_round_seq, string.state.combat_round_seq, "{label}: the combat round counter after the fight (before {}, rules version {}, events {:?})", state.combat_round_seq, state.rules_version, string.events);
        for id in state.players.iter().map(|p| &p.id) {
            let (l, s) = (gs.player(id).unwrap(), string.state.player(id).unwrap());
            assert_eq!((l.victory_points, l.trade_goods), (s.victory_points, s.trade_goods), "{label} {id}");
            assert_eq!(l.event_feats, s.event_feats, "{label} {id}: the feats");
            // What an exploration card can change: commodities, command tokens, cards, fragments, relics, the play area.
            assert_eq!(
                (l.commodities, l.tactic_tokens, l.fleet_tokens, l.strategic_tokens, &l.action_cards, &l.secret_objectives, &l.relics, &l.exploration_cards, &l.relic_fragments),
                (s.commodities, s.tactic_tokens, s.fleet_tokens, s.strategic_tokens, &s.action_cards, &s.secret_objectives, &s.relics, &s.exploration_cards, &s.relic_fragments),
                "{label} {id}: what the exploration gave"
            );
        }
        assert_eq!(gs.laws, string.state.laws, "{label}: the laws (the Crown of Emphidia changes hands)");
        assert_eq!(gs.exploration_log, string.state.exploration_log, "{label}: the exploration log");
        assert_eq!(gs.exploration_decks, string.state.exploration_decks, "{label}: the exploration decks");
        assert_eq!(gs.planet_attachments, string.state.planet_attachments, "{label}: the attachments");
        assert_eq!((&gs.wormhole_tokens, &gs.ion_storm), (&string.state.wormhole_tokens, &string.state.ion_storm), "{label}: the tokens an exploration placed");
        assert_eq!(gs.exhausted_planets, string.state.exhausted_planets, "{label}: the exhausted planets");
        assert_eq!(gs.system_state(system).planet_units, string.state.system_state(system).planet_units, "{label}: the units on the planets");
        assert_eq!(gs.system_state(system).units, string.state.system_state(system).units, "{label}: the units in space");
        assert_eq!(gs.system_state(system).planet_control, string.state.system_state(system).planet_control, "{label}: the control");
        assert_eq!(gs.exhausted_planets, string.state.exhausted_planets, "{label}: the exhausted planets");
        asked.len()
    }

    /// A conquest of a planet another player holds (no first claim, no exploration): the questions, the answers and the position agree with the
    /// string window's, for several factions of the invader and several seeds.
    #[test]
    fn a_conquest_runs_natively_with_the_questions_and_the_position_of_the_string_window() {
        let _guard = crate::dual::tests::serial();
        for (seed, invader) in (1..=18_u64).zip(["hacan", "sardakk", "mentak", "letnev", "sol", "naalu"].into_iter().cycle()) {
            let (mut state, system, planet) = game(&[("a", invader), ("b", "jolnar")], seed);
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
            // The invader wins with six against two; with three against four it loses or both are wiped out in some seeds.
            let (n, d) = if seed % 3 == 0 { (3, 4) } else { (6, 2) };
            crate::fixtures::put(&mut state, &system, "infantry", &a, n);
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, d);
            check(invader, &state, seed, &system, false, prefer(&[]));
        }
    }

    /// Valkyrie Particle Weave ("after making combat rolls during a round of ground combat, if your opponent produced 1 or more hits, you produce 1 additional
    /// hit"; it was `Unported(HitsAddedToARoll)` and stopped 31 native bench games): the added hit is assigned by the opponent like any other, native and
    /// string agree, with the weave on the invading side, on the defending side and on both.
    #[test]
    fn valkyrie_particle_weave_adds_its_hit_natively_in_lockstep_with_the_string_window() {
        let _guard = crate::dual::tests::serial();
        for (seed, (invader, defender)) in (1..=12_u64).zip([("sardakk", "jolnar"), ("hacan", "sardakk"), ("sardakk", "sardakk")].into_iter().cycle()) {
            let (mut state, system, planet) = game(&[("a", invader), ("b", defender)], seed);
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            for (who, faction) in [(&a, invader), (&b, defender)] {
                if faction == "sardakk" {
                    state.player_mut(who).unwrap().technologies.insert(ti4_model::TechnologyId::new("vpw"));
                }
            }
            crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
            crate::fixtures::put(&mut state, &system, "infantry", &a, 5);
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 4);
            check(&format!("{invader} v {defender}"), &state, seed, &system, false, prefer(&[]));
        }
    }

    /// Lane L: a planet taken while the invader owns Integrated Economy (it was `Unported(IntegratedEconomy)`): the production on the planet gained is a
    /// window of the control gain, asked and paid on the native path as the string's `technology::control_gained` does, for several seeds.
    #[test]
    fn a_conquest_with_integrated_economy_asks_the_production_natively_as_the_string_window_does() {
        let _guard = crate::dual::tests::serial();
        for (seed, invader) in (1..=6_u64).zip(["hacan", "sardakk", "mentak", "letnev", "sol", "naalu"]) {
            let (mut state, system, planet) = game(&[("a", invader), ("b", "jolnar")], seed);
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
            crate::fixtures::put(&mut state, &system, "infantry", &a, 6);
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 1);
            state.player_mut(&a).unwrap().technologies.insert(ti4_model::TechnologyId::new("ie"));
            state.player_mut(&a).unwrap().trade_goods = 10;
            check(invader, &state, seed, &system, false, prefer(&[]));
        }
    }

    /// Infiltrate, held by the invader: when the planet is taken, the native invasion's control step opens the window the card is played in (the play
    /// facts and the previous owner's structures are the position's), and the previous owner's space dock and PDS become the invader's, as the
    /// string window does it.
    #[test]
    fn infiltrate_is_played_when_the_invader_takes_a_planet_natively_as_the_string_does() {
        let _guard = crate::dual::tests::serial();
        let mut played = 0;
        for (seed, invader) in (1..=6_u64).zip(["hacan", "sol", "naalu", "letnev", "mentak", "sardakk"]) {
            let (mut state, system, planet) = game(&[("a", invader), ("b", "jolnar")], seed);
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
            crate::fixtures::put(&mut state, &system, "infantry", &a, 6);
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "spacedock", &b, 1);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "pds", &b, 1);
            state.player_mut(&a).unwrap().action_cards.push(ti4_model::id::ActionCardId::new("infiltrate"));
            let asked = check(invader, &state, seed, &system, false, prefer(&["reaction:", "infiltrate"]));
            played += usize::from(asked > 0);
        }
        assert!(played > 0);
    }

    /// Bombardment (dreadnoughts), the space cannon defense of a PDS and a long fight, against several defender factions.
    #[test]
    fn bombardment_and_the_space_cannon_defense_run_natively() {
        let _guard = crate::dual::tests::serial();
        let mut asked = 0;
        for (seed, defender) in (1..=6_u64).zip(["letnev", "hacan", "sol", "jolnar", "mentak", "sardakk"]) {
            let (mut state, system, planet) = game(&[("a", "naalu"), ("b", defender)], seed);
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            crate::fixtures::put(&mut state, &system, "dreadnought", &a, 2);
            crate::fixtures::put(&mut state, &system, "infantry", &a, 5);
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 3);
            if seed % 2 == 0 {
                crate::fixtures::put_on_planet(&mut state, &system, &planet, "pds", &b, 1);
            }
            asked += check(defender, &state, seed, &system, false, prefer(&[]));
        }
        assert!(asked > 6);
    }

    /// L4 Disruptors silence the PDS against their owner, and an attachment that gives its planet SPACE CANNON (Titans' Geoform) rolls for the
    /// planet's controller: both in lockstep with the string window (the native defense bailed on them before).
    #[test]
    fn the_space_cannon_defense_follows_l4_disruptors_and_attachment_cannons_natively() {
        use crate::dual::{take_report, with_bail_is_failure};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut with_cannon = 0;
        for seed in 1..=10_u64 {
            let (mut state, system, planet) = game(&[("a", if seed % 2 == 0 { "letnev" } else { "naalu" }), ("b", "hacan")], seed);
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
            crate::fixtures::put(&mut state, &system, "infantry", &a, 6);
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 2);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "pds", &b, 2);
            if seed % 2 == 0 {
                state.player_mut(&a).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("l4"));
            }
            if seed % 3 != 0 {
                state.planet_attachments.entry(planet.clone()).or_default().push("titanshero".to_owned());
                with_cannon += 1;
            }
            with_bail_is_failure(true, || check("defense", &state, seed, &system, false, prefer(&[])));
        }
        assert!(with_cannon > 5);
        let _ = take_report();
    }

    /// Package W16: the invader's Jol-Nar commander is asked about each die of the bombardment by the native invasion (it was the counted bail
    /// `Unported::JolNarCommanderReroll`): the questions, their options and context, the dice drawn again and the position after the fight
    /// agree with the string window's, for a decider that rerolls some dice and keeps the others, under several seeds and defender factions.
    #[test]
    fn the_jolnar_commander_rerolls_the_dice_of_a_bombardment_natively() {
        let _guard = crate::dual::tests::serial();
        // (Not the Arborec: this test keeps the tables it was written for. Magen Defense Grid's hit at the start of a ground combat, `technology:*:md_hit`,
        // is a native rule now, `silent::g4::magen_hit`.)
        let mut asked = 0;
        for (seed, defender) in (1..=8_u64).zip(["letnev", "hacan", "sol", "mentak", "sardakk", "naalu", "xxcha", "yssaril"]) {
            let (mut state, system, planet) = game(&[("a", "jolnar"), ("b", defender)], seed);
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            state.player_mut(&a).unwrap().leaders.insert(ti4_model::id::LeaderId::new("jolnarcommander"), ti4_model::state::LeaderStatus::Unlocked);
            crate::fixtures::put(&mut state, &system, "dreadnought", &a, 2);
            crate::fixtures::put(&mut state, &system, "infantry", &a, 5);
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 3);
            let pick = |n: u64| -> Box<dyn FnMut(&Choice) -> usize> {
                let mut count = n;
                Box::new(move |c| {
                    if c.context.as_ref().is_some_and(|x| x.subtype == "jolnar_commander_reroll") {
                        count += 1;
                        // Reroll every other die (the reroll option comes first, then the decline).
                        return usize::from(count % 2 == 0);
                    }
                    0
                })
            };
            asked += check(defender, &state, seed, &system, false, pick(seed));
            let (string_asked, _) = (string_run(state.clone(), seed, &system, "a", false, pick(seed)).asked, ());
            let walk = string_asked.iter().filter(|c| c.context.as_ref().is_some_and(|x| x.subtype == "jolnar_commander_reroll")).count();
            assert!(walk >= 2, "seed {seed}: one question for each die of the bombardment: {walk}");
        }
        assert!(asked > 16);
    }

    /// The custodians offer on Mecatol Rex: declined, paid (the payment asks which planets), and the hero's opening at the commit step.
    #[test]
    fn the_custodians_offer_runs_natively_declined_and_paid() {
        let _guard = crate::dual::tests::serial();
        let mecatol = SystemId::new(crate::seating::MECATOL);
        for (seed, (faction, choice)) in [("sol", &["no"][..]), ("sol", &["yes", "exhaust|"][..]), ("winnu", &["yes"][..])].into_iter().enumerate() {
            let mut state = crate::fixtures::seated_dealt(&[("a", faction), ("b", "letnev")], POK);
            state.promissory_notes.clear();
            state.promissory_faceup.clear();
            state.rng_seed = seed as u64 + 1;
            state.board.entry(mecatol.clone()).or_default();
            crate::fixtures::put(&mut state, &mecatol, "infantry", &PlayerId::new("a"), 2);
            state.player_mut(&PlayerId::new("a")).unwrap().trade_goods = 4;
            check(faction, &state, seed as u64 + 1, &mecatol, true, prefer(choice));
        }
    }

    /// Letnev's Dunlain Reaper at the start of each ground round: the offer, the payment (a child bill resumable from the payload), declined, unaffordable
    /// and with no mech in the reinforcements; the Letnev seat is the defender or the invader.
    #[test]
    fn the_dunlain_reaper_runs_natively_offered_paid_and_declined() {
        let _guard = crate::dual::tests::serial();
        let mut paid = 0;
        for seed in 1..=18_u64 {
            let invader_letnev = seed % 2 == 0;
            let seats = if invader_letnev { [("a", "letnev"), ("b", "hacan")] } else { [("a", "hacan"), ("b", "letnev")] };
            let (mut state, system, planet) = game(&seats, seed);
            let (l, other) = if invader_letnev { ("a", "b") } else { ("b", "a") };
            crate::fixtures::put(&mut state, &system, "carrier", &PlayerId::new("a"), 1);
            crate::fixtures::put(&mut state, &system, "infantry", &PlayerId::new("a"), 6);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &PlayerId::new("b"), 4);
            if !invader_letnev {
                state.system_mut(&system).set_control(planet.clone(), PlayerId::new("b"));
            }
            let poor = seed % 9 >= 7 || seed % 9 == 0;
            state.player_mut(&PlayerId::new(l)).unwrap().trade_goods = if poor { 0 } else { 5 };
            if poor {
                for (_, p) in state.controlled_planets(&PlayerId::new(l)).into_iter().map(|(s, p)| (s.clone(), p.clone())).collect::<Vec<_>>() {
                    state.exhausted_planets.insert(p);
                }
            }
            if seed % 9 == 4 {
                let home = state.player(&PlayerId::new(other)).unwrap().home_system.clone().unwrap();
                let at = state.system_state(&home).planet_control.keys().next().cloned().unwrap();
                crate::fixtures::put_on_planet(&mut state, &home, &at, "letnev_mech", &PlayerId::new(l), 4);
            }
            let decline = seed % 3 == 1;
            let n = check(
                "reaper",
                &state,
                seed,
                &system,
                false,
                Box::new(move |c| if c.prompt.starts_with("Dunlain Reaper") && decline { c.options.len() - 1 } else { 0 }),
            );
            paid += usize::from(n > 8);
        }
        assert!(paid > 0);
    }

    /// The capture of a home planet: the winner earns the feats of a rival's home system, the former holder the feat of a lost home planet, and the
    /// machine stops for the scoring windows in between (the lockstep closes them at once, as the string run ignores the occurrences).
    #[test]
    fn the_capture_of_a_home_planet_pauses_for_the_scoring_windows() {
        let _guard = crate::dual::tests::serial();
        for (seed, defender) in (1..=4_u64).zip(["hacan", "letnev", "sol", "mentak"]) {
            let (mut state, _, _) = game(&[("a", "naalu"), ("b", defender)], seed);
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            let home = state.player(&b).unwrap().home_system.clone().unwrap();
            let planet = state.system_state(&home).planet_control.keys().next().cloned().unwrap();
            crate::fixtures::put(&mut state, &home, "carrier", &a, 1);
            crate::fixtures::put(&mut state, &home, "infantry", &a, 9);
            check(defender, &state, seed, &home, false, prefer(&[]));
            let _ = planet;
        }
    }

    /// A Naaz mech that stands in space in its ship form lands in its ground form (the Z-Grav Eidolon flips as it is committed).
    #[test]
    fn a_ship_form_unit_flips_to_its_ground_form_as_it_lands() {
        use crate::dual::{take_report, with_bail_is_failure};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        for seed in 1..=4_u64 {
            let (mut state, system, planet) = game(&[("a", "naaz"), ("b", "jolnar")], seed);
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
            crate::fixtures::put(&mut state, &system, "naaz_mech_space", &a, 1);
            crate::fixtures::put(&mut state, &system, "infantry", &a, 3);
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 2);
            with_bail_is_failure(true, || check("naaz eidolon", &state, seed, &system, false, prefer(&[])));
        }
        let _ = take_report();
    }

    /// A planet taken pays the Minister of Exploration a trade good, and the Crown of Emphidia changes hands when the planet is in its owner's
    /// home system (the owner loses the point, the taker gains it).
    #[test]
    fn a_planet_taken_pays_the_minister_of_exploration_and_the_crown_of_emphidia_changes_hands() {
        use crate::dual::{take_report, with_bail_is_failure};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        for (seed, defender) in (1..=4_u64).zip(["hacan", "letnev", "sol", "mentak"]) {
            let (mut state, _, _) = game(&[("a", "naalu"), ("b", defender)], seed);
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            let home = state.player(&b).unwrap().home_system.clone().unwrap();
            crate::fixtures::put(&mut state, &home, "carrier", &a, 1);
            crate::fixtures::put(&mut state, &home, "infantry", &a, 9);
            state.laws.insert("minister_exploration".to_owned(), "a".to_owned());
            if seed % 2 == 1 {
                state.laws.insert("crown_of_emphidia".to_owned(), "b".to_owned());
                state.player_mut(&b).unwrap().victory_points = 2;
            }
            with_bail_is_failure(true, || check(defender, &state, seed, &home, false, prefer(&[])));
            let (string_goods, _) = (state.player(&a).unwrap().trade_goods, ());
            let _ = string_goods;
        }
        let _ = take_report();
    }

    /// A planet nobody holds whose printed traits are exactly `wanted` (lower case), with its system.
    fn free_planet(wanted: &[&str]) -> (SystemId, ti4_model::id::PlanetId) {
        let mut found: Vec<_> = ti4_content::galaxy::all_planets(ti4_content::ContentStore::embedded(), POK)
            .iter()
            .filter(|(id, p)| {
                p.system_id().is_some() && !p.is_placed_during_play() && p.homeworld_of().is_none() && **id != "mr" && !p.is_legendary() && p.planet_types().iter().map(|t| t.to_ascii_lowercase()).collect::<Vec<_>>() == wanted
            })
            .map(|(id, p)| (SystemId::new(p.system_id().unwrap()), ti4_model::id::PlanetId::new(*id)))
            .collect();
        found.sort();
        found.into_iter().next().unwrap_or_else(|| panic!("a free planet of {wanted:?}"))
    }

    /// A chooser that varies the answers with the seed and the question, so the options of the exploration cards are all taken somewhere.
    fn varied(seed: u64) -> Box<dyn FnMut(&Choice) -> usize> {
        let mut n = seed;
        Box::new(move |c| {
            n = n.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            // The fight itself and the commitment are answered with the first option (they decide nothing the exploration reads).
            if c.prompt.starts_with("Fight a round") || c.options.iter().any(|o| o.id.starts_with("commit")) {
                return 0;
            }
            usize::try_from(n >> 33).unwrap_or(0) % c.options.len().max(1)
        })
    }

    /// Put the exploration deck of `trait` in the order of `seed` (the top card varies) and a fleet with ground forces beside the free planet.
    fn first_claim(invader: &str, wanted: &[&str], seed: u64) -> (GameState, SystemId) {
        let (mut state, _, _) = game(&[("a", invader), ("b", "jolnar")], seed);
        let (system, _planet) = free_planet(wanted);
        let a = PlayerId::new("a");
        crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
        crate::fixtures::put(&mut state, &system, "infantry", &a, 2);
        for deck in state.exploration_decks.values_mut() {
            let n = deck.len().max(1);
            deck.rotate_left(usize::try_from(seed).unwrap_or(0) % n);
        }
        state.player_mut(&a).unwrap().commodities = i32::try_from(seed % 3).unwrap_or(0);
        state.player_mut(&a).unwrap().trade_goods = i32::try_from(seed % 4).unwrap_or(0);
        (state, system)
    }

    /// The first claim of a planet with a trait to explore into: the exploration is native (the deck, the card, its questions and rewards, the log
    /// row, the holdings), the questions and the position agree with the string window's, for the three traits and several invader factions. The
    /// acceptance switch is on: any bail is a failure of the run.
    #[test]
    fn a_first_claim_explores_natively_in_lockstep_with_the_string_window() {
        use crate::dual::{report, take_report, with_bail_is_failure};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let factions = ["hacan", "sol", "jolnar", "naalu", "letnev", "sardakk", "mentak", "mahact", "yssaril", "arborec", "ghost", "nekro"];
        let traits: [&[&str]; 3] = [&["cultural"], &["hazardous"], &["industrial"]];
        let mut asked = 0;
        for seed in 1..=36_u64 {
            let invader = factions[usize::try_from(seed).unwrap() % factions.len()];
            let wanted = traits[usize::try_from(seed).unwrap() % 3];
            let (state, system) = first_claim(invader, wanted, seed);
            asked += with_bail_is_failure(true, || check(&format!("first claim {invader} {wanted:?} {seed}"), &state, seed, &system, false, varied(seed)));
        }
        assert!(asked > 36);
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// A first claim by the Titans explores natively too: Terragenesis ("after you explore a planet: place or move a sleeper token onto it") is a rule of the
    /// core table, so the window of the ability is offered inside the exploration and its question is the string's. Bail-as-failure is on.
    #[test]
    fn a_titans_first_claim_explores_natively_with_terragenesis_in_lockstep_with_the_string_window() {
        use crate::dual::{report, take_report, with_bail_is_failure};
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let mut asked = 0;
        for seed in 1..=12_u64 {
            let (state, system) = first_claim("titans", &["cultural"], seed);
            // (The dual driver of the exploration has no ability registry, so it declines a Titans planet exploration with a counted bail; that is the
            // dual's, not the live path's: the string runs alone here.)
            let run = || check(&format!("titans first claim {seed}"), &state, seed, &system, false, prefer(&["ability:titans:terragenesis", "place", "move"]));
            asked += with_bail_is_failure(true, || crate::dual::with_mask(crate::dual::AreaMask::STRING, run));
        }
        assert!(asked > 12);
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let _ = take_report();
    }

    /// The 14 alpha saves carry no rules version (version 0): every legacy branch of the invasion (the round is not numbered, the rival structures are
    /// destroyed BEFORE the capture is announced, no copied round agent) runs natively with the same questions and the same position.
    #[test]
    fn a_version_zero_game_runs_natively_like_the_string_window() {
        let _guard = crate::dual::tests::serial();
        for (seed, version) in [(1_u64, 0_u32), (2, 0), (3, 1), (4, 15), (5, 0), (6, 16)] {
            let (mut state, system, planet) = game(&[("a", "l1z1x"), ("b", if seed % 2 == 0 { "letnev" } else { "sol" })], seed);
            state.rules_version = version;
            let (a, b) = (PlayerId::new("a"), PlayerId::new("b"));
            crate::fixtures::put(&mut state, &system, "carrier", &a, 1);
            crate::fixtures::put(&mut state, &system, "infantry", &a, 7);
            state.system_mut(&system).set_control(planet.clone(), b.clone());
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "infantry", &b, 2);
            crate::fixtures::put_on_planet(&mut state, &system, &planet, "pds", &b, 1);
            check(&format!("version {version}"), &state, seed, &system, false, prefer(&[]));
        }
    }
}
