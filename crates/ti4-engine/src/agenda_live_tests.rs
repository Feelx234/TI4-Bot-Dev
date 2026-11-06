//! Lane D (package KV): the native agenda phase (`ti4_engine_core::driver::agenda`) run through `live::live_step` against the string steps
//! it replaces (`step_agenda`, `open_next_vote`, `step_vote`, `close_vote` and the close of the agenda-timed scoring window), question by
//! question: the questions as rendered, the events, the position and the dice streams, from the opening of the agenda phase to
//! `AGENDA_PHASE_RESOLVED`.
//!
//! The string side plays `Game::step` until the phase's last event, answering the scripted ids and then the first option; the native side
//! takes the answers the string was given and runs one `live_step` per step the position's arm names.

use std::rc::Rc;

use ti4_content::ContentStore;
use ti4_engine_core::driver::session::{step_arm, Arm, Facts};
use ti4_engine_core::state::SliceSet;
use ti4_model::content_types::DEFAULT;
use ti4_model::state::{GameState, Phase};

use super::turn_live_tests::{assert_same, based, pid, Run};
use crate::core_sync::codec::rng::TextSeeds;
use crate::core_sync::{materialize, Codec, Ctx, Sources};
use crate::live::{self, Scripted};
use crate::render::Render;

/// How the table answers once the script has run out: the id of the first option whose id is in `prefer` (in that order of preference),
/// else the first option.
type Policy = &'static [&'static str];

/// A table that answers the scripted ids, then by `prefer`; it records the questions and the answers it took.
struct Chooser {
    ids: std::collections::VecDeque<String>,
    prefer: Policy,
    seen: Rc<std::cell::RefCell<Vec<crate::choice::Choice>>>,
    taken: Rc<std::cell::RefCell<Vec<String>>>,
}

impl crate::choice::Decider for Chooser {
    fn choose(&mut self, choice: &crate::choice::Choice) -> Result<crate::choice::ChoiceOption, crate::choice::IllegalChoice> {
        self.seen.borrow_mut().push(choice.clone());
        let id = self.ids.pop_front().or_else(|| {
            let pick = self.prefer.iter().find_map(|p| choice.options.iter().find(|o| o.id == *p)).or_else(|| choice.options.first())?;
            self.taken.borrow_mut().push(pick.id.clone());
            Some(pick.id.clone())
        });
        let Some(id) = id else { return Err(crate::choice::IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() }) };
        choice.options.iter().find(|o| o.id == id).cloned().ok_or_else(|| panic!("the script answers {id:?}, the question {:?} offers {:?}", choice.prompt, choice.ids()))
    }
}

fn string_agenda_preferring(state: &GameState, ids: &[&str], prefer: Policy) -> (Run, Vec<String>) {
    let _serial = crate::dual::tests::serial();
    let seen: Rc<std::cell::RefCell<Vec<crate::choice::Choice>>> = Rc::default();
    let taken: Rc<std::cell::RefCell<Vec<String>>> = Rc::default();
    let table = crate::choice::Table::with_default(Box::new(Chooser { ids: ids.iter().map(|s| (*s).to_owned()).collect(), prefer, seen: seen.clone(), taken: taken.clone() }));
    let mut g = super::Game::with_table(state.clone(), ContentStore::embedded(), table);
    {
        let g = &mut g;
        for _ in 0..400 {
            let result = g.step();
            assert_eq!(result.error, None, "the string step refused");
            if g.events.iter().any(|e| e == "AGENDA_PHASE_RESOLVED") || g.state.finished {
                break;
            }
        }
    }
    let questions = seen.borrow().clone();
    let (state, staged) = super::turn_live_tests::without_staged(serde_json::to_value(&g.state).expect("serializes"));
    let mut all: Vec<String> = ids.iter().map(|s| (*s).to_owned()).collect();
    all.extend(taken.borrow().iter().cloned());
    (Run { questions, events: g.events.clone(), state, rng: g.rng_positions(), staged }, all)
}

/// The native agenda phase on a copy of `state`: the reconcile hooks and one `live_step` of the arm the position names, while the arm is
/// one of the agenda phase's.
fn native_agenda(state: &GameState, ids: &[String]) -> Run {
    let mut cx = Ctx::of(state).expect("a table the codecs carry");
    let render = Render::new(cx.owners.ids().to_vec());
    let game = super::Game::with_table(state.clone(), ContentStore::embedded(), crate::choice::Table::with_default(Box::new(crate::choice::FirstOption)));
    let positions = game.rng_positions();
    let slices = SliceSet(SliceSet::ALL.0 & !(1 << ti4_engine_core::state::Slice::Map as u32));
    let mut core = Codec::load(&mut cx, state, Sources { galaxy: None, rng: Some(&positions) }, slices).expect("the position loads");
    let sources = DEFAULT;
    let kernel = live::kernel::kernel_for(sources);
    let env = live::env::env_with_plays(&render, &core, kernel, Rc::new(TextSeeds::of(&cx)), sources);
    let lcx = live::Cx::new(&render, &env);
    let mut asker = Scripted::new(ids.iter().cloned());
    for _ in 0..400 {
        loop {
            let step = live::live_reconcile(&mut core, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native reconcile stopped: {e:?}"));
            if step.finished {
                break;
            }
        }
        if core.meta().finished {
            break;
        }
        let arm = step_arm(&Facts::of_core(&core, false), false);
        if !matches!(arm, Arm::Agenda | Arm::Voting | Arm::EventScoring) {
            break;
        }
        live::live_step(&mut core, arm, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native {arm:?} stopped: {e:?}"));
    }
    let questions = asker.seen.clone();
    assert_eq!(asker.left(), 0, "every answer the string was given was taken");
    let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
    let after = materialize::materialize(&core, &cx, state).expect("materializes");
    let rng = materialize::rng_positions(&core).expect("the streams have names");
    let staged = core.pending().staged.iter().map(|e| crate::kernel_sync::text::event_text(&render, e.kind)).collect();
    let (state, _) = super::turn_live_tests::without_staged(serde_json::to_value(&after).expect("serializes"));
    Run { questions, events, state, rng, staged }
}

/// Both sides on one position.
fn both(state: &GameState, ids: &[&str]) -> (Run, Run) {
    both_preferring(state, ids, &[])
}

fn both_preferring(state: &GameState, ids: &[&str], prefer: Policy) -> (Run, Run) {
    let (string, taken) = string_agenda_preferring(state, ids, prefer);
    let native = native_agenda(state, &taken);
    (string, native)
}

/// A table in the agenda phase: the custodians gone, seat `a` the speaker, `deck` on top of the agenda deck and two planets with influence for
/// each seat to vote with.
fn agenda_table(seats: &[(&str, &str)], deck: &[&str]) -> GameState {
    let mut state = crate::fixtures::seated_dealt(seats, DEFAULT);
    state.phase = Phase::Agenda;
    state.active = None;
    state.custodians_removed = true;
    state.speaker = pid(seats[0].0);
    state.agenda_deck = deck.iter().map(|a| (*a).to_owned()).collect();
    let catalogue = ti4_content::galaxy::all_planets(ContentStore::embedded(), ti4_model::content_types::POK);
    let mut free = catalogue.iter().filter(|(_, p)| p.influence() > 0 && p.homeworld_of().is_none() && !p.is_placed_during_play());
    for (name, _) in seats {
        for _ in 0..2 {
            if let Some((id, record)) = free.next() {
                state.system_mut(&ti4_model::id::SystemId::new(record.system_id().unwrap_or("18"))).set_control(ti4_model::id::PlanetId::new(*id), pid(name));
            }
        }
    }
    based(state)
}

const FOUR: [(&str, &str); 4] = [("a", "sol"), ("b", "hacan"), ("c", "arborec"), ("d", "letnev")];

fn has(run: &Run, event: &str) -> bool {
    run.events.iter().any(|e| e == event)
}

#[test]
fn a_for_against_law_is_revealed_voted_and_closed_like_the_string() {
    let (string, native) = both(&agenda_table(&FOUR, &["checks", "conscription"]), &[]);
    assert_same(&string, &native);
    for e in ["AGENDA_PHASE_RESOLVED", "AGENDA_REVEALED:checks"] {
        assert!(has(&string, e), "{e} in {:?}", string.events);
    }
}

#[test]
fn an_empty_agenda_deck_finishes_the_phase_like_the_string() {
    let (string, native) = both(&agenda_table(&FOUR, &[]), &[]);
    assert_same(&string, &native);
    assert!(has(&string, "AGENDA_PHASE_RESOLVED"));
}


/// Every agenda of the corpus, on a table that gives each one something to act on: laws in play, a scored secret, a hand of cards.
fn rich_table(alias: &str) -> GameState {
    let mut state = agenda_table(&FOUR, &[alias, "conscription"]);
    for law in ["minister_war", "minister_peace"] {
        state.laws.insert(law.to_owned(), "b".to_owned());
    }
    state.laws.insert("regulations".to_owned(), "for".to_owned());
    state.scored_objectives.entry(pid("c")).or_default().insert(ti4_model::id::ObjectiveId::new("dtd"));
    based(state)
}

/// The agendas whose effect is a counted gap of the live path (`docs/KV_RESUME.md`): none since lane E2 (Galactic Crisis Pact performs the secondary
/// of the elected card in `driver::agenda::crisis`).
const GAPS: [&str; 0] = [];

fn sweep(prefer: Policy) -> Vec<String> {
    let mut failures = Vec::new();
    for agenda in ti4_engine_core::generated::Agenda::ALL {
        let alias = crate::generated::IdStr::as_str(agenda);
        if GAPS.contains(&alias) {
            continue;
        }
        // Ixthian Artifact, For (the first option) is the printed card in the core since rules version 36 and the string's effect is the known bug S6
        // (docs/STRING_ENGINE_BUGS.md): the two are not the same game by design; `ti4-engine-core-tests/tests/agenda_crater.rs` pins the core's. Against is compared.
        if alias == "artifact" && prefer.is_empty() {
            continue;
        }
        // Rearmament Agreement, For: the core places each seat's own mech from its reinforcements, the string the plain `mech` (known bug S9);
        // `ti4-engine-core-tests/tests/agenda_table_effects2.rs` pins the core's, `rearmament_for_is_live_the_seats_own_mech` below the live one. Against is compared.
        if alias == "rearmament" && prefer.is_empty() {
            continue;
        }
        let state = rich_table(alias);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (string, native) = both_preferring(&state, &[], prefer);
            assert_same(&string, &native);
        }));
        if let Err(e) = outcome {
            let text = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default();
            failures.push(format!("{alias}: {}", text.lines().next().unwrap_or("")));
        }
    }
    failures
}

/// Known string bug S9: Rearmament Agreement (For) on the live path places each seat's own mech (Sol, Arborec, Letnev and Hacan seated: each a
/// mech of its faction where the faction has one), where the string places the plain `mech`. Same questions and events; the board differs only in
/// the form of the new mechs.
#[test]
fn rearmament_for_is_live_the_seats_own_mech() {
    fn kinds(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(m) => {
                if let Some(serde_json::Value::String(t)) = m.get("type_id") {
                    out.push(t.clone());
                }
                m.values().for_each(|x| kinds(x, out));
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| kinds(x, out)),
            _ => {}
        }
    }
    let (string, native) = both(&rich_table("rearmament"), &[]);
    assert_eq!(string.events, native.events, "the events");
    let count = |run: &Run, f: &dyn Fn(&str) -> bool| {
        let mut all = Vec::new();
        kinds(&run.state, &mut all);
        all.iter().filter(|t| f(t)).count()
    };
    let (plain, any_mech) = (|t: &str| t == "mech", |t: &str| t.ends_with("mech"));
    assert!(count(&string, &plain) > 0, "the string placed plain mechs");
    assert_eq!(count(&native, &plain), 0, "the core placed no plain mech");
    assert_eq!(count(&string, &any_mech), count(&native, &any_mech), "as many mechs on both sides");
}

#[test]
fn every_agenda_is_voted_and_resolved_like_the_string_when_the_table_backs_the_first_option() {
    let failures = sweep(&[]);
    assert!(failures.is_empty(), "{} agendas differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn every_agenda_is_voted_and_resolved_like_the_string_when_the_table_votes_against() {
    let failures = sweep(&["against"]);
    assert!(failures.is_empty(), "{} agendas differ:\n{}", failures.len(), failures.join("\n"));
}

// -- the voting rules of the factions ------------------------------------------------------------------------------------

/// Print the questions and events of the string's run (an exploration aid for writing scenarios).
#[allow(dead_code)]
fn show(state: &GameState, ids: &[&str], prefer: Policy) {
    let (string, _) = string_agenda_preferring(state, ids, prefer);
    for q in &string.questions {
        eprintln!("{} | {} | {:?}", q.player, q.prompt, q.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>());
    }
    eprintln!("{:?}", string.events);
}

fn voters(run: &Run) -> Vec<String> {
    run.questions.iter().filter(|q| q.prompt.starts_with("vote for which outcome")).map(|q| q.player.to_string()).collect()
}

#[test]
fn nekro_cannot_vote_on_agendas_and_the_others_vote_in_order_like_the_string() {
    let state = agenda_table(&[("a", "sol"), ("b", "nekro"), ("c", "hacan"), ("d", "arborec")], &["checks"]);
    let (string, native) = both(&state, &[]);
    assert_same(&string, &native);
    assert_eq!(voters(&string), ["c", "d", "a"], "{:?}", voters(&string));
}

#[test]
fn argents_zeal_votes_first_like_the_string() {
    let state = agenda_table(&[("a", "sol"), ("b", "hacan"), ("c", "argent"), ("d", "arborec")], &["checks"]);
    let (string, native) = both(&state, &[]);
    assert_same(&string, &native);
    assert_eq!(voters(&string).first().map(String::as_str), Some("c"), "{:?}", voters(&string));
}

#[test]
fn a_silent_table_goes_to_the_speaker_and_an_abstention_is_not_a_vote_like_the_string() {
    let (string, native) = both_preferring(&agenda_table(&FOUR, &["checks"]), &[], &["decline"]);
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.prompt.starts_with("speaker breaks the tie")), "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
}

#[test]
fn a_tie_between_two_outcomes_is_broken_by_the_speaker_like_the_string() {
    // a and b each back a different outcome with one planet.
    let state = agenda_table(&[("a", "sol"), ("b", "hacan")], &["checks"]);
    let (string, native) = both(&state, &["decline", "decline"]);
    assert_same(&string, &native);
}

#[test]
fn hacans_commander_lets_its_owner_spend_trade_goods_as_votes_like_the_string() {
    let mut state = agenda_table(&[("a", "sol"), ("b", "hacan"), ("c", "arborec")], &["checks"]);
    state.player_mut(&pid("b")).unwrap().leaders.insert(ti4_model::id::LeaderId::new("hacancommander"), ti4_model::state::LeaderStatus::Unlocked);
    state.player_mut(&pid("b")).unwrap().trade_goods = 4;
    let state = based(state);
    let (string, native) = both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.prompt.contains("trade goods")), "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
}

#[test]
fn predictive_intelligence_offers_three_more_votes_like_the_string() {
    let mut state = agenda_table(&FOUR, &["checks"]);
    state.player_mut(&pid("c")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("pi"));
    let state = based(state);
    let (string, native) = both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.options.iter().any(|o| o.id.contains("predictive"))), "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
}

#[test]
fn genetic_recombination_asks_its_holder_before_each_vote_like_the_string() {
    let mut state = agenda_table(&[("a", "sol"), ("b", "mahact"), ("c", "arborec")], &["checks"]);
    state.player_mut(&pid("b")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("gr"));
    let state = based(state);
    let (string, native) = both(&state, &[]);
    assert_same(&string, &native);
}

#[test]
fn a_blood_pact_adds_four_votes_when_the_partner_backed_the_same_outcome_like_the_string() {
    let mut state = agenda_table(&[("a", "sol"), ("b", "empyrean"), ("c", "arborec")], &["checks"]);
    let note = state.promissory_notes.iter().find(|(n, _)| crate::promissory::alias_of(n) == "blood_pact").map(|(n, _)| n.clone());
    if let Some(note) = note {
        state.promissory_notes.insert(note.clone(), pid("a"));
        state.promissory_faceup.insert(note);
    }
    let state = based(state);
    let (string, native) = both(&state, &[]);
    assert_same(&string, &native);
}

#[test]
fn research_grant_reallocation_gains_the_chosen_technology_and_pays_its_prerequisites_like_the_string() {
    let state = rich_table("grant_reallocation");
    let (string, native) = both(&state, &[]);
    let last = |r: &Run| r.questions.iter().find(|q| q.prompt.starts_with("gain which")).map(|q| (q.prompt.clone(), q.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>(), q.context.clone().map(|c| format!("{:?}", c.source)), q.details.clone()));
    assert_eq!(last(&string), last(&native));
    assert_same(&string, &native);
}

fn with_hand(mut state: GameState, who: &str, cards: &[&str]) -> GameState {
    state.player_mut(&pid(who)).unwrap().action_cards = cards.iter().map(|c| ti4_model::id::ActionCardId::new(*c)).collect();
    based(state)
}

/// Every rider card, played by seat `b` into the reveal of the first agenda (the table's first option predicts the outcome that wins).
fn rider_sweep(prefer: Policy, table: impl Fn(&str) -> GameState) -> Vec<String> {
    let mut failures = Vec::new();
    for rider in ["imp_rider", "trade_rider", "politic_rider", "lead_rider", "const_rider", "diplo_rider", "war_rider", "tech_rider", "sanction", "assassin"] {
        let state = table(rider);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (string, native) = both_preferring(&state, &[], prefer);
            assert!(string.events.iter().any(|e| e == "ACTION_CARD_PLAYED"), "the rider was never played: {:?}", string.events);
            assert_same(&string, &native);
        }));
        if let Err(e) = outcome {
            let text = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_default();
            failures.push(format!("{rider}: {}", text.lines().next().unwrap_or("")));
        }
    }
    failures
}

/// Bribery ("after the speaker votes ... spend trade goods, one extra vote each"): held by a seat with trade goods, offered and played in the
/// vote window of the native agenda phase as the string does it. The judge of the goods is the core's (`potential_goods`).
#[test]
fn bribery_is_played_in_the_vote_like_the_string() {
    for seat in ["b", "c"] {
        let mut state = with_hand(agenda_table(&FOUR, &["checks", "conscription"]), seat, &["bribery"]);
        state.player_mut(&pid(seat)).unwrap().trade_goods = 3;
        let state = based(state);
        let reaction = format!("reaction:{}:VOTES_CAST:after", if seat == "b" { "hacan" } else { "arborec" });
        let policy: Policy = Box::leak(vec![Box::leak(reaction.into_boxed_str()) as &'static str, "1"].into_boxed_slice());
        let (string, native) = both_preferring(&state, &[], policy);
        assert!(string.events.iter().any(|e| e == "ACTION_CARD_PLAYED"), "{seat}: Bribery was never played: {:?}", string.events);
        assert_same(&string, &native);
    }
}

#[test]
fn a_correct_rider_is_paid_like_the_string() {
    let failures = rider_sweep(&[], |rider| with_hand(agenda_table(&FOUR, &["checks", "conscription"]), "b", &[rider]));
    assert!(failures.is_empty(), "{} riders differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn a_rider_of_a_yssaril_seat_is_paid_like_the_string() {
    let failures = rider_sweep(&[], |rider| with_hand(agenda_table(&[("a", "sol"), ("b", "yssaril"), ("c", "arborec"), ("d", "letnev")], &["checks", "conscription"]), "b", &[rider]));
    assert!(failures.is_empty(), "{} riders differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
#[ignore = "exploration aid"]
fn explore_tech_rider() {
    let state = with_hand(agenda_table(&FOUR, &["checks", "conscription"]), "b", &["tech_rider"]);
    let (string, native) = both(&state, &[]);
    for (name, run) in [("string", &string), ("native", &native)] {
        for q in run.questions.iter().filter(|q| q.player.as_str() == "b" && !q.prompt.starts_with("exhaust") && !q.prompt.starts_with("vote")) {
            eprintln!("{name}: {} | {} | {:?} | {:?}", q.player, q.prompt, q.options.iter().take(4).collect::<Vec<_>>(), q.details);
        }
    }
}

#[test]
#[ignore = "exploration aid"]
fn explore_politics_rider_yssaril() {
    let state = with_hand(agenda_table(&[("a", "sol"), ("b", "yssaril"), ("c", "arborec"), ("d", "letnev")], &["checks", "conscription"]), "b", &["politic_rider"]);
    let (string, native) = both(&state, &[]);
    let cut = |r: &Run| r.events.iter().take(24).cloned().collect::<Vec<_>>();
    eprintln!("string {:?}\nnative {:?}", cut(&string), cut(&native));
    eprintln!("string q {:?}", string.questions.iter().filter(|q| q.player.as_str() == "b" && !q.prompt.starts_with("exhaust") && !q.prompt.starts_with("vote")).map(|q| q.prompt.clone()).collect::<Vec<_>>());
    eprintln!("native q {:?}", native.questions.iter().filter(|q| q.player.as_str() == "b" && !q.prompt.starts_with("exhaust") && !q.prompt.starts_with("vote")).map(|q| q.prompt.clone()).collect::<Vec<_>>());
}

#[test]
fn a_valefar_assimilator_carrying_genetic_recombination_asks_the_nekro_before_each_vote_like_the_string() {
    let mut state = agenda_table(&[("a", "nekro"), ("b", "mahact"), ("c", "sol")], &["checks"]);
    // The Mahact's own card is spent: only the Nekro's Valefar Assimilator carrying its text can be used.
    let (a, b) = (pid("a"), pid("b"));
    state.player_mut(&b).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("gr"));
    state.player_mut(&b).unwrap().exhausted_technologies.insert(ti4_model::id::TechnologyId::new("gr"));
    state.player_mut(&a).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("vax"));
    state.player_mut(&a).unwrap().assimilated_technologies.insert("vax".into(), ti4_model::id::TechnologyId::new("gr"));
    let state = based(state);
    let (string, native) = both(&state, &[]);
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.prompt.starts_with("Genetic Recombination")), "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
}

#[test]
fn galactic_crisis_pact_asks_each_player_the_secondary_without_a_token_like_the_string() {
    // A silent table (nobody spends a planet to vote: the speaker breaks the tie and elects the first card), everyone declines the pact ("no" is the first option).
    let (string, native) = both_preferring(&rich_table("crisis"), &[], &["decline"]);
    assert_same(&string, &native);
    let pact: Vec<_> = string.questions.iter().filter(|q| q.prompt.contains("Galactic Crisis Pact") || q.details.contains_key("agenda") && q.details.get("kind").is_some_and(|k| k == "strategy_secondary")).collect();
    assert!(!pact.is_empty(), "the pact asks: {:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
    assert!(pact.iter().all(|q| q.details.get("costs_token") == Some(&serde_json::Value::Bool(false)) && !q.details.contains_key("tokens_left") && !q.details.contains_key("played_by")), "no token, no primary player");
}

#[test]
fn galactic_crisis_pact_performs_the_secondary_of_the_elected_card_for_those_who_accept_like_the_string() {
    // The same silent table; everyone who is asked takes the secondary.
    let (string, native) = both_preferring(&rich_table("crisis"), &[], &["decline", "yes"]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e == "AGENDA_EFFECT_RESOLVED:crisis" || e.starts_with("AGENDA_EFFECT_RESOLVED")), "{:?}", string.events);
    let asked = string.questions.iter().filter(|q| q.details.contains_key("agenda") && q.details.get("kind").is_some_and(|k| k == "strategy_secondary")).count();
    assert!(asked >= 1, "the pact asked at least one player");
}

#[test]
fn covert_legislation_decides_the_face_down_agenda_that_resolves_like_the_string() {
    let mut state = agenda_table(&FOUR, &["covert", "checks", "conscription"]);
    state.rules_version = ti4_model::state::RULES_VERSION;
    let (string, native) = both(&based(state), &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().any(|e| e.starts_with("COVERT_AGENDA_RESOLVED:")), "{:?}", string.events);
}

#[test]
fn committee_formation_discarded_elects_the_player_its_owner_names_and_skips_the_vote_like_the_string() {
    let mut state = agenda_table(&FOUR, &["secret", "conscription"]);
    state.laws.insert("committee".to_owned(), "b".to_owned());
    let state = based(state);
    let (string, native) = both(&state, &["discard", "c"]);
    assert_same(&string, &native);
    assert!(has(&string, "AGENDA_RESOLVED:secret:c"), "{:?}", string.events);
    // Declined, the agenda is voted on as usual.
    let (string, native) = both(&state, &["decline"]);
    assert_same(&string, &native);
    assert!(string.questions.iter().any(|q| q.prompt == "vote for which outcome"));
}

#[test]
fn a_rider_of_the_first_seat_is_paid_like_the_string() {
    let failures = rider_sweep(&[], |rider| with_hand(agenda_table(&FOUR, &["checks", "conscription"]), "a", &[rider]));
    assert!(failures.is_empty(), "{} riders differ:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn research_grant_reallocation_below_rules_version_14_takes_one_fleet_token_like_the_string() {
    let mut state = rich_table("grant_reallocation");
    state.rules_version = ti4_model::state::RULES_AGENDA_GRANT_REVEAL - 1;
    state.player_mut(&pid("a")).unwrap().fleet_tokens = 3;
    let (string, native) = both(&based(state), &[]);
    assert_same(&string, &native);
    assert!(!string.questions.iter().any(|q| q.prompt.starts_with("gain which")));
}

#[test]
fn demilitarized_zone_wipes_the_planet_and_the_destroyed_ground_forces_are_announced_in_the_next_step_like_the_string() {
    let mut state = rich_table("demilitarized_zone");
    // A sustained mech and an infantry of seat b on a planet seat b holds.
    let (system, planet) = crate::fixtures::a_placed_planet();
    state.system_mut(&system).set_control(planet.clone(), pid("b"));
    crate::fixtures::put(&mut state, &system, "infantry", &pid("b"), 0);
    state.system_mut(&system).planet_units.entry(planet.clone()).or_default().push(ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("infantry"), pid("b")));
    let mut mech = ti4_model::units::Unit::new(ti4_model::id::UnitTypeId::new("mech"), pid("b"));
    mech.sustained_damage = true;
    state.system_mut(&system).planet_units.entry(planet).or_default().push(mech);
    let (string, native) = both(&based(state), &[]);
    assert_same(&string, &native);
    assert!(string.events.iter().filter(|e| *e == "GROUND_FORCE_DESTROYED").count() >= 2, "{:?}", string.events);
}

#[test]
#[ignore = "exploration aid"]
fn explore_committee() {
    let mut state = agenda_table(&FOUR, &["secret", "conscription"]);
    state.laws.insert("committee".to_owned(), "b".to_owned());
    let state = based(state);
    let (string, native) = both(&state, &["discard", "c"]);
    for (name, run) in [("string", &string), ("native", &native)] {
        for q in run.questions.iter().take(3) {
            eprintln!("{name}: {} | {} | {:?} | {:?} | {:?}", q.player, q.prompt, q.options.iter().map(|o| (&o.id, &o.kind, &o.label)).collect::<Vec<_>>(), q.details, q.context.as_ref().map(|c| (c.actor.clone(), format!("{:?}", c.source), c.subtype.clone())));
        }
    }
}
