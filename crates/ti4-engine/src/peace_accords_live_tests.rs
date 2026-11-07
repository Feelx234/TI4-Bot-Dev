//! Lane E2: Xxcha's Peace Accords (`rules::peace_accords`, `driver::windows::strategic::perform::resolved`) run natively against the string's
//! `faction_abilities::strategy_resolved`: the question as rendered (every field of the choice, the options' payloads and previews), the
//! events, the position and the dice streams, from the menu's answer (the primary) or the follower's (the secondary) to the end of the
//! action, with the planet's exploration inside it.
//!
//! The string side plays `Game::step` on a map (the hub of seven plain systems), answering by a rule (`want`) and the first option
//! otherwise; the native side takes the answers the string was given.

use std::cell::RefCell;
use std::rc::Rc;

use ti4_content::ContentStore;
use ti4_engine_core::driver::session::{step_arm, Arm, Facts};
use ti4_engine_core::state::{SliceSet, WindowKind};
use ti4_model::content_types::DEFAULT;
use ti4_model::id::{PlanetId, PlayerId, StrategyCardId, SystemId};
use ti4_model::state::{GameState, Phase};

use super::turn_live_tests::{based, pid, without_staged, Run, Seen};
use crate::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Table};
use crate::core_sync::{materialize, Codec, Ctx, Sources};
use crate::live::{self, Scripted};
use crate::render::Render;

/// The rule a table answers by: the id to take at a question, or `None` for the first option.
type Want = Box<dyn Fn(&Choice) -> Option<String>>;

/// Answers by `want` (the id to take, or the first option) and records the questions and the ids taken.
struct Picker {
    want: Want,
    seen: Seen,
    taken: Rc<RefCell<Vec<String>>>,
}

impl Decider for Picker {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.seen.borrow_mut().push(choice.clone());
        let id = (self.want)(choice).or_else(|| choice.options.first().map(|o| o.id.clone()));
        let Some(id) = id else { return Err(IllegalChoice::NoOptions { player: choice.player.clone(), prompt: choice.prompt.clone() }) };
        self.taken.borrow_mut().push(id.clone());
        choice.answer_for(&id).ok_or_else(|| panic!("the picker answers {id:?}, the question {:?} offers {:?}", choice.prompt, choice.ids()))
    }
}

fn named(cx: &Ctx, state: &GameState) -> Render {
    let r = Render::new(cx.owners.ids().to_vec());
    let factions = r.seats.iter().map(|id| state.players.iter().find(|p| p.id.as_str() == id).map_or_else(String::new, |p| p.faction.as_str().to_owned())).collect();
    r.with_factions(factions)
}

/// The two runs agree on every question (the whole choice), the events, the staged events, the position and the dice streams.
fn assert_same(string: &Run, native: &Run) {
    let whole = |c: &Choice| serde_json::to_value(c).expect("a choice serializes");
    for (i, (a, b)) in string.questions.iter().zip(&native.questions).enumerate() {
        let diffs = crate::dual::diff_json(&whole(a), &whole(b), 8);
        assert!(diffs.is_empty(), "question {i} ({}) differs: {diffs:?}", a.prompt);
    }
    assert_eq!(string.questions.iter().map(|q| q.prompt.clone()).collect::<Vec<_>>(), native.questions.iter().map(|q| q.prompt.clone()).collect::<Vec<_>>(), "the questions");
    assert_eq!(string.events, native.events, "the events");
    assert_eq!(string.staged, native.staged, "the events left staged for the next step");
    let diffs = crate::dual::diff_json(&string.state, &native.state, 12);
    assert!(diffs.is_empty(), "the position differs: {diffs:?}");
    assert_eq!(format!("{:?}", string.rng), format!("{:?}", native.rng), "the dice streams stand at the same place");
}

fn diplomacy() -> ti4_engine_core::generated::StrategyCard {
    <ti4_engine_core::generated::StrategyCard as crate::generated::IdStr>::from_id("pok2diplomacy").expect("a card of the tables")
}

fn card_id(card: &str) -> StrategyCardId {
    StrategyCardId::new(card)
}

/// The string's steps from the state, until `STRATEGIC_ACTION_COMPLETE`.
fn string_run(state: &GameState, galaxy: &ti4_content::galaxy::Galaxy, want: Want, open: impl FnOnce(&mut super::Game<'static>)) -> (Run, Vec<String>) {
    let _serial = crate::dual::tests::serial();
    let seen: Seen = Rc::default();
    let taken: Rc<RefCell<Vec<String>>> = Rc::default();
    let table = Table::with_default(Box::new(Picker { want, seen: seen.clone(), taken: taken.clone() }));
    let mut g = super::Game::with_table(state.clone(), ContentStore::embedded(), table);
    g.galaxy = Some(galaxy.clone());
    g.prepared_turn_seq = Some(g.state.turn_seq);
    open(&mut g);
    for _ in 0..60 {
        let _ = g.step();
        if g.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE") || g.state.finished {
            break;
        }
    }
    let questions = seen.borrow().clone();
    crate::multiask_dual::remember_string_questions(questions.clone());
    let (state, staged) = without_staged(serde_json::to_value(&g.state).expect("serializes"));
    let run = Run { questions, events: g.events.clone(), state, rng: g.rng_positions(), staged };
    let ids = taken.borrow().clone();
    (run, ids)
}

/// The native arms on a copy of `state` loaded WITH the map, after `setup` opened the window or the job, with the answers the string took.
fn native_run(state: &GameState, galaxy: &ti4_content::galaxy::Galaxy, ids: &[String], setup: &dyn Fn(&mut ti4_engine_core::state::CoreState, &Render)) -> Run {
    let mut cx = Ctx::of(state).expect("a table the codecs carry");
    let render = named(&cx, state);
    let game = super::Game::with_table(state.clone(), ContentStore::embedded(), Table::with_default(Box::new(crate::choice::FirstOption)));
    let positions = game.rng_positions();
    let mut core = Codec::load(&mut cx, state, Sources { galaxy: Some(galaxy), rng: Some(&positions) }, SliceSet::ALL).expect("the position loads");
    let kernel = live::kernel::kernel_for(DEFAULT);
    let env = live::env::env_with_plays(&render, &core, kernel, DEFAULT);
    let lcx = live::Cx::new(&render, &env);
    let mut asker = Scripted::new(ids.iter().cloned());
    loop {
        let step = live::live_reconcile(&mut core, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native reconcile stopped: {e:?}"));
        if step.finished {
            break;
        }
    }
    setup(&mut core, &render);
    for _ in 0..80 {
        let chain = step_arm(&Facts::of_core(&core, false), true);
        let arm = match chain {
            Arm::Secondary | Arm::LeaderFollowers => chain,
            _ if core.pending().window(WindowKind::TurnFlow).is_some() => Arm::Ask,
            _ => break,
        };
        live::live_step(&mut core, arm, &lcx, &mut asker).unwrap_or_else(|e| panic!("the native {arm:?} stopped: {e:?} after {:?}", asker.seen.iter().map(|q| q.prompt.clone()).collect::<Vec<_>>()));
    }
    assert!(core.pending().window(WindowKind::StrategySecondary).is_none(), "the follower window ended");
    assert_eq!(asker.left(), 0, "every answer the string was given was taken");
    let questions = asker.seen.clone();
    let events = crate::render::events::events_text(&render, &cx, &core).expect("every event has a text");
    let after = materialize::materialize(&core, &cx, state).expect("materializes");
    let rng = materialize::rng_positions(&core).expect("the streams have names");
    let staged = core.pending().staged.iter().map(|e| crate::kernel_sync::text::event_text(&render, e.kind)).collect();
    let (state, _) = without_staged(serde_json::to_value(&after).expect("serializes"));
    Run { questions, events, state, rng, staged }
}

/// A hub (a centre and a ring of six) of systems whose planets can all be explored (a trait to explore into, no home system, no station): `plain_hub`'s tiles are
/// the first systems of the corpus, which are home systems.
fn explorable_hub() -> crate::fixtures::Hub {
    let content = ContentStore::embedded();
    let planets = ti4_content::galaxy::all_planets(content, ti4_model::content_types::POK);
    let ids: Vec<String> = ti4_content::galaxy::all_systems(content, ti4_model::content_types::POK)
        .iter()
        .filter(|(_, system)| !system.is_anomaly() && !system.is_hyperlane())
        .filter(|(_, system)| {
            let ps = system.planets();
            !ps.is_empty() && ps.iter().all(|p| planets.get(p).is_some_and(|p| p.homeworld_of().is_none() && !p.is_placed_during_play() && !p.is_space_station() && !p.traits().is_empty()))
        })
        .map(|(id, _)| (*id).to_owned())
        .take(7)
        .collect();
    assert_eq!(ids.len(), 7, "the corpus has seven explorable systems");
    crate::fixtures::hub_from(&ids)
}

/// The planets a system prints.
fn planets_of(system: &str) -> Vec<String> {
    ContentStore::embedded().get(ti4_model::content_types::ContentType::Systems, system).map(|r| r.strings("planets").into_iter().map(str::to_owned).collect()).unwrap_or_default()
}

/// Whether the planet has a trait to explore into.
fn has_trait(planet: &str) -> bool {
    ti4_content::galaxy::planet(ContentStore::embedded(), planet, DEFAULT).is_some_and(|p| !p.traits().is_empty())
}

/// The table: `a` (the Xxcha) controls the first planet of the hub's centre, the other seats hold no strategy token (the followers decline
/// without a question). `diplomacy` is who holds the card.
fn table(seats: &[(&str, &str); 4], holder: &str, hub: &crate::fixtures::Hub, tweak: impl FnOnce(&mut GameState)) -> GameState {
    let mut state = crate::fixtures::seated_dealt(seats, DEFAULT);
    state.phase = Phase::Action;
    state.active = Some(pid(holder));
    state.player_mut(&pid(holder)).unwrap().strategy_cards.push(card_id("pok2diplomacy"));
    for p in seats.iter().map(|s| s.0).filter(|p| *p != holder) {
        state.player_mut(&pid(p)).unwrap().strategic_tokens = 0;
    }
    let held = planets_of(&hub.centre).remove(0);
    state.system_mut(&SystemId::new(&hub.centre)).set_control(PlanetId::new(&held), pid("a"));
    tweak(&mut state);
    based(state)
}

const SEATS: [(&str, &str); 4] = [("a", "xxcha"), ("b", "sol"), ("c", "letnev"), ("d", "jolnar")];

fn is_annex(c: &Choice) -> bool {
    c.prompt == "Peace Accords: annex a planet"
}

/// The planet id the picker takes at the Peace Accords question: the first with a trait if `explored`, else the first without one.
fn annex_pick(explored: bool) -> Want {
    Box::new(move |c| {
        if c.player.as_str() == "a" && c.context.as_ref().is_some_and(|x| x.subtype == "action_menu") {
            return Some("strategic".to_owned());
        }
        if is_annex(c) {
            return c.options.iter().find(|o| o.kind == "annex" && has_trait(&o.id) == explored).map(|o| o.id.clone());
        }
        None
    })
}

fn primary_case(want: Want, seats: &[(&str, &str); 4]) -> (Run, Run) {
    primary_case_with(want, seats, |_| {})
}

fn primary_case_with(want: Want, seats: &[(&str, &str); 4], tweak: impl FnOnce(&mut GameState)) -> (Run, Run) {
    let hub = explorable_hub();
    let state = table(seats, "a", &hub, tweak);
    let (mut string, taken) = string_run(&state, &hub.galaxy, want, |_| {});
    assert_eq!(string.questions.first().and_then(|q| q.context.as_ref()).map(|c| c.subtype.as_str()), Some("action_menu"));
    string.questions.remove(0);
    let native = native_run(&state, &hub.galaxy, &taken[1..], &|core, _| {
        ti4_engine_core::driver::turn::open_job(core, ti4_engine_core::state::TurnJob::Strategic(diplomacy()));
    });
    (string, native)
}

fn asked_annex(run: &Run) -> usize {
    run.questions.iter().filter(|q| is_annex(q)).count()
}

#[test]
fn the_xxcha_annexes_an_explored_planet_after_its_diplomacy_primary_like_the_string() {
    let (string, native) = primary_case(annex_pick(true), &SEATS);
    assert_eq!(asked_annex(&string), 1, "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
    assert_same(&string, &native);
    // The exploration really ran: the exploration decks of the position differ from the declined run's (a drawn card leaves its deck).
    let (declined, _) = primary_case(Box::new(|c| if is_annex(c) { Some("decline".to_owned()) } else if c.context.as_ref().is_some_and(|x| x.subtype == "action_menu") { Some("strategic".to_owned()) } else { None }), &SEATS);
    let explored = |run: &Run| -> Vec<(String, serde_json::Value)> { run.state.as_object().map(|o| o.iter().filter(|(k, _)| k.contains("explor")).map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default() };
    assert!(!explored(&string).is_empty(), "the position keeps exploration data");
    assert_ne!(explored(&string), explored(&declined), "the annexed planet was explored");
    assert!(string.questions.iter().any(|q| is_annex(q) && q.options.iter().any(|o| o.kind == "annex")), "the question lists planets");
    assert!(string.events.iter().any(|e| e == "STRATEGIC_ACTION_COMPLETE"), "{:?}", string.events);
}

#[test]
fn the_xxcha_with_integrated_economy_produces_on_the_annexed_planet_before_exploring_it_like_the_string() {
    let want: Want = Box::new(|c| {
        if c.player.as_str() == "a" && c.context.as_ref().is_some_and(|x| x.subtype == "action_menu") {
            return Some("strategic".to_owned());
        }
        if is_annex(c) {
            // A planet worth resources (the production budget) with a trait (the exploration that follows).
            return c.options.iter().find(|o| o.kind == "annex" && resources_of(&o.id) > 0 && has_trait(&o.id)).map(|o| o.id.clone());
        }
        None
    });
    let (string, native) = primary_case_with(want, &SEATS, |state| {
        state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("ie"));
    });
    assert_eq!(asked_annex(&string), 1);
    let economy = string.questions.iter().filter(|q| q.prompt.starts_with("Integrated Economy on ")).count();
    assert!(economy >= 1, "the string offers the production: {:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
    assert_same(&string, &native);
}

/// The resources a planet prints.
fn resources_of(planet: &str) -> i64 {
    ti4_content::galaxy::planet(ContentStore::embedded(), planet, DEFAULT).map_or(0, |p| p.resources())
}

/// The Xxcha with Integrated Economy annexes a planet worth exactly `worth` resources (the budget of the production) and explores it, like the string.
/// Returns whether the hub has such a planet with a trait.
fn integrated_economy_on_a_planet_worth(worth: i64) -> bool {
    let hub = explorable_hub();
    let offered: Vec<String> = std::iter::once(&hub.centre).chain(hub.outer.iter()).flat_map(|s| planets_of(s)).collect();
    if !offered.iter().any(|p| resources_of(p) == worth && has_trait(p)) {
        return false;
    }
    let want: Want = Box::new(move |c| {
        if c.player.as_str() == "a" && c.context.as_ref().is_some_and(|x| x.subtype == "action_menu") {
            return Some("strategic".to_owned());
        }
        if is_annex(c) {
            return c.options.iter().find(|o| o.kind == "annex" && resources_of(&o.id) == worth && has_trait(&o.id)).map(|o| o.id.clone());
        }
        None
    });
    let (string, native) = primary_case_with(want, &SEATS, |state| {
        state.player_mut(&pid("a")).unwrap().technologies.insert(ti4_model::id::TechnologyId::new("ie"));
    });
    assert_eq!(asked_annex(&string), 1);
    let economy = string.questions.iter().filter(|q| q.prompt.starts_with("Integrated Economy on ")).count();
    assert_eq!(economy > 0, worth > 0, "the production is offered exactly when the planet is worth resources: {:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
    assert_same(&string, &native);
    true
}

#[test]
fn integrated_economy_on_a_planet_worth_nothing_asks_nothing_and_on_one_worth_one_asks_like_the_string() {
    let (zero, one) = (integrated_economy_on_a_planet_worth(0), integrated_economy_on_a_planet_worth(1));
    assert!(zero || one, "the hub has a planet worth 0 or 1 resources with a trait to test the budget's edge");
}

#[test]
fn a_declined_annexation_changes_nothing_like_the_string() {
    let (string, native) = primary_case(Box::new(|c| if is_annex(c) { Some("decline".to_owned()) } else if c.context.as_ref().is_some_and(|x| x.subtype == "action_menu") { Some("strategic".to_owned()) } else { None }), &SEATS);
    assert_eq!(asked_annex(&string), 1);
    assert_same(&string, &native);
}

#[test]
fn a_follower_xxcha_annexes_after_the_diplomacy_secondary_like_the_string() {
    let hub = explorable_hub();
    let seats: [(&str, &str); 4] = [("a", "sol"), ("b", "xxcha"), ("c", "letnev"), ("d", "jolnar")];
    // `b`, the Xxcha, holds the hub centre's planet and one strategy token: it follows `a`'s Diplomacy.
    let mut state = crate::fixtures::seated_dealt(&seats, DEFAULT);
    state.phase = Phase::Action;
    state.active = Some(pid("a"));
    state.player_mut(&pid("a")).unwrap().strategy_cards.push(card_id("pok2diplomacy"));
    for p in ["c", "d"] {
        state.player_mut(&pid(p)).unwrap().strategic_tokens = 0;
    }
    state.player_mut(&pid("b")).unwrap().strategic_tokens = 2;
    // (Diplomacy's secondary readies two exhausted planets: `b` needs some to be asked.)
    let planets: Vec<_> = state.controlled_planets(&pid("b")).into_iter().map(|(_, planet)| planet.clone()).collect();
    for planet in planets {
        state.exhausted_planets.insert(planet);
    }
    let held = planets_of(&hub.centre).remove(0);
    state.system_mut(&SystemId::new(&hub.centre)).set_control(PlanetId::new(&held), pid("b"));
    let state = based(state);
    let want: Want = Box::new(|c| if c.player.as_str() == "b" && c.details.get("kind").and_then(|v| v.as_str()) == Some("strategy_secondary") { Some("yes".to_owned()) } else if is_annex(c) { c.options.iter().find(|o| o.kind == "annex" && has_trait(&o.id)).map(|o| o.id.clone()) } else { None });
    let (string, taken) = string_run(&state, &hub.galaxy, want, |g| {
        let answer = ChoiceOption::labelled(crate::strategy::STRATEGIC_ACTION_ID, crate::strategy::ACTION_KIND, "strategic");
        let window = crate::strategy::begin_strategic_action(&mut g.state, g.content, &pid("a"), answer).expect("the seat holds the card");
        g.secondary = Some(window);
    });
    assert_eq!(asked_annex(&string), 1, "{:?}", string.questions.iter().map(|q| &q.prompt).collect::<Vec<_>>());
    let native = native_run(&state, &hub.galaxy, &taken, &|core, render| {
        let primary = render.seat("a").expect("seat a");
        let followers: Vec<_> = state.clockwise_from(&pid("a")).iter().skip(1).map(|p| render.seat(p.as_str()).expect("a seated follower")).collect();
        core.pending_mut().open(ti4_engine_core::state::Window::StrategySecondary(ti4_engine_core::driver::windows::strategic::secondary_of(primary, diplomacy(), &followers, true)));
    });
    assert_same(&string, &native);
}

#[test]
fn a_seat_of_another_faction_is_asked_nothing_after_diplomacy_like_the_string() {
    let seats: [(&str, &str); 4] = [("a", "sol"), ("b", "hacan"), ("c", "letnev"), ("d", "jolnar")];
    let (string, native) = primary_case(annex_pick(true), &seats);
    assert_eq!(asked_annex(&string), 0);
    assert_same(&string, &native);
}

#[allow(dead_code)]
fn unused(_: PlayerId) {}

// -- the two secrets the core answers since lane E2 (Strengthen Bonds, Produce en Masse) --------------------------------------------------------------

mod secrets {
    use ti4_model::content_types::POK;
    use ti4_model::id::{PlayerId, SecretObjectiveId};
    use ti4_model::state::GameState;

    use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};
    use crate::objectives::RequirementProgress;
    use crate::secrets::Position;

    fn a() -> PlayerId {
        PlayerId::new("a")
    }

    /// The string's progress, the typed view's answer, the differences the Dual comparison reported.
    type Both = (Option<RequirementProgress>, Option<Option<RequirementProgress>>, u64);

    /// The string's progress of `alias` for `a`, the typed view's answer next to it (both through the Dual backend of the objectives area,
    /// which compares them), and the count of differences the comparison reported.
    fn both(state: &GameState, alias: &str) -> Both {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        let content = ti4_content::ContentStore::embedded();
        let player = a();
        let alias = SecretObjectiveId::new(alias);
        let view = crate::objectives_core::ObjView::build(state, content, POK, None).expect("the position loads");
        let position = Position { state, content, sources: POK, player: &player, galaxy: None, imagined: crate::objectives::Imagined::NONE };
        let (string, typed) = with_mask(AreaMask::STRING.with(Area::Objectives, Backend::Dual), || {
            crate::objectives_core::scoped(Some(&view), || (crate::secrets::remaining_position_progress(&alias, &position), crate::objectives_core::secret_typed(&position, &alias, false)))
        });
        let r = report();
        let differences = r.total_differences();
        if differences > 0 {
            eprintln!("E2 secret dual differences: {:?}", r.differences);
        }
        let _ = take_report();
        (string, typed, differences)
    }

    fn holding(state: &mut GameState, alias: &str) {
        state.player_mut(&a()).unwrap().secret_objectives = vec![SecretObjectiveId::new(alias)];
    }

    #[test]
    fn strengthen_bonds_is_answered_by_the_typed_view_like_the_string() {
        let content = ti4_content::ContentStore::embedded();
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "titans"), ("c", "hacan")], POK);
        crate::promissory::deal(&mut state, content, POK);
        holding(&mut state, "sb");
        let (string, typed, differences) = both(&state, "sb");
        assert_eq!(differences, 0);
        assert_eq!(typed, Some(string.clone()), "the typed view answers (0 bonds)");
        assert_eq!(string.as_ref().map(|p| p.have), Some(0));
        // The Titans' Terraform (a play-area card: faceup on receipt) is a bond; so is the Support for the Throne `c` lent.
        crate::promissory::take(&mut state, content, &a(), "terraform:titans");
        let (string, typed, differences) = both(&state, "sb");
        assert_eq!((differences, string.as_ref().map(|p| p.have)), (0, Some(1)));
        assert_eq!(typed, Some(string.clone()));
        state.support_holders.insert(PlayerId::new("c"), a());
        let (string, typed, differences) = both(&state, "sb");
        assert_eq!((differences, string.as_ref().map(|p| p.have)), (0, Some(2)));
        assert_eq!(typed, Some(string.clone()));
        // Your own Support in your own area is not a bond.
        state.support_holders.insert(a(), a());
        let (string, typed, differences) = both(&state, "sb");
        assert_eq!((differences, string.as_ref().map(|p| p.have)), (0, Some(2)));
        assert_eq!(typed, Some(string.clone()));
    }

    #[test]
    fn produce_en_masse_is_answered_by_the_typed_view_like_the_string() {
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "titans"), ("c", "hacan")], POK);
        // (The notes are dealt after the factions are seated: the cards codec refuses the placeholder faction.)
        crate::promissory::deal(&mut state, ti4_content::ContentStore::embedded(), POK);
        holding(&mut state, "pem");
        let (string, typed, differences) = both(&state, "pem");
        assert_eq!(differences, 0);
        assert_eq!(typed, Some(string.clone()), "the typed view answers");
        let home = string.as_ref().expect("answered").have;
        assert!(home > 0, "the home system's space dock produces: {home}");
    }
}

// -- the paid research whose bill takes the prerequisite away ---------------------------------------------------------------------------------------------

mod research {
    use ti4_content::ContentStore;
    use ti4_model::content_types::POK;
    use ti4_model::id::{PlanetId, PlayerId, SystemId, TechnologyId};
    use ti4_model::state::GameState;

    use crate::choice::{Scripted, Table};
    use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};

    const F3: [(&str, &str); 3] = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];

    fn a() -> PlayerId {
        PlayerId::new("a")
    }

    /// A planet (not a home planet) with a tech specialty, its system and a technology of no faction, no unit upgrade, whose only prerequisite is one of that colour.
    fn specialty_case() -> (PlanetId, SystemId, TechnologyId) {
        let content = ContentStore::embedded();
        let base = crate::fixtures::seated_game(&F3, POK);
        for (id, planet) in ti4_content::galaxy::all_planets(content, POK) {
            let (Some(system), false) = (planet.system_id(), planet.is_placed_during_play()) else { continue };
            if id == "jord" || planet.homeworld_of().is_some() {
                continue;
            }
            let planet_id = PlanetId::new(id);
            for colour in crate::technology::COLOURS {
                if crate::planets::tech_specialties_now(&base, content, POK, &planet_id).iter().any(|s| s.eq_ignore_ascii_case(colour)) {
                    let wanted = crate::technology::active_aliases(content, POK).into_iter().find(|t| {
                        let need = crate::technology::prerequisites(content, t);
                        need.len() == 1 && need.get(colour) == Some(&1) && !crate::technology::is_unit_upgrade(content, t) && crate::technology::faction_of(content, t).is_none()
                    });
                    if let Some(tech) = wanted {
                        return (planet_id, SystemId::new(system), tech);
                    }
                }
            }
        }
        panic!("the corpus has a specialty planet and a technology that needs its colour");
    }

    /// The secondary of `a` under the Dual backend: `(compared, bailed)` of its window and the string position afterwards.
    fn play(setup: impl FnOnce(&mut GameState), answers: &[&str]) -> ((u64, u64), GameState) {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        crate::dual::reset_thread_windows();
        let mut after = None;
        with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
            let mut state = crate::fixtures::seated_game(&F3, POK);
            crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
            setup(&mut state);
            let mut table = Table::with_default(Box::new(Scripted::new(answers.iter().map(|x| (*x).to_owned()))));
            crate::strategy_cards::secondary(&mut state, ContentStore::embedded(), POK, None, &mut table, &a(), "pok7technology").unwrap();
            after = Some(state);
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let counts = crate::dual::thread_window_counts("strategy_cards::technology_secondary");
        let _ = take_report();
        (counts, after.expect("played"))
    }

    #[test]
    fn a_bill_paid_with_the_specialty_planet_that_covered_the_prerequisite_ends_the_research_with_the_position_back_in_lockstep() {
        let (planet, system, tech) = specialty_case();
        // The specialty planet is the only ready planet of `a`, with trade goods for the rest of the four resources: the lone plan spends the planet.
        let resources = crate::production::planet_value_now(&crate::fixtures::seated_game(&F3, POK), ContentStore::embedded(), POK, &planet, crate::production::Spend::Resources);
        let only = |s: &mut GameState| {
            s.system_mut(&system).set_control(planet.clone(), a());
            let others: Vec<_> = s.controlled_planets(&a()).into_iter().map(|(_, p)| p.clone()).filter(|p| *p != planet).collect();
            for p in others {
                s.exhausted_planets.insert(p);
            }
            s.player_mut(&a()).unwrap().trade_goods = i32::try_from((4 - resources).max(0)).unwrap_or(0);
        };
        let (counts, after) = play(only, &[tech.as_str()]);
        assert_eq!(counts, (1, 0), "{tech}: compared, the core carries it: the string ends the research with nothing gained");
        assert!(!after.player(&a()).unwrap().technologies.contains(&tech), "nothing was researched");
        assert!(!after.exhausted_planets.contains(&planet), "the payment was undone");
    }
}
