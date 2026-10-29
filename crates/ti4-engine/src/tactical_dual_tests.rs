//! Package A2b: the tactical action runs on the core beside the string engine (`tactical_dual`,
//! `rules::tactical`) under `movement=dual`, answer by answer, and agrees with it. A bail is silent, so every test counts
//! the windows the core followed (`compared`) and the ones it left to the string (`bailed`).

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::POK;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use crate::dual::{Area, AreaMask, Backend, report, take_report, with_mask};
use crate::game::{Game, TACTICAL_ACTION_ID};

fn a() -> PlayerId {
    PlayerId::new("a")
}
fn b() -> PlayerId {
    PlayerId::new("b")
}

/// The windows of the area, by label.
const LABELS: [&str; 5] = ["tactical::activate", "tactical::move", "tactical::load", "tactical::finish", "tactical::free"];

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

fn sol_hacan() -> GameState {
    table(&[("a", "sol"), ("b", "hacan")])
}

struct Done {
    game_events: Vec<String>,
    state: GameState,
    rolls: usize,
    /// `(compared, bailed)` by label.
    counts: Vec<(&'static str, (u64, u64))>,
    /// The bail reasons the report counted.
    reasons: Vec<String>,
}

impl Done {
    fn counts(&self, label: &str) -> (u64, u64) {
        self.counts.iter().find(|(l, _)| *l == label).map_or((0, 0), |(_, c)| *c)
    }
}

/// Play the scripted answers on `galaxy` from `state`, with `movement=dual`, and fail on any difference.
fn play(state: GameState, galaxy: &Galaxy, script: &[&str], edit: impl FnOnce(&mut Game<'_>)) -> Done {
    let _guard = crate::dual::tests::serial();
    let _ = take_report();
    crate::dual::reset_thread_windows();
    let table = crate::choice::Table::with_default(Box::new(crate::choice::Scripted::new(script.iter().map(|s| (*s).to_owned()))));
    let mut game = Game::with_table(state, ContentStore::embedded(), table).with_galaxy(galaxy.clone());
    edit(&mut game);
    with_mask(AreaMask::STRING.with(Area::Movement, Backend::Dual), || {
        for _ in 0..40 {
            if game.step().error.is_some() {
                break; // the script ran out: we assert on what was reached
            }
        }
    });
    let r = report();
    assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
    let counts = LABELS.iter().map(|l| (*l, crate::dual::thread_window_counts(l))).collect();
    let reasons = r.windows.iter().filter(|(k, _)| k.starts_with("tactical::") && k.contains("[bail")).map(|(k, w)| format!("{k} x{}", w.bailed)).collect();
    let rolls = game.dice.history().iter().filter(|roll| roll.reason == "gravity rift").count();
    Done { game_events: game.events.clone(), state: game.state.clone(), rolls, counts, reasons }
}

fn hub() -> (crate::fixtures::Hub, SystemId, SystemId) {
    let hub = crate::fixtures::plain_hub();
    let (origin, target) = (SystemId::new(hub.outer[0].clone()), SystemId::new(hub.centre.clone()));
    (hub, origin, target)
}

#[test]
fn a_system_is_activated_in_lockstep() {
    let (hub, origin, target) = hub();
    let mut state = sol_hacan();
    crate::fixtures::put(&mut state, &origin, "carrier", &a(), 1);
    let done = play(state, &hub.galaxy, &[TACTICAL_ACTION_ID, target.as_str()], |_| {});
    assert_eq!(done.counts("tactical::activate"), (1, 0), "the core followed the activation: {:?}", done.reasons);
    assert!(done.game_events.iter().any(|e| e == &format!("SYSTEM_ACTIVATED:{target}")));
    assert_eq!(done.state.player(&a()).unwrap().tactic_tokens, 2, "one tactic token was spent");
    assert!(done.state.system_state(&target).command_tokens.contains(&a()));
}
