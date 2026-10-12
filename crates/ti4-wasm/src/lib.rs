//! The engine behind a wasm boundary, for local play in `web2/`.
//! See `README.md` for the measurements.
//!
//! The boundary is the plain C ABI rather than `wasm-bindgen`. An export returns a status: zero
//! or more is the length of a JSON result, less than zero is the negated length of an error text.
//! Either is read from the response buffer, whose address never changes ([`ti4_response_ptr`]).
//!
//! # Memory
//!
//! This crate does not allocate for its own data: results and the update for the host (the view
//! of the asked seat and its choice) are written straight into the fixed buffers of [`buffer`]. What still allocates
//! here is what the interfaces of the engine and of `ti4-view` ask for (owned ids, the decider
//! box, the copy of the state, a projected view) and the text of an error.

pub mod buffer;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, VecDeque};
use std::fmt::{self, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use serde::Serialize;
use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeededRandom, Table};
use ti4_engine::game::{Game, RunError};
use ti4_engine::reaction_modes::{NeverOffer, NeverSet};
use ti4_model::content_types::POK;
use ti4_model::id::PlayerId;
use ti4_model::state::{GameState, ReactionMode};
use ti4_view::components::catalog;
use ti4_view::map::{build_board_tiles, create_game_with_template};
use ti4_view::maps::{TemplateLoader, default_template_for};
use ti4_view::planning::draft::{DraftBase, DraftStep, ScriptAnswer, run_draft, script_answer};
use ti4_view::projection::{FactsContext, project_game_view_full, project_session_update};
use ti4_view::status::ViewerRole;
use ti4_view::view::{BoardTileView, GameView};

use crate::buffer::{DRAFT, REQUEST, RESPONSE, TooLarge, UPDATE, store};

const SEATS: [&str; 8] = ["a", "b", "c", "d", "e", "f", "g", "h"];
const ROUNDS: u32 = 50;

#[derive(Debug)]
enum Failure {
    Setup(String),
    Run(RunError),
    TooLarge(TooLarge),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Setup(error) => write!(f, "setup: {error}"),
            Self::Run(error) => write!(f, "run: {error}"),
            Self::TooLarge(error) => error.fmt(f),
        }
    }
}

impl From<TooLarge> for Failure {
    fn from(error: TooLarge) -> Self {
        Self::TooLarge(error)
    }
}

fn setup(error: impl fmt::Display) -> Failure {
    Failure::Setup(error.to_string())
}

/// A seeded game for `players` seats on the recommended map template for that count. With a
/// scenario, the board of that scenario is set up on it (`ti4_view::scenario`).
fn new_game(
    content: &'static ContentStore,
    seed: u64,
    players: usize,
    scenario: Option<&str>,
) -> Result<Game<'static>, Failure> {
    let seats = SEATS
        .get(..players)
        .filter(|seats| seats.len() >= 3)
        .ok_or_else(|| {
            setup(format_args!(
                "{players} players: a game has 3 to {} seats",
                SEATS.len()
            ))
        })?;
    let ids: Vec<PlayerId> = seats.iter().map(|name| PlayerId::new(*name)).collect();
    let loader = TemplateLoader::load().map_err(setup)?;
    let template = default_template_for(content, &loader, players, POK)
        .ok_or_else(|| setup(format_args!("no map template builds for {players} players")))?;
    let (mut state, mut galaxy) =
        create_game_with_template(content, &ids, seed, Some(&template)).map_err(setup)?;
    if let Some(scenario) = scenario {
        ti4_view::scenario::apply(scenario, &mut state, &mut galaxy, content, POK)
            .map_err(setup)?;
    }
    Ok(Game::with_seeded_random(state, content, seed)
        .with_sources(POK)
        .with_galaxy(galaxy))
}

/// What a view is made from while the game itself is borrowed by its run: a decider is given no
/// state, so the run leaves a copy here after each step.
struct Latest {
    state: GameState,
    tiles: Vec<BoardTileView>,
    /// The map, for the facts of a tactical action. It is copied once: a game of this host is
    /// played on the map it began with.
    galaxy: Option<Galaxy>,
}

static LATEST: Mutex<Option<Latest>> = Mutex::new(None);

fn latest_of(game: &Game<'_>, content: &ContentStore) -> Latest {
    Latest {
        state: game.state.clone(),
        tiles: game
            .galaxy()
            .map_or_else(Vec::new, |galaxy| build_board_tiles(content, galaxy)),
        galaxy: game.galaxy().cloned(),
    }
}

/// What the facts of an update of this host are made from: its games are of one content.
fn facts_context(galaxy: Option<&Galaxy>) -> FactsContext<'_> {
    FactsContext {
        content: ContentStore::embedded(),
        sources: POK,
        galaxy,
    }
}

/// Serializes as the text a value displays as, without building that text first.
struct Shown<T>(T);

impl<T: fmt::Display> Serialize for Shown<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

#[derive(Serialize)]
struct Report<'a> {
    decisions: usize,
    round: u32,
    /// Why the game stopped before its last round, when it did.
    stopped: Option<Shown<&'a RunError>>,
    state: Option<&'a GameState>,
    /// What seat `a` is shown of `state`.
    view: Option<GameView>,
}

fn respond(what: &'static str, report: &impl Serialize) -> Result<usize, Failure> {
    let mut response = RESPONSE.lock().expect("response lock");
    Ok(store(&mut response, what, "RESPONSE_CAPACITY", report)?)
}

/// Play a seeded game of `players` seats with random deciders for at most `max_steps` steps. The result
/// is the state reached and the number of decisions taken.
fn run_seeded(seed: u64, players: usize, max_steps: usize) -> Result<usize, Failure> {
    let content = ContentStore::embedded();
    let mut game = new_game(content, seed, players, None)?;
    match game.run(ROUNDS, max_steps) {
        Ok(_) | Err(RunError::StepLimit { .. }) => {}
        Err(error) => return Err(Failure::Run(error)),
    }
    let latest = latest_of(&game, content);
    let viewer = ViewerRole::Player(PlayerId::new("a"));
    respond(
        "the result of ti4_run_seeded",
        &Report {
            decisions: game.table.log.records.len(),
            round: game.state.round,
            stopped: None,
            state: Some(&game.state),
            view: Some(project_game_view_full(
                &latest.state,
                &viewer,
                &latest.tiles,
                None,
                &[],
            )),
        },
    )
}

/// What the seats on the host share during one game.
#[derive(Default)]
struct Host {
    /// How many choices the host was asked. The nonce of a choice is its number.
    asked: u64,
    /// The seat that was asked last: whose view the host is shown.
    viewer: Option<PlayerId>,
    /// The host left the game. Every later choice fails without asking, so the run unwinds.
    left: bool,
    /// An update did not fit its buffer. The engine's decider error carries text only, and the
    /// size must reach the host intact.
    too_large: Option<TooLarge>,
    /// The host takes back its last answer. Every later choice fails without asking, so the
    /// step unwinds, and the run goes back to a checkpoint.
    undo: bool,
    /// After an undo: the answers between the checkpoint and the choice that is open again.
    /// They are given without asking. (The engine's `Scripted` owns a boxed decider, which a
    /// static cannot hold.)
    script: VecDeque<String>,
    /// The answers that a checkpoint can still take back: the number of the choice, the id of
    /// the chosen option, and whether the choice was passed for the host ([`NEVER`]).
    recent: VecDeque<(u64, String, bool)>,
    /// How many choices the host had been asked at the oldest checkpoint.
    oldest: Option<u64>,
    /// The seats of the game, and which of them the host answers (bit 0 is seat `a`).
    players: usize,
    humans: u32,
    /// The game waits before a decision of a random seat ([`ti4_set_stepping`]), not in a choice.
    paused: bool,
    /// How many decisions the seats made, the random seats included. A checkpoint keeps it.
    decided: u64,
    /// The changes of the host's seats, in order ([`ti4_switch_seats`]).
    switches: Vec<Switch>,
    /// The host changed its seats in place of an answer: the decision is asked again.
    switched: bool,
    /// How many choices the host answers from a saved game ([`ti4_replay`]). They are asked
    /// whole: a window that was passed then is a saved answer.
    saved: u64,
}

/// From the decision with this number on, the host answers the seats of `mask`.
struct Switch {
    decision: u64,
    mask: u32,
    /// How many choices the host had been asked when the switch came into force. The answers up
    /// to it are of other seats, and are not taken back.
    asked: Option<u64>,
}

/// Whether the bit of `player` is set in `mask` (bit 0 is seat `a`).
fn in_mask(mask: u32, player: &PlayerId) -> bool {
    SEATS
        .iter()
        .position(|seat| *seat == player.as_str())
        .is_some_and(|index| mask >> index & 1 == 1)
}

impl Host {
    /// Whether the answer before the pending choice can be taken back from a checkpoint.
    fn can_undo(&self) -> bool {
        // The pending choice has the number `asked`, so the answer before it is `asked - 1`.
        // Before a decision of a random seat no choice is pending: the last answer is `asked`.
        // An answer from before the last switch of the seats stays.
        let fence = self.switch().and_then(|switch| switch.asked).unwrap_or(0);
        self.oldest
            .is_some_and(|oldest| oldest.max(fence) < self.again() && !self.left)
    }

    /// The number of the choice that an undo opens again: the last one that the host answered
    /// itself. A window that was passed for it ([`NEVER`]) would be passed again.
    fn again(&self) -> u64 {
        let mut again = (self.asked + u64::from(self.paused)).saturating_sub(1);
        while self
            .recent
            .iter()
            .any(|(number, _, passed)| *number == again && *passed)
        {
            again -= 1;
        }
        again
    }

    /// The switch that is in force.
    fn switch(&self) -> Option<&Switch> {
        self.switches
            .iter()
            .rfind(|switch| switch.decision <= self.decided)
    }

    /// The seats that the host answers now (bit 0 is seat `a`).
    fn mask(&self) -> u32 {
        self.switch().map_or(self.humans, |switch| switch.mask)
    }

    /// Whether the host answers the next decision of `player`.
    fn plays(&mut self, player: &PlayerId) -> bool {
        let (asked, decided) = (self.asked, self.decided);
        let switch = self
            .switches
            .iter_mut()
            .rfind(|switch| switch.decision <= decided);
        let mask = switch.map_or(self.humans, |switch| {
            switch.asked.get_or_insert(asked);
            switch.mask
        });
        in_mask(mask, player)
    }

    /// The seat that the host is shown the game as: the seat asked last when the host still
    /// answers it, or the first of its seats.
    fn shown_seat(&self) -> Option<PlayerId> {
        let mask = self.mask();
        self.viewer
            .clone()
            .filter(|viewer| in_mask(mask, viewer))
            .or_else(|| {
                SEATS
                    .iter()
                    .take(self.players)
                    .enumerate()
                    .find(|(index, _)| mask >> index & 1 == 1)
                    .map(|(_, seat)| PlayerId::new(*seat))
            })
    }
}

static HOST: Mutex<Host> = Mutex::new(Host {
    asked: 0,
    viewer: None,
    left: false,
    too_large: None,
    undo: false,
    script: VecDeque::new(),
    recent: VecDeque::new(),
    oldest: None,
    players: 0,
    humans: 0,
    paused: false,
    decided: 0,
    switches: Vec::new(),
    switched: false,
    saved: 0,
});

/// The printed names of the action cards that the host is never offered
/// ([`ti4_set_reaction_mode`]). A setting of the host, not of the game.
static NEVER: LazyLock<NeverSet> = LazyLock::new(|| Arc::new(Mutex::new(BTreeSet::new())));

/// A window that was passed for the host: the option that declined it, and the cards.
#[derive(Serialize)]
struct Passed {
    option: ChoiceOption,
    cards: Vec<String>,
}

/// The windows that were passed since the host last asked ([`ti4_passed`]).
static PASSED: Mutex<Vec<Passed>> = Mutex::new(Vec::new());

/// The answer of the host that lets a random seat decide. See [`ti4_set_stepping`].
pub const GO: i32 = -3;

/// The host is asked before each decision of a random seat.
static STEPPING: AtomicBool = AtomicBool::new(false);

/// The answer of the host after [`ti4_switch_seats`]: the decision is asked again, of the seat's
/// new decider.
pub const SWITCH: i32 = -4;

/// The answer of the host that takes back its last answer. See [`ti4_can_undo`].
pub const UNDO: i32 = -2;

/// How many checkpoints are kept: one after each step that asked the host. An answer older than
/// these is taken back by playing the game again from its seed, which the host does itself.
const CHECKPOINT_LIMIT: usize = 32;

/// The stream of the seats that decide at random. It is here, not inside the table, so that a
/// checkpoint can keep its position.
static RANDOM: Mutex<Option<SeededRandom>> = Mutex::new(None);

/// Every seat of a played game: the host answers the seats of its mask, and the others draw
/// from [`RANDOM`]. The mask can change during the game ([`ti4_switch_seats`]), so the decider
/// of a seat is found at each decision.
struct Seat {
    /// The host, behind its Never cards: a window that offers only such cards is declined
    /// without asking, and any other window is asked without them. The same wrapper as on the
    /// server (`ti4_engine::reaction_modes`).
    host: NeverOffer,
}

impl Seat {
    fn new() -> Self {
        let host = NeverOffer::new(Box::new(Asked), NEVER.clone()).on_skip(|choice, cards| {
            let Some(option) = choice.options.iter().find(|option| option.is_decline()) else {
                return;
            };
            // The window is a choice of the host with an answer, so an undo and a saved game
            // have it. The host is told with its next choice.
            let mut host = host();
            host.asked += 1;
            let number = host.asked;
            host.recent.push_back((number, option.id.clone(), true));
            PASSED.lock().expect("passed lock").push(Passed {
                option: option.clone(),
                cards: cards.to_vec(),
            });
        });
        Self { host }
    }
}

impl Decider for Seat {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        loop {
            let ours = host().plays(&choice.player);
            let answer = if !ours {
                draw(choice)?
            } else if let Some(option) = given(choice)? {
                Some(option)
            } else {
                // An answer of a saved game is given below the Never cards, as a replay on the
                // server is: the game is the same whatever the cards are set to now.
                let replayed = {
                    let host = host();
                    host.asked < host.saved
                };
                let answer = if replayed {
                    Asked.choose(choice)
                } else {
                    self.host.choose(choice)
                };
                match answer {
                    Ok(option) => Some(option),
                    Err(_) if std::mem::take(&mut host().switched) => None,
                    Err(error) => return Err(error),
                }
            };
            // Without an answer the host changed its seats here: the decision is asked again.
            if let Some(option) = answer {
                host().decided += 1;
                return Ok(option);
            }
        }
    }
}

/// The host itself: every choice that reaches it is handed out.
struct Asked;

impl Decider for Asked {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        ask_host(choice)?.ok_or_else(|| {
            host().switched = true;
            IllegalChoice::DeciderFailed {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
                reason: "the host changed its seats".to_owned(),
            }
        })
    }
}

/// The answer to a choice of the host that needs no asking: the game left, or the answer is
/// given again after an undo. None when the host is asked.
fn given(choice: &Choice) -> Result<Option<ChoiceOption>, IllegalChoice> {
    let failed = |reason: &str| IllegalChoice::DeciderFailed {
        player: choice.player.clone(),
        prompt: choice.prompt.clone(),
        reason: reason.to_owned(),
    };
    let mut host = host();
    if host.left {
        return Err(failed(LEFT));
    }
    if host.undo {
        return Err(failed(TAKEN_BACK));
    }
    let Some(wanted) = host.script.pop_front() else {
        return Ok(None);
    };
    host.asked += 1;
    choice
        .option(&wanted)
        .cloned()
        .map(Some)
        .ok_or_else(|| IllegalChoice::ScriptDiverged {
            player: choice.player.clone(),
            wanted,
            offered: choice.ids().into_iter().map(str::to_owned).collect(),
        })
}

/// A decision of a random seat: all random seats share one stream, in decision order. None
/// when the host changed its seats while the seat waited for it.
fn draw(choice: &Choice) -> Result<Option<ChoiceOption>, IllegalChoice> {
    if STEPPING.load(Ordering::Relaxed) && !wait_for_go(choice)? {
        return Ok(None);
    }
    RANDOM
        .lock()
        .expect("random lock")
        .as_mut()
        .expect("a played game has a random stream")
        .choose(choice)
        .map(Some)
}

/// Wait until the host lets the seat of `choice` decide. The host is shown the game as its own
/// seat sees it, with no choice of its own; the turn status names the seat that decides. False
/// when the host changed its seats instead.
fn wait_for_go(choice: &Choice) -> Result<bool, IllegalChoice> {
    let failed = |reason: &str| IllegalChoice::DeciderFailed {
        player: choice.player.clone(),
        prompt: choice.prompt.clone(),
        reason: reason.to_owned(),
    };
    let viewer = {
        let mut host = host();
        if host.left {
            return Err(failed(LEFT));
        }
        if host.undo {
            return Err(failed(TAKEN_BACK));
        }
        host.paused = true;
        host.shown_seat()
            .map_or(ViewerRole::Spectator, ViewerRole::Player)
    };
    let shown = show(&viewer, choice, "");
    PENDING.set(Some(choice.clone()));
    let index = if shown.is_ok() { host_ask() } else { LEAVE };
    PENDING.take();
    UPDATE.lock().expect("update lock").clear();
    let mut host = host();
    if let Err(error) = shown {
        host.too_large = Some(error);
    }
    if index == GO || index == SWITCH {
        host.paused = false;
        return Ok(index == GO);
    }
    if index == UNDO && host.can_undo() {
        // As if a choice with the next number were pending: `take_back` opens the one before it.
        host.paused = false;
        host.asked += 1;
        host.undo = true;
        return Err(failed(TAKEN_BACK));
    }
    host.paused = false;
    host.left = true;
    Err(failed(LEFT))
}

/// The answer that leaves the game.
const LEAVE: i32 = -1;

thread_local! {
    /// The game after each step that asked the host, oldest first; at first, the game at its
    /// start. Here, not in [`play`], so that a draft can start from the last one while the game
    /// is parked in a choice. A game is not `Send`, so this is not a static behind a lock.
    static CHECKPOINTS: RefCell<VecDeque<Checkpoint>> = const { RefCell::new(VecDeque::new()) };
    /// How many steps the game finished since the last checkpoint. None of them asked the host.
    static SINCE: Cell<usize> = const { Cell::new(0) };
    /// The game at its last finished step, for a draft. Made when the first draft asks for it,
    /// and dropped when the game goes on.
    static DRAFT_BASE: RefCell<Option<Game<'static>>> = const { RefCell::new(None) };
    /// The choice that the game is parked in, as the host is asked it: what a plan answers
    /// ([`ti4_plan`]). Before a decision of a random seat, the choice of that seat.
    static PENDING: RefCell<Option<Choice>> = const { RefCell::new(None) };
}

/// The game between two steps, with what the game does not hold itself.
struct Checkpoint {
    game: Game<'static>,
    random: Option<SeededRandom>,
    /// How many choices the host had been asked.
    asked: u64,
    /// How many decisions the log had.
    decisions: usize,
    /// How many decisions the seats had made ([`Host::decided`]).
    decided: u64,
}

/// The table of a played game: its seats, and the hook that keeps [`LATEST`] current inside a
/// step.
fn host_table() -> Table {
    let mut table = Table::with_default(Box::new(Seat::new()));
    // A choice inside a step is offered on the position it is asked in, not on the one the step
    // began with.
    table.on_observed_offer(|_, state| {
        if let Some(latest) = LATEST.lock().expect("latest lock").as_mut() {
            latest.state.clone_from(state);
        }
    });
    table
}

fn host() -> std::sync::MutexGuard<'static, Host> {
    HOST.lock().expect("host lock")
}

/// A decision of a seat of the host: the choice is handed out and the game waits for the
/// answer. None when the host changed its seats instead.
fn ask_host(choice: &Choice) -> Result<Option<ChoiceOption>, IllegalChoice> {
    let failed = |reason: &str| IllegalChoice::DeciderFailed {
        player: choice.player.clone(),
        prompt: choice.prompt.clone(),
        reason: reason.to_owned(),
    };
    let nonce = {
        let mut host = host();
        if host.left {
            return Err(failed(LEFT));
        }
        if host.undo {
            return Err(failed(TAKEN_BACK));
        }
        host.asked += 1;
        host.viewer = Some(choice.player.clone());
        host.asked
    };
    if let Err(error) = offer(choice, nonce) {
        let mut host = host();
        host.too_large = Some(error);
        host.left = true;
        return Err(failed("what the host is shown did not fit its buffer"));
    }
    // No lock is held while the host is asked: it reads the update while this stack is parked.
    PENDING.set(Some(choice.clone()));
    let index = host_ask();
    PENDING.take();
    UPDATE.lock().expect("update lock").clear();
    let mut host = host();
    if index == SWITCH {
        // The choice was not answered: the next one of the host gets its number.
        host.asked -= 1;
        return Ok(None);
    }
    if index == UNDO && host.can_undo() {
        host.undo = true;
        return Err(failed(TAKEN_BACK));
    }
    let Ok(index) = usize::try_from(index) else {
        host.left = true;
        return Err(failed(if index == UNDO {
            "the host took back an answer that no checkpoint holds; ti4_can_undo says when one does"
        } else {
            LEFT
        }));
    };
    let option = choice
        .options
        .get(index)
        .cloned()
        .ok_or_else(|| failed("the host answered with an index that is no option"))?;
    host.recent.push_back((nonce, option.id.clone(), false));
    Ok(Some(option))
}

const LEFT: &str = "the host left the game";
const TAKEN_BACK: &str = "the host took back its last answer";

/// Write what the asked seat is shown, and its choice, into the update buffer.
fn offer(choice: &Choice, nonce: u64) -> Result<(), TooLarge> {
    let mut number = NonceText::default();
    let _ = write!(number, "{nonce}");
    show(
        &ViewerRole::Player(choice.player.clone()),
        choice,
        number.as_str(),
    )
}

/// Write what `viewer` is shown while the game waits in `choice` into the update buffer. The
/// choice itself is in the update only when it is the viewer's.
fn show(viewer: &ViewerRole, choice: &Choice, nonce: &str) -> Result<(), TooLarge> {
    let latest = LATEST.lock().expect("latest lock");
    let Some(latest) = latest.as_ref() else {
        return Ok(());
    };
    let update = project_session_update(
        &latest.state,
        viewer,
        Some((choice, nonce)),
        &latest.tiles,
        Some(&facts_context(latest.galaxy.as_ref())),
    );
    store(
        &mut UPDATE.lock().expect("update lock"),
        "the update for the host",
        "UPDATE_CAPACITY",
        &update,
    )?;
    Ok(())
}

/// The decimal digits of a `u64`, without an allocation.
#[derive(Default)]
struct NonceText {
    bytes: [u8; 20],
    len: usize,
}

impl NonceText {
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("digits are ASCII")
    }
}

impl fmt::Write for NonceText {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len + text.len();
        self.bytes
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
fn host_ask() -> i32 {
    #[link(wasm_import_module = "host")]
    unsafe extern "C" {
        /// Returns the index of the chosen option, or a negative number to leave the game. The
        /// host may suspend the caller (JSPI).
        safe fn ask() -> i32;
    }
    ask()
}

/// The host of a native run: it is given the update and answers like the wasm import.
#[cfg(not(target_arch = "wasm32"))]
type NativeHost = Box<dyn FnMut(&str) -> i32 + Send>;

#[cfg(not(target_arch = "wasm32"))]
static NATIVE_HOST: Mutex<Option<NativeHost>> = Mutex::new(None);

/// Answer the choices of [`ti4_play`] in a native run. Without a host the first option is taken.
#[cfg(not(target_arch = "wasm32"))]
pub fn set_native_host(host: impl FnMut(&str) -> i32 + Send + 'static) {
    *NATIVE_HOST.lock().expect("native host lock") = Some(Box::new(host));
}

#[cfg(not(target_arch = "wasm32"))]
fn host_ask() -> i32 {
    let shown = update();
    NATIVE_HOST
        .lock()
        .expect("native host lock")
        .as_mut()
        .map_or(0, |host| host(&shown))
}

/// Play a seeded game of `players` seats to its end. A seat whose bit is set in `human_mask`
/// (bit 0 is seat `a`) is answered by the host; the other seats decide at random.
fn play(
    seed: u64,
    players: usize,
    human_mask: u32,
    scenario: Option<&str>,
) -> Result<usize, Failure> {
    let content = ContentStore::embedded();
    let mut game = new_game(content, seed, players, scenario)?;
    game.table = host_table();
    *RANDOM.lock().expect("random lock") = Some(SeededRandom::new(seed));
    {
        let mut host = host();
        // The switches and the answers of a game that is played again were set before this
        // call.
        let switches = std::mem::take(&mut host.switches);
        let saved = host.saved;
        *host = Host {
            players,
            humans: human_mask,
            switches,
            saved,
            ..Host::default()
        };
    }
    PASSED.lock().expect("passed lock").clear();
    CHECKPOINTS.with_borrow_mut(VecDeque::clear);
    *LATEST.lock().expect("latest lock") = Some(latest_of(&game, content));
    let target = game.state.round.saturating_add(ROUNDS);
    let mut stopped = None;
    // A checkpoint is made after each step that asked the host ([`CHECKPOINTS`]). A copy after
    // every step would cost a third of the run. With these, an undo plays the steps of the
    // random seats since the checkpoint again, which is the wait before any choice.
    let mut marked = None;
    while stopped.is_none() && game.state.round < target && !game.state.finished && !host().left {
        let asked = host().asked;
        if marked != Some(asked) {
            let oldest = CHECKPOINTS.with_borrow_mut(|checkpoints| {
                checkpoints.push_back(Checkpoint {
                    game: game.fork(),
                    random: RANDOM.lock().expect("random lock").clone(),
                    asked,
                    decisions: game.table.log.records.len(),
                    decided: host().decided,
                });
                if checkpoints.len() > CHECKPOINT_LIMIT {
                    checkpoints.pop_front();
                }
                checkpoints.front().map_or(asked, |first| first.asked)
            });
            SINCE.set(0);
            marked = Some(asked);
            let mut host = host();
            // An answer from before the oldest checkpoint cannot be taken back from one.
            host.oldest = Some(oldest);
            while host
                .recent
                .front()
                .is_some_and(|(number, ..)| *number <= oldest)
            {
                host.recent.pop_front();
            }
        }
        DRAFT_BASE.take();
        // No borrow of the checkpoints is held here: the step may park in a choice, and a
        // draft reads them then.
        let error = game.step().error;
        DRAFT_BASE.take();
        if std::mem::take(&mut host().undo) {
            game = CHECKPOINTS.with_borrow_mut(|checkpoints| take_back(checkpoints, game));
            SINCE.set(0);
            marked = Some(host().asked);
        } else {
            SINCE.set(SINCE.get() + 1);
            stopped = error.map(RunError::from);
        }
        if let Some(latest) = LATEST.lock().expect("latest lock").as_mut() {
            latest.state.clone_from(&game.state);
        }
    }
    let (viewer, too_large) = {
        let mut host = host();
        let viewer = host.shown_seat();
        // The switches and the saved answers are of this game.
        host.switches.clear();
        host.saved = 0;
        (viewer, host.too_large.take())
    };
    // A buffer that was too small fails the call. It must not sit in a field of a result.
    if let Some(error) = too_large {
        return Err(error.into());
    }
    // The last update: where the game stopped, with nothing left to answer.
    let viewer = viewer.map_or(ViewerRole::Spectator, ViewerRole::Player);
    let tiles = latest_of(&game, content).tiles;
    let update = project_session_update(
        &game.state,
        &viewer,
        None,
        &tiles,
        Some(&facts_context(game.galaxy())),
    );
    store(
        &mut UPDATE.lock().expect("update lock"),
        "the update for the host",
        "UPDATE_CAPACITY",
        &update,
    )?;
    respond(
        "the result of ti4_play",
        &Report {
            decisions: game.table.log.records.len(),
            round: game.state.round,
            stopped: stopped.as_ref().map(Shown),
            state: None,
            view: None,
        },
    )
}

/// Go back to the choice before the pending one: the game of the last checkpoint before it, and
/// the answers between the two, which are given again without asking.
fn take_back(checkpoints: &mut VecDeque<Checkpoint>, mut left: Game<'static>) -> Game<'static> {
    let mut host = host();
    // The pending choice has the number `asked`; the one to open again is the last one before
    // it that the host answered itself.
    let again = host.again();
    while checkpoints.len() > 1
        && checkpoints
            .back()
            .is_some_and(|checkpoint| checkpoint.asked >= again)
    {
        checkpoints.pop_back();
    }
    let checkpoint = checkpoints
        .back()
        .expect("the oldest checkpoint is before the choice: Host::can_undo");
    let mut game = checkpoint.game.fork();
    game.table = host_table();
    // The log goes on from where the checkpoint was taken.
    let mut records = std::mem::take(&mut left.table.log.records);
    records.truncate(checkpoint.decisions);
    game.table.log.records = records;
    RANDOM
        .lock()
        .expect("random lock")
        .clone_from(&checkpoint.random);
    host.recent.retain(|(number, ..)| *number < again);
    host.script = host
        .recent
        .iter()
        .filter(|(number, ..)| *number > checkpoint.asked)
        .map(|(_, id, _)| id.clone())
        .collect();
    // What is asked from here on is asked now, not played again from a saved game.
    host.saved = host.saved.min(again.saturating_sub(1));
    host.asked = checkpoint.asked;
    host.decided = checkpoint.decided;
    game
}

/// The game at its last finished step: the last checkpoint, and the steps of the random seats
/// since then played again on a copy of their stream. The game that is parked is not touched.
/// These steps asked the host nothing, so every seat draws: a seat of the host may have been a
/// random seat in them ([`ti4_switch_seats`]).
fn draft_base() -> Result<Game<'static>, Failure> {
    CHECKPOINTS.with_borrow(|checkpoints| {
        let checkpoint = checkpoints
            .back()
            .ok_or_else(|| setup("no game is played: a draft needs one"))?;
        let mut game = checkpoint.game.fork();
        let random = checkpoint
            .random
            .clone()
            .ok_or_else(|| setup("the game has no random stream"))?;
        game.table = Table::with_default(Box::new(random));
        for _ in 0..SINCE.get() {
            if let Some(error) = game.step().error {
                return Err(Failure::Run(error.into()));
            }
        }
        Ok(game)
    })
}

/// Run the draft script of the request buffer as the next turn of the host's seat, on a copy
/// of the game. The outcome is in the draft buffer.
fn draft() -> Result<usize, Failure> {
    #[derive(serde::Deserialize)]
    struct Request {
        script: Vec<DraftStep>,
    }
    let request: Request = serde_json::from_slice(REQUEST.lock().expect("request lock").as_bytes())
        .map_err(|error| setup(format_args!("the draft request is not read: {error}")))?;
    let player = host()
        .shown_seat()
        .ok_or_else(|| setup("no seat is played here: a draft is of one seat"))?;
    if DRAFT_BASE.with_borrow(Option::is_none) {
        let base = draft_base()?;
        DRAFT_BASE.set(Some(base));
    }
    let latest = LATEST.lock().expect("latest lock");
    let latest = latest
        .as_ref()
        .ok_or_else(|| setup("no game is played: a draft needs one"))?;
    let outcome = DRAFT_BASE.with_borrow(|base| {
        let base = DraftBase {
            game: base.as_ref().expect("the base was made above"),
            content: ContentStore::embedded(),
            sources: POK,
            tiles: &latest.tiles,
            galaxy: latest.galaxy.as_ref(),
        };
        run_draft(&base, &player, &request.script)
    });
    let mut buffer = DRAFT.lock().expect("draft lock");
    Ok(store(
        &mut buffer,
        "the outcome of a draft",
        "DRAFT_CAPACITY",
        &outcome,
    )?)
}

/// What the script of the request buffer answers to the choice that the game is parked in.
fn plan() -> Result<usize, Failure> {
    #[derive(serde::Deserialize)]
    struct Request {
        script: Vec<DraftStep>,
        next: usize,
        actor: PlayerId,
    }
    #[derive(Serialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Reply {
        Answer {
            /// The option of the pending choice, by its place.
            index: usize,
            next: usize,
            #[serde(skip_serializing_if = "Option::is_none")]
            script: Option<Vec<DraftStep>>,
        },
        Done,
        Interrupted {
            reason: String,
        },
        Mismatch {
            step: usize,
            reason: String,
        },
    }
    let request: Request = serde_json::from_slice(REQUEST.lock().expect("request lock").as_bytes())
        .map_err(|error| setup(format_args!("the plan request is not read: {error}")))?;
    let answer = PENDING.with_borrow(|pending| -> Result<Reply, Failure> {
        let choice = pending
            .as_ref()
            .ok_or_else(|| setup("the game is not parked in a choice: a plan answers one"))?;
        let answer = script_answer(&request.script, request.next, choice, &request.actor);
        Ok(match answer {
            ScriptAnswer::Answer {
                option,
                next,
                script,
            } => Reply::Answer {
                index: choice
                    .options
                    .iter()
                    .position(|offered| offered.id == option.id)
                    .expect("the answer is an option of the choice"),
                next,
                script,
            },
            ScriptAnswer::Done => Reply::Done,
            ScriptAnswer::Interrupted { reason } => Reply::Interrupted { reason },
            ScriptAnswer::Mismatch { step, reason } => Reply::Mismatch { step, reason },
        })
    })?;
    respond("the answer of a plan", &answer)
}

/// Set whether the host is offered a card: the request buffer holds `{"card", "mode"}`.
fn set_reaction_mode() -> Result<usize, Failure> {
    #[derive(serde::Deserialize)]
    struct Request {
        card: String,
        mode: ReactionMode,
    }
    let request: Request = serde_json::from_slice(REQUEST.lock().expect("request lock").as_bytes())
        .map_err(|error| setup(format_args!("the reaction mode is not read: {error}")))?;
    let mut never = NEVER.lock().expect("never set lock");
    match request.mode {
        ReactionMode::Never => never.insert(request.card),
        ReactionMode::Always => never.remove(&request.card),
    };
    Ok(0)
}

/// Whether the request buffer holds `request_len` bytes; else the error that says so.
fn request_filled(request_len: u32) -> Result<(), Failure> {
    if REQUEST
        .lock()
        .expect("request lock")
        .filled(request_len as usize)
    {
        return Ok(());
    }
    Err(setup(format_args!(
        "ti4-wasm buffer full: the request is {request_len} bytes, but REQUEST_CAPACITY is {} \
         bytes. Raise REQUEST_CAPACITY in crates/ti4-wasm/src/buffer.rs and rebuild, or make \
         the request smaller.",
        buffer::REQUEST_CAPACITY
    )))
}

/// The status of an export: a length, negated when the response buffer holds an error text.
fn status(result: Result<usize, Failure>) -> i32 {
    let length = |bytes: usize| i32::try_from(bytes).expect("a buffer is smaller than 2 GiB");
    match result {
        Ok(bytes) => length(bytes),
        Err(failure) => {
            let mut response = RESPONSE.lock().expect("response lock");
            response.clear();
            let _ = write!(response, "{failure}");
            -length(response.len())
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ti4_run_seeded(seed: u32, players: u32, max_steps: u32) -> i32 {
    status(run_seeded(
        u64::from(seed),
        players as usize,
        max_steps as usize,
    ))
}

/// Play a game to its end, asking the host for the seats in `human_mask`. With JSPI the export is
/// wrapped in `WebAssembly.promising` and the import `host.ask` in `WebAssembly.Suspending`.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_play(seed: u32, players: u32, human_mask: u32) -> i32 {
    status(play(u64::from(seed), players as usize, human_mask, None))
}

/// Play a scenario of `ti4_view::scenario` as [`ti4_play`] plays a new game: the request buffer
/// holds its id in `id_len` bytes. The seed and the number of seats are those of the scenario.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_play_scenario(human_mask: u32, id_len: u32) -> i32 {
    // The id is copied: a draft writes its request into the same buffer while the game waits.
    let id = {
        let mut request = REQUEST.lock().expect("request lock");
        request
            .filled(id_len as usize)
            .then(|| String::from_utf8_lossy(request.as_bytes()).into_owned())
    };
    let scenario = id.as_deref().and_then(ti4_view::scenario::find);
    let Some(scenario) = scenario else {
        let known: Vec<&str> = ti4_view::scenario::SCENARIOS
            .iter()
            .map(|scenario| scenario.id)
            .collect();
        return status(Err(setup(format_args!(
            "no scenario '{}': the scenarios are {}",
            id.as_deref()
                .unwrap_or("(an id longer than the request buffer)"),
            known.join(", ")
        ))));
    };
    status(play(
        scenario.seed,
        scenario.players,
        human_mask,
        Some(scenario.id),
    ))
}

/// The scenarios that [`ti4_play_scenario`] plays, as a JSON list in the response buffer.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_scenarios() -> i32 {
    status(respond(
        "the list of scenarios",
        &ti4_view::scenario::SCENARIOS,
    ))
}

/// The printed cards as JSON in the response buffer: the exploration cards and the attachments,
/// and the cards of the play areas (abilities, leaders, technologies, relics). A host asks once:
/// an update names cards by id only.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_catalog() -> i32 {
    status(respond(
        "the catalogue of printed cards",
        &catalog(ContentStore::embedded(), POK),
    ))
}

/// Ask the host before each decision of a random seat (1), or let these seats play on (0). The
/// host is then asked with an update that has no pending choice, and answers [`GO`]. The seat
/// decides from the seeded stream either way, so the game is the same game.
///
/// Only a flag is set: the host may call this while the game waits.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_set_stepping(on: u32) {
    STEPPING.store(on != 0, Ordering::Relaxed);
}

/// How many decisions the seats of the game made, the random seats included. While the game
/// waits, this is the number of the decision that it waits for.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_decided() -> u32 {
    u32::try_from(host().decided).expect("a game has fewer decisions than 2^32")
}

/// From the decision with the number `decision` on, the host answers the seats of `human_mask`
/// and the others decide at random.
///
/// While the game waits, the host calls this with [`ti4_decided`] and then answers [`SWITCH`]:
/// the decision is asked again, of the seat's new decider. The answers of the host from before
/// are then not taken back ([`ti4_can_undo`]). For a game that is played again, the host calls
/// this before [`ti4_play`] for each switch of the game, in order.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_switch_seats(decision: u32, human_mask: u32) {
    host().switches.push(Switch {
        decision: u64::from(decision),
        mask: human_mask,
        asked: None,
    });
}

/// Where the host writes a request before the export that reads it. Constant for the life of
/// the instance; `REQUEST_CAPACITY` bytes.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_request_ptr() -> *mut u8 {
    REQUEST.lock().expect("request lock").as_mut_ptr()
}

/// Run a draft: the request buffer holds `{"script": [...]}` in `request_len` bytes, the steps
/// of `ti4_view::planning::draft::DraftStep`. The script is played as the next turn of the
/// host's seat on a copy of the game at its last finished step, and the outcome
/// (`DraftOutcome`) is left at [`ti4_draft_ptr`] with the returned length. A status below 0 is
/// the negated length of an error text in the response buffer.
///
/// The game itself is not touched, so the host calls this while the game waits in a choice.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_draft(request_len: u32) -> i32 {
    status(request_filled(request_len).and_then(|()| draft()))
}

/// What a script answers to the choice that the game is parked in: the request buffer holds
/// `{"script": [...], "next": n, "actor": seat}` in `request_len` bytes, the entries of
/// `ti4_view::planning::draft::DraftStep`, the first entry that is still to play, and the seat
/// whose script it is. The response is one of `{"kind":"answer","index","next","script"?}`
/// (answer the choice with the option at `index`; with `script` the script goes on in that
/// order), `{"kind":"done"}`, `{"kind":"interrupted","reason"}` and
/// `{"kind":"mismatch","step","reason"}` (`ti4_view::planning::draft::script_answer`).
///
/// This is how a host sends a plan or a draft to the game: it asks here at each choice and
/// answers with the index, until the script is done or stops. Nothing is changed by the call.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_plan(request_len: u32) -> i32 {
    status(request_filled(request_len).and_then(|()| plan()))
}

/// Set the mode of an action card for the host: the request buffer holds
/// `{"card": <printed name>, "mode": "never" | "always"}` in `request_len` bytes. A window that
/// offers only Never cards is declined for the host without asking ([`ti4_passed`]), and any
/// other window is asked without them (`ti4_engine::reaction_modes::NeverOffer`).
///
/// The modes are of the instance, not of the game: the host sets them before [`ti4_play`], or
/// while the game waits.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_set_reaction_mode(request_len: u32) -> i32 {
    status(request_filled(request_len).and_then(|()| set_reaction_mode()))
}

/// The windows that were passed for the host since it last asked, as a JSON list of
/// `{"option", "cards"}` in the response buffer: the option that declined the window, and the
/// printed names of its cards. A passed window is a choice of the host with an answer: it has a
/// number, an undo goes back over it, and a saved game has its answer.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_passed() -> i32 {
    let passed = std::mem::take(&mut *PASSED.lock().expect("passed lock"));
    status(respond("the passed windows", &passed))
}

/// The first `answers` choices of the next [`ti4_play`] are answered from a saved game. They
/// are asked whole, whatever the modes of the cards are: a window that was passed when the game
/// was played is one of the saved answers.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_replay(answers: u32) {
    host().saved = u64::from(answers);
}

/// Where the outcome of the last [`ti4_draft`] is kept. Constant for the life of the instance.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_draft_ptr() -> *const u8 {
    DRAFT.lock().expect("draft lock").as_ptr()
}

/// Whether the host may answer the pending choice with [`UNDO`]: 1 when a checkpoint holds the
/// answer before it, else 0. Then that choice is asked again, as it was.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_can_undo() -> u32 {
    u32::from(host().can_undo())
}

/// Where every export leaves its result or its error. Constant for the life of the instance.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_response_ptr() -> *const u8 {
    RESPONSE.lock().expect("response lock").as_ptr()
}

/// Where the update for the host is kept: while the game is suspended in a choice, and after
/// [`ti4_play`] has returned. Constant for the life of the instance.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_update_ptr() -> *const u8 {
    UPDATE.lock().expect("update lock").as_ptr()
}

/// The length of that update, or zero when there is none.
#[unsafe(no_mangle)]
pub extern "C" fn ti4_update_len() -> u32 {
    u32::try_from(UPDATE.lock().expect("update lock").len()).expect("a buffer is small")
}

/// The update for the host, for callers on the native side.
#[must_use]
pub fn update() -> String {
    String::from_utf8_lossy(UPDATE.lock().expect("update lock").as_bytes()).into_owned()
}

/// Run a draft script (JSON, see [`ti4_draft`]) and return its outcome, for callers on the
/// native side.
///
/// # Errors
/// The error text of the export.
pub fn draft_native(request: &str) -> Result<String, String> {
    fill_request(request)?;
    let status = status(draft());
    if status < 0 {
        return response(status);
    }
    Ok(String::from_utf8_lossy(DRAFT.lock().expect("draft lock").as_bytes()).into_owned())
}

/// Ask what a script answers to the pending choice (JSON, see [`ti4_plan`]), for callers on the
/// native side.
///
/// # Errors
/// The error text of the export.
pub fn plan_native(request: &str) -> Result<String, String> {
    fill_request(request)?;
    response(status(plan()))
}

/// Set the mode of a card as [`ti4_set_reaction_mode`] does, for callers on the native side.
///
/// # Errors
/// The error text of the export.
pub fn set_reaction_mode_native(card: &str, mode: ReactionMode) -> Result<(), String> {
    let request = serde_json::json!({ "card": card, "mode": mode }).to_string();
    fill_request(&request)?;
    response(status(set_reaction_mode())).map(|_| ())
}

fn fill_request(request: &str) -> Result<(), String> {
    let mut buffer = REQUEST.lock().expect("request lock");
    buffer.clear();
    std::io::Write::write_all(&mut *buffer, request.as_bytes())
        .map_err(|_| "the request does not fit REQUEST_CAPACITY".to_owned())
}

/// Play a scenario as [`ti4_play_scenario`] does, for callers on the native side.
#[must_use]
pub fn play_scenario_native(id: &str, human_mask: u32) -> i32 {
    let fits = {
        let mut buffer = REQUEST.lock().expect("request lock");
        buffer.clear();
        std::io::Write::write_all(&mut *buffer, id.as_bytes()).is_ok()
    };
    let length = if fits { id.len() } else { usize::MAX };
    ti4_play_scenario(human_mask, u32::try_from(length).unwrap_or(u32::MAX))
}

/// The response of the last export, for callers on the native side.
///
/// # Errors
/// The error text, when `status` is negative.
pub fn response(status: i32) -> Result<String, String> {
    let response = RESPONSE.lock().expect("response lock");
    let text = String::from_utf8_lossy(response.as_bytes()).into_owned();
    if status < 0 { Err(text) } else { Ok(text) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_eight_player_game_runs_with_random_deciders() {
        let content = ContentStore::embedded();
        let mut game = new_game(content, 3, 8, None).unwrap();
        assert_eq!(game.state.players.len(), 8);
        match game.run(ROUNDS, 2000) {
            Ok(_) | Err(RunError::StepLimit { .. }) => {}
            Err(error) => panic!("{error}"),
        }
        assert!(game.state.round > 1, "the game left its first round");
    }

    #[test]
    fn the_scenarios_run_with_random_deciders() {
        let content = ContentStore::embedded();
        for scenario in ti4_view::scenario::SCENARIOS {
            let mut game =
                new_game(content, scenario.seed, scenario.players, Some(scenario.id)).unwrap();
            match game.run(ROUNDS, 400) {
                Ok(_) | Err(RunError::StepLimit { .. }) => {}
                Err(error) => panic!("{}: {error}", scenario.id),
            }
            assert!(
                game.table.log.records.len() > 100,
                "{}: the seats decided",
                scenario.id
            );
        }
    }

    /// A seat that draws from a stream which a checkpoint can copy.
    struct Shared(std::sync::Arc<Mutex<ti4_engine::choice::SeededRandom>>);

    impl Decider for Shared {
        fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
            self.0.lock().unwrap().choose(choice)
        }
    }

    /// `Game::fork` at a step boundary, with a copy of the random seats' stream, is a full
    /// checkpoint: played on, it makes the decisions of the game it was taken from and reaches
    /// the same state. A game made again from the state alone does not.
    ///
    /// Prints how often a window is open at a step boundary and what a fork costs:
    /// `cargo test -p ti4-wasm fork -- --nocapture`. The numbers in the README are from seeds 3
    /// and 11 with 700 steps, a copy every 7 steps, played 30 steps on.
    #[test]
    fn a_fork_at_a_step_boundary_plays_the_same_game() {
        use std::sync::Arc;
        use std::time::{Duration, Instant};
        use ti4_engine::choice::{SeededRandom, Table};

        const STEPS: usize = 300;
        const EVERY: usize = 10;
        const AHEAD: usize = 20;
        /// A copy of the game at one step, played `AHEAD` steps on.
        struct Probe {
            step: usize,
            from: usize,
            decisions: Vec<ti4_engine::choice::DecisionRecord>,
            state: String,
        }
        let content = ContentStore::embedded();
        let state_of =
            |game: &Game<'_>| serde_json::to_string(&game.state).expect("a state serializes");
        let ahead = |mut copy: Game<'static>, step: usize, from: usize| {
            for _ in 0..AHEAD {
                if copy.state.finished || copy.step().error.is_some() {
                    break;
                }
            }
            Probe {
                step,
                from,
                decisions: copy.table.log.records.clone(),
                state: state_of(&copy),
            }
        };

        for seed in [3] {
            let mut game = new_game(content, seed, 8, None).unwrap();
            let stream = Arc::new(Mutex::new(SeededRandom::new(seed)));
            game.table = Table::with_default(Box::new(Shared(stream.clone())));
            let table = || {
                let copy = stream.lock().unwrap().clone();
                Table::with_default(Box::new(Shared(Arc::new(Mutex::new(copy)))))
            };

            let (mut forks, mut remade) = (Vec::new(), Vec::new());
            let (mut quiet, mut open) = (0, std::collections::BTreeMap::new());
            let (mut fork_time, mut largest) = (Duration::ZERO, 0);
            let mut states = Vec::new();
            for step in 0..STEPS {
                if game.state.finished {
                    break;
                }
                states.push(state_of(&game));
                let windows = game.open_windows();
                if windows.is_empty() {
                    quiet += 1;
                }
                for window in &windows {
                    *open.entry(*window).or_insert(0) += 1;
                }
                // A copy is compared over `AHEAD` steps, so the run must go that far.
                if step % EVERY == 0 && step + AHEAD < STEPS {
                    let from = game.table.log.records.len();
                    let start = Instant::now();
                    let mut fork = game.fork();
                    fork_time += start.elapsed();
                    largest = largest.max(states[step].len());
                    fork.table = table();
                    forks.push(ahead(fork, step, from));
                    // What the server's snapshot holds: the state, and nothing else of the game.
                    let again = Game::with_table(game.state.clone(), content, table())
                        .with_sources(POK)
                        .with_galaxy(game.galaxy().unwrap().clone());
                    remade.push((ahead(again, step, from), windows.is_empty()));
                }
                assert!(game.step().error.is_none());
            }
            states.push(state_of(&game));

            let log = &game.table.log.records;
            let same = |probe: &Probe| {
                let end = probe.from + probe.decisions.len();
                log.get(probe.from..end) == Some(&probe.decisions[..])
                    && states.get(probe.step + AHEAD) == Some(&probe.state)
            };
            let wrong: Vec<usize> = forks
                .iter()
                .filter(|probe| !same(probe))
                .map(|probe| probe.step)
                .collect();
            let count = |quiet_only: bool| {
                let of: Vec<_> = remade
                    .iter()
                    .filter(|(_, quiet)| *quiet || !quiet_only)
                    .collect();
                (of.iter().filter(|(probe, _)| same(probe)).count(), of.len())
            };
            println!(
                "seed {seed}: {} steps, {} decisions; no window open at {quiet} boundaries; open: {open:?}",
                states.len() - 1,
                log.len()
            );
            println!(
                "  forks: {} of {} play the same game; one fork takes {:?}; the state is up to {largest} bytes of JSON",
                forks.len() - wrong.len(),
                forks.len(),
                fork_time / u32::try_from(forks.len()).unwrap()
            );
            println!(
                "  made again from the state alone: {:?} the same; at boundaries with no window open: {:?}",
                count(false),
                count(true)
            );
            assert!(
                wrong.is_empty(),
                "forks at steps {wrong:?} played another game"
            );
        }
    }

    /// One test for everything that plays through the host: the host and the update buffer are
    /// statics, so two such tests would not be independent.
    #[test]
    fn the_host_is_asked_for_its_seats_can_leave_and_a_replay_is_the_same_game() {
        use std::sync::{Arc, Mutex};
        let seen = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let sink = seen.clone();
        set_native_host(move |update| {
            let mut seen = sink.lock().unwrap();
            seen.push(serde_json::from_str(update).unwrap());
            if seen.len() == 12 { -1 } else { 0 }
        });
        // Seats a and c.
        let status = ti4_play(3, 8, 0b101);
        let report: serde_json::Value = serde_json::from_str(&response(status).unwrap()).unwrap();
        assert!(
            report["stopped"]
                .as_str()
                .is_some_and(|text| text.contains(LEFT)),
            "{report}"
        );
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 12);
        let mut seats = std::collections::BTreeSet::new();
        for (index, update) in seen.iter().enumerate() {
            let seat = update["pending_choice"]["choice"]["player"]
                .as_str()
                .unwrap();
            seats.insert(seat.to_owned());
            assert_eq!(update["viewer"]["seat"], seat);
            assert_eq!(update["pending_choice"]["nonce"], (index + 1).to_string());
            assert_eq!(update["turn_status"]["seat"], seat);
            assert_eq!(update["view"]["players"].as_array().unwrap().len(), 8);
        }
        assert!(
            seats.iter().all(|seat| seat == "a" || seat == "c"),
            "{seats:?}"
        );
        // After the game: the last update, with nothing to answer.
        let last: serde_json::Value = serde_json::from_str(&update()).unwrap();
        assert!(last.get("pending_choice").is_none());
        assert_eq!(last["viewer"]["role"], "player");
        drop(seen);

        // A saved game is the seed and the ids of the host's answers. Played again with them,
        // the game asks the same choices and reaches the same position.
        const ANSWERS: usize = 40;
        let options = |update: &serde_json::Value| -> Vec<String> {
            update["pending_choice"]["choice"]["options"]
                .as_array()
                .unwrap()
                .iter()
                .map(|option| option["id"].as_str().unwrap().to_owned())
                .collect()
        };
        let first = Arc::new(Mutex::new((Vec::<String>::new(), String::new())));
        let sink = first.clone();
        set_native_host(move |update| {
            let (answers, reached) = &mut *sink.lock().unwrap();
            if answers.len() == ANSWERS {
                update.clone_into(reached);
                return -1;
            }
            let ids = options(&serde_json::from_str(update).unwrap());
            let index = (answers.len() * 7) % ids.len();
            answers.push(ids[index].clone());
            i32::try_from(index).unwrap()
        });
        ti4_play(3, 8, 1);
        let (answers, reached) = first.lock().unwrap().clone();
        assert_eq!(answers.len(), ANSWERS);

        let again = Arc::new(Mutex::new((0, String::new())));
        let sink = again.clone();
        set_native_host(move |update| {
            let (asked, reached) = &mut *sink.lock().unwrap();
            let Some(wanted) = answers.get(*asked) else {
                update.clone_into(reached);
                return -1;
            };
            *asked += 1;
            let ids = options(&serde_json::from_str(update).unwrap());
            let index = ids.iter().position(|id| id == wanted);
            i32::try_from(index.expect("the replayed game offers the saved answer")).unwrap()
        });
        ti4_play(3, 8, 1);
        assert_eq!(again.lock().unwrap().1, reached);
        assert!(
            reached.contains("\"nonce\":\"41\""),
            "the position after 40 answers"
        );

        // An undo opens the choice before the pending one exactly as it was shown, and the same
        // answers lead to the same choices again. Five in a row go back over several steps.
        #[derive(Default)]
        struct Undoing {
            shown: std::collections::BTreeMap<u64, String>,
            shown_again: usize,
            left: usize,
            active: bool,
        }
        let undoing = Arc::new(Mutex::new(Undoing {
            left: 5,
            ..Undoing::default()
        }));
        let sink = undoing.clone();
        set_native_host(move |update| {
            let mut undoing = sink.lock().unwrap();
            let value: serde_json::Value = serde_json::from_str(update).unwrap();
            let number: u64 = value["pending_choice"]["nonce"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();
            if let Some(before) = undoing.shown.get(&number) {
                assert_eq!(before, update, "choice {number} is shown as before");
                undoing.shown_again += 1;
            } else {
                undoing.shown.insert(number, update.to_owned());
            }
            if number == 1 {
                assert_eq!(ti4_can_undo(), 0, "there is no answer to take back yet");
            }
            if number == 41 && undoing.left > 0 {
                undoing.active = true;
            }
            if undoing.active && undoing.left > 0 {
                assert_eq!(ti4_can_undo(), 1);
                undoing.left -= 1;
                return UNDO;
            }
            undoing.active = false;
            if number == 42 {
                return -1;
            }
            let count = options(&value).len();
            i32::try_from(usize::try_from(number).unwrap() * 7 % count).unwrap()
        });
        let status = ti4_play(3, 8, 1);
        let report = response(status).unwrap();
        assert!(report.contains(LEFT), "{report}");
        let undoing = undoing.lock().unwrap();
        // Choices 40 down to 36 after each undo, then 37 to 41 after the same answers.
        assert_eq!(undoing.shown_again, 10);
        assert_eq!(undoing.shown.len(), 42);
        drop(undoing);

        // The facts of a tactical action say what the engine then offers: seat a activates the
        // system next to its home, moves a carrier, and is offered the cargo of the facts.
        let seen = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let sink = seen.clone();
        set_native_host(move |update| {
            let value: serde_json::Value = serde_json::from_str(update).unwrap();
            let mut seen = sink.lock().unwrap();
            let script = ["pok8imperial", "no", "tactical", "23", "move|01|0"];
            let wanted = script.get(seen.len()).copied();
            let index = wanted.and_then(|id| options(&value).iter().position(|have| have == id));
            seen.push(value);
            index.map_or(-1, |index| i32::try_from(index).unwrap())
        });
        let status = ti4_play(3, 8, 1);
        assert!(response(status).unwrap().contains(LEFT));
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 6);
        assert!(
            seen[2].get("tactical").is_none(),
            "the turn menu has no facts"
        );

        let activation = &seen[3]["tactical"];
        assert_eq!(activation["kind"], "activation");
        assert_eq!(activation["tactic_tokens"], 3);
        let reach = activation["systems"].as_array().unwrap();
        assert_eq!(reach.len(), options(&seen[3]).len());
        let next_to_home = reach.iter().find(|fact| fact["system"] == "23").unwrap();
        // Two carriers and a destroyer; the fighters do not move on their own.
        assert_eq!(next_to_home["ships"], 3);
        assert_eq!(next_to_home["origins"], 1);
        // The seat produces at home only, and has no ships of another player to fire at.
        assert!(next_to_home.get("production").is_none());
        let producing: Vec<_> = reach
            .iter()
            .filter(|fact| fact.get("production").is_some())
            .collect();
        assert_eq!(producing.len(), 1);
        assert!(reach.iter().all(|fact| fact.get("cannon").is_none()));

        let movement = &seen[4]["tactical"];
        assert_eq!(movement["kind"], "movement");
        assert_eq!(movement["active"], "23");
        assert_eq!(movement["moved"], 0);
        let ships = movement["ships"].as_array().unwrap();
        assert_eq!(ships.len(), 3, "{ships:?}");
        // Every ship that the engine offers is a ship of the facts that can move.
        let offered = seen[4]["pending_choice"]["choice"]["options"]
            .as_array()
            .unwrap();
        for option in offered.iter().filter(|option| option["kind"] == "move") {
            let (_, place) = option["id"].as_str().unwrap().split_once('|').unwrap();
            let (origin, index) = place.split_once('|').unwrap();
            let ship = ships
                .iter()
                .find(|ship| {
                    ship["origin"] == origin && ship["index"].as_u64() == index.parse().ok()
                })
                .unwrap_or_else(|| panic!("no ship for {option}"));
            assert_eq!(ship["unit"], option["payload"]["unit"]);
            assert_eq!(ship["capacity"], option["payload"]["capacity"]);
            assert_eq!(ship["move"]["path"], serde_json::json!(["01", "23"]));
        }
        // The hold of the carrier is offered the pools that the facts name for it.
        let carrier = &ships[0];
        let pools = movement["cargo"].as_array().unwrap();
        let mut named: Vec<_> = carrier["loads"]
            .as_array()
            .unwrap()
            .iter()
            .map(|place| {
                let pool = &pools[usize::try_from(place.as_u64().unwrap()).unwrap()];
                assert!(pool["count"].as_u64().unwrap() > 0, "{pool}");
                (
                    pool["system"].clone(),
                    pool["source"].clone(),
                    pool["unit"].clone(),
                )
            })
            .collect();
        let mut loads: Vec<_> = seen[5]["pending_choice"]["choice"]["options"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|option| option["kind"] == "load")
            .map(|option| {
                let payload = &option["payload"];
                (
                    payload["pickup_system"].clone(),
                    payload["source"].clone(),
                    payload["unit"].clone(),
                )
            })
            .collect();
        let text = |value: &(serde_json::Value, serde_json::Value, serde_json::Value)| {
            format!("{value:?}")
        };
        named.sort_by_key(text);
        loads.sort_by_key(text);
        assert!(!loads.is_empty());
        assert_eq!(named, loads);
        assert!(seen[5].get("tactical").is_none(), "a hold has no facts");
        let live_movement = seen[4].clone();
        drop(seen);

        // The landings: the carrier takes two infantry along, and the facts of each landing say
        // what the engine offers, with the count of the units that are still in space.
        let seen = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let sink = seen.clone();
        set_native_host(move |update| {
            let value: serde_json::Value = serde_json::from_str(update).unwrap();
            let mut seen = sink.lock().unwrap();
            let ids = options(&value);
            let infantry = value["pending_choice"]["choice"]["options"]
                .as_array()
                .unwrap()
                .iter()
                .position(|option| {
                    option["kind"] == "load" && option["payload"]["unit"] == "sol_infantry"
                });
            let pick = |id: &str| ids.iter().position(|have| have == id);
            let index = match seen.len() {
                0 => pick("pok8imperial"),
                1 => pick("no"),
                2 => pick("tactical"),
                3 => pick("23"),
                4 => pick("move|01|0"),
                5 | 6 => infantry,
                7 => pick("done_loading"),
                8 => pick("done_moving"),
                9 => pick("commit|0|saudor"),
                _ => None,
            };
            seen.push(value);
            index.map_or(-1, |index| i32::try_from(index).unwrap())
        });
        let status = ti4_play(3, 8, 1);
        assert!(response(status).unwrap().contains(LEFT));
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 11);
        for (update, left) in [(&seen[9], 2), (&seen[10], 1)] {
            let facts = &update["tactical"];
            assert_eq!(facts["kind"], "invasion");
            assert_eq!(facts["active"], "23");
            assert_eq!(
                facts["forces"],
                serde_json::json!([{ "unit": "sol_infantry", "damaged": false, "count": left }])
            );
            // Each landing that the engine offers is one force at one planet of the facts.
            let mut named = Vec::new();
            for planet in facts["planets"].as_array().unwrap() {
                for place in planet["lands"].as_array().unwrap() {
                    let force = &facts["forces"][usize::try_from(place.as_u64().unwrap()).unwrap()];
                    named.push((planet["planet"].clone(), force["unit"].clone()));
                }
            }
            let offered: Vec<_> = update["pending_choice"]["choice"]["options"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|option| option["kind"] == "commit")
                .map(|option| {
                    (
                        option["payload"]["planet"].clone(),
                        option["payload"]["unit"].clone(),
                    )
                })
                .collect();
            assert!(!offered.is_empty());
            assert_eq!(named, offered);
        }
        drop(seen);

        // A draft at the turn menu plays the same tactical action on a copy: it shows the
        // movement that the game then shows, and the game is as it was.
        let drafted = Arc::new(Mutex::new((0, Vec::<serde_json::Value>::new())));
        let sink = drafted.clone();
        set_native_host(move |update| {
            let value: serde_json::Value = serde_json::from_str(update).unwrap();
            let (asked, outcomes) = &mut *sink.lock().unwrap();
            let script = ["pok8imperial", "no"];
            let Some(wanted) = script.get(*asked) else {
                for script in [
                    r#"[]"#,
                    r#"[{"kind":"choose","option_id":"tactical"},
                        {"kind":"choose","option_id":"23"},
                        {"kind":"move","origin":"01","unit":"sol_carrier","damaged":false,
                         "gravity_drive":false,"ionian":false},
                        {"kind":"done_loading"},
                        {"kind":"done_moving"}]"#,
                ] {
                    let outcome = draft_native(&format!(r#"{{"script":{script}}}"#)).unwrap();
                    outcomes.push(serde_json::from_str(&outcome).unwrap());
                }
                assert_eq!(
                    crate::update(),
                    update,
                    "a draft leaves the update as it was"
                );
                return -1;
            };
            *asked += 1;
            let index = options(&value).iter().position(|have| have == wanted);
            i32::try_from(index.unwrap()).unwrap()
        });
        ti4_play(3, 8, 1);
        let (_, outcomes) = &*drafted.lock().unwrap();
        assert_eq!(outcomes[0]["stop"]["kind"], "open");
        assert_eq!(options(&outcomes[0]["update"]), ["tactical"]);
        assert_eq!(
            outcomes[1]["stop"]["kind"], "complete",
            "{}",
            outcomes[1]["stop"]
        );
        assert_eq!(outcomes[1]["consumed"], 5);
        assert_eq!(
            outcomes[1]["movement"]["tactical"],
            live_movement["tactical"]
        );
        assert_eq!(
            outcomes[1]["movement"]["view"]["board"],
            live_movement["view"]["board"]
        );
        let arrived = outcomes[1]["update"]["view"]["board"]["systems"]["23"]["units"]
            .as_array()
            .unwrap();
        assert!(
            arrived
                .iter()
                .any(|unit| unit["unit_type"] == "sol_carrier")
        );

        // With stepping the host is asked before each decision of a random seat. The seats
        // still follow the seed: the same answers reach the same position. An undo there
        // opens the last choice of the host again.
        #[derive(Default)]
        struct Stepped {
            answers: usize,
            pauses: usize,
            undone: bool,
            again: bool,
            reached: String,
        }
        let stepped = Arc::new(Mutex::new(Stepped::default()));
        let sink = stepped.clone();
        ti4_set_stepping(1);
        set_native_host(move |update| {
            let mut stepped = sink.lock().unwrap();
            let value: serde_json::Value = serde_json::from_str(update).unwrap();
            if value.get("pending_choice").is_none() {
                stepped.pauses += 1;
                assert_eq!(value["viewer"]["seat"], "a");
                assert_ne!(value["turn_status"]["seat"], "a");
                if stepped.answers >= 10 && !stepped.undone {
                    assert_eq!(ti4_can_undo(), 1);
                    stepped.undone = true;
                    return UNDO;
                }
                return GO;
            }
            if stepped.undone && !stepped.again {
                // The choice of the last answer, with its number.
                let number = stepped.answers.to_string();
                assert_eq!(value["pending_choice"]["nonce"], number.as_str());
                stepped.answers -= 1;
                stepped.again = true;
            }
            if stepped.answers == ANSWERS {
                update.clone_into(&mut stepped.reached);
                return -1;
            }
            let count = options(&value).len();
            let index = (stepped.answers * 7) % count;
            stepped.answers += 1;
            i32::try_from(index).unwrap()
        });
        ti4_play(3, 8, 1);
        ti4_set_stepping(0);
        let stepped = stepped.lock().unwrap();
        assert!(stepped.pauses > 0);
        assert!(stepped.again, "the undo opened the last choice again");
        assert_eq!(stepped.reached, reached);
        drop(stepped);

        // A scenario starts at the turn menu of seat a, and its board is in the facts.
        let scenarios: serde_json::Value =
            serde_json::from_str(&response(ti4_scenarios()).unwrap()).unwrap();
        assert_eq!(scenarios[0]["id"], ti4_view::scenario::LATE_GAME);
        assert_eq!(scenarios[0]["players"], 8);
        let seen = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let sink = seen.clone();
        set_native_host(move |update| {
            let value: serde_json::Value = serde_json::from_str(update).unwrap();
            let mut seen = sink.lock().unwrap();
            let index = seen
                .is_empty()
                .then(|| options(&value).iter().position(|have| have == "tactical"))
                .flatten();
            seen.push(value);
            index.map_or(-1, |index| i32::try_from(index).unwrap())
        });
        let status = play_scenario_native(ti4_view::scenario::LATE_GAME, 1);
        assert!(response(status).unwrap().contains(LEFT));
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0]["pending_choice"]["nonce"], "1");
        assert_eq!(seen[0]["view"]["round"], 4);
        let activation = &seen[1]["tactical"];
        assert_eq!(activation["kind"], "activation");
        assert_eq!(activation["tactic_tokens"], 7);
        let reach = activation["systems"].as_array().unwrap();
        let in_range = reach.iter().filter(|fact| fact["ships"] != 0).count();
        assert!(in_range >= 20, "{in_range} systems in range");
        drop(seen);

        let status = play_scenario_native("nope", 1);
        let error = response(status).unwrap_err();
        assert!(error.contains("late-game"), "{error}");

        // The host changes its seats during the game: a to c, c to e and at once to b, b back
        // to a. Each time the open decision and the later ones of the old seat are drawn, and
        // the host is asked for the new seat only. An undo stays with the seat.
        const PLAN: [(usize, &str, u32); 4] = [
            (6, "c", 0b100),
            (10, "e", 0b1_0000),
            (10, "b", 0b10),
            (14, "a", 0b1),
        ];
        #[derive(Default)]
        struct Switching {
            answers: Vec<String>,
            switches: Vec<(u32, u32)>,
            seat: &'static str,
            undone: bool,
            reached: String,
        }
        let switching = Arc::new(Mutex::new(Switching {
            seat: "a",
            ..Switching::default()
        }));
        let sink = switching.clone();
        set_native_host(move |update| {
            let mut run = sink.lock().unwrap();
            let value: serde_json::Value = serde_json::from_str(update).unwrap();
            assert_eq!(value["pending_choice"]["choice"]["player"], run.seat);
            assert_eq!(value["viewer"]["seat"], run.seat);
            if let Some((at, seat, mask)) = PLAN.get(run.switches.len()).copied()
                && run.answers.len() == at
            {
                let decision = ti4_decided();
                ti4_switch_seats(decision, mask);
                run.switches.push((decision, mask));
                run.seat = seat;
                return SWITCH;
            }
            let since = PLAN[..run.switches.len()]
                .last()
                .map_or(0, |(at, _, _)| *at);
            // The answers of the seat before are not taken back.
            assert_eq!(
                ti4_can_undo(),
                u32::from(run.answers.len() > since),
                "after {} answers",
                run.answers.len()
            );
            if run.answers.len() == 8 && !run.undone {
                run.undone = true;
                run.answers.pop();
                return UNDO;
            }
            if run.answers.len() == 20 {
                update.clone_into(&mut run.reached);
                return -1;
            }
            let ids = options(&value);
            let index = (run.answers.len() * 7) % ids.len();
            run.answers.push(ids[index].clone());
            i32::try_from(index).unwrap()
        });
        ti4_play(3, 8, 1);
        let run = std::mem::take(&mut *switching.lock().unwrap());
        assert_eq!(run.switches.len(), PLAN.len());
        assert!(run.undone);
        assert!(run.reached.contains("\"nonce\":\"21\""), "{}", run.reached);

        // A saved game names its switches by the number of the decision. Played again with
        // them and the same answers, the game reaches the same position.
        for (decision, mask) in &run.switches {
            ti4_switch_seats(*decision, *mask);
        }
        let again = Arc::new(Mutex::new((0, String::new())));
        let sink = again.clone();
        let answers = run.answers.clone();
        set_native_host(move |update| {
            let (asked, reached) = &mut *sink.lock().unwrap();
            let Some(wanted) = answers.get(*asked) else {
                update.clone_into(reached);
                return -1;
            };
            *asked += 1;
            let ids = options(&serde_json::from_str(update).unwrap());
            let index = ids.iter().position(|id| id == wanted);
            i32::try_from(index.expect("the replayed game offers the saved answer")).unwrap()
        });
        ti4_play(3, 8, 1);
        assert_eq!(again.lock().unwrap().1, run.reached);

        // The Never cards of the host. Seat d of the late game holds Nav Suite; its window
        // opens twice after an activation. Both are passed for the host: each is a choice with
        // a number and an answer, and an undo goes back over them to the activation. A script
        // answers the choice that the game waits in.
        const NAV_SUITE: &str = "Nav Suite";
        let asked_of = |value: &serde_json::Value| {
            let pending = &value["pending_choice"];
            (
                pending["nonce"].as_str().unwrap().to_owned(),
                pending["choice"]["context"]["subtype"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
            )
        };
        let passed = || -> Vec<serde_json::Value> {
            serde_json::from_str(&response(ti4_passed()).unwrap()).unwrap()
        };
        set_reaction_mode_native(NAV_SUITE, ReactionMode::Never).unwrap();
        let seen = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
        let sink = seen.clone();
        set_native_host(move |update| {
            let value: serde_json::Value = serde_json::from_str(update).unwrap();
            let mut seen = sink.lock().unwrap();
            seen.push(asked_of(&value));
            let pick = |id: &str| {
                let index = options(&value).iter().position(|have| have == id);
                i32::try_from(index.unwrap()).unwrap()
            };
            match seen.len() {
                1 => {
                    let script =
                        r#"[{"kind":"choose","option_id":"tactical"},{"kind":"done_moving"}]"#;
                    let answer: serde_json::Value = serde_json::from_str(
                        &plan_native(&format!(r#"{{"script":{script},"next":0,"actor":"d"}}"#))
                            .unwrap(),
                    )
                    .unwrap();
                    assert_eq!(answer["kind"], "answer", "{answer}");
                    assert_eq!(answer["next"], 1);
                    assert_eq!(answer["index"], pick("tactical"));
                    // The next entry is no answer to the turn menu, and no other seat answers.
                    let later: serde_json::Value = serde_json::from_str(
                        &plan_native(&format!(r#"{{"script":{script},"next":1,"actor":"d"}}"#))
                            .unwrap(),
                    )
                    .unwrap();
                    assert_eq!(later["kind"], "interrupted", "{later}");
                    pick("tactical")
                }
                2 | 4 => pick("41"),
                3 => {
                    assert_eq!(passed().len(), 2);
                    assert_eq!(ti4_can_undo(), 1);
                    UNDO
                }
                _ => {
                    let again = passed();
                    assert_eq!(again.len(), 2);
                    assert_eq!(again[0]["cards"], serde_json::json!([NAV_SUITE]));
                    assert_eq!(again[0]["option"]["id"], "decline");
                    -1
                }
            }
        });
        let status = play_scenario_native(ti4_view::scenario::LATE_GAME, 0b1000);
        assert!(response(status).unwrap().contains(LEFT));
        let asked: Vec<(String, String)> = std::mem::take(&mut *seen.lock().unwrap());
        let asked: Vec<(&str, &str)> = asked
            .iter()
            .map(|(nonce, subtype)| (nonce.as_str(), subtype.as_str()))
            .collect();
        assert_eq!(
            asked,
            [
                ("1", "action_menu"),
                ("2", "activate_system"),
                ("5", "movement_step"),
                ("2", "activate_system"),
                ("5", "movement_step"),
            ]
        );

        // A saved game has the answers of the windows. It is asked for them whole, whatever
        // the cards are set to, so it is the same game.
        for never in [true, false] {
            let mode = if never {
                ReactionMode::Never
            } else {
                ReactionMode::Always
            };
            set_reaction_mode_native(NAV_SUITE, mode).unwrap();
            let seen = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
            let sink = seen.clone();
            set_native_host(move |update| {
                let value: serde_json::Value = serde_json::from_str(update).unwrap();
                let mut seen = sink.lock().unwrap();
                seen.push(asked_of(&value));
                let saved = ["tactical", "41", "decline", "decline"];
                saved.get(seen.len() - 1).map_or(-1, |wanted| {
                    let index = options(&value).iter().position(|have| have == wanted);
                    i32::try_from(index.expect("the saved answer is offered")).unwrap()
                })
            });
            ti4_replay(4);
            let status = play_scenario_native(ti4_view::scenario::LATE_GAME, 0b1000);
            assert!(response(status).unwrap().contains(LEFT));
            let seen = seen.lock().unwrap();
            assert_eq!(seen.len(), 5, "{seen:?}");
            assert!(seen[2].1.starts_with("reaction_after"), "{seen:?}");
            assert_eq!(seen[4], ("5".to_owned(), "movement_step".to_owned()));
            assert!(passed().is_empty());
        }
    }
}
