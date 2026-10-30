//! The adapter half of the window dual (package D1; docs/ENGINE_CORE_DRIVER.md): the recorder of the
//! string window's questions and emissions, the lockstep driver of a core window, and the conversion of the
//! core's staged events to the string state.
//!
//! * [`start`] / [`finish`]: while a recording runs, [`note_ask`] (called by `Table::ask*`) and
//!   [`leave_emit`] (called when an outermost `Resolver::emit*` returns) append to it. Questions and emits
//!   that happen inside an emission of the window (a reaction window's decisions, the events an ability causes) are not
//!   the window's own and are left out: the A5 reaction dual (`kernel_sync::shim`) covers them. Costs one
//!   thread-local read per ask and per emit outside a recording.
//! * [`drive`]: runs a core [`Window`] through the [`Driver`], rendering each question, comparing nothing
//!   itself (the harness does), and answering from the recording so both sides take the same path.
//! * [`sustain_commander`]: the worked example's core half (`driver::windows::sustain_commander`).

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use serde_json::Value;
use ti4_engine_core::choice::Choice as CoreChoice;
use ti4_engine_core::driver::windows::sustain_commander::SustainCommander;
use ti4_engine_core::driver::{Driver, DriverError, Status};
use ti4_engine_core::kernel::stage::{AbilityTable, Window};
use ti4_engine_core::kernel::{Kernel, Resolver};
use ti4_engine_core::state::{CoreState, EventRec, Slice, Val};
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::choice::{Choice, ChoiceOption, IllegalChoice};
use crate::core_sync::{Ctx, Footprint, Sources, with_core};
use crate::dual::Why;
use crate::dual::{CoreWindow, Run};
use crate::event::Event;
use crate::generated::IdStr;
use crate::kernel_sync::text::key_text;
use crate::render::{self, Render, Typed};

// --- the recorder -------------------------------------------------------------------------------------

/// One question the string window put at its own level, with the option id it was answered with (`None`: the
/// decider had no answer yet and the window unwound, the replay engine's way of suspending).
#[derive(Debug, Clone)]
pub struct Asked {
    pub choice: Choice,
    pub answer: Option<String>,
    /// Asked inside an emission of the window (a reaction window's pick, an ability's question).
    pub nested: bool,
}

/// The string resolver's configuration when the window's first emission began: what a core resolver needs
/// besides the registry (which the core builds from its ability table) to open the same windows.
#[derive(Debug, Clone)]
pub struct ResolverCfg {
    pub initiative: Vec<String>,
    pub seating: Vec<String>,
    pub active: Option<String>,
    pub speaker: Option<String>,
    pub phase: ti4_model::state::Phase,
    pub round: u64,
    pub turn: u64,
    pub max_depth: usize,
    /// `(ability id, scope Debug text)`
    pub used: Vec<(String, String)>,
    /// The sources in play (the play of an action card reads them).
    pub sources: ti4_model::content_types::SourceSet,
    /// The emissions open around the window's first emission (`(event type, trace id)`, outermost first).
    pub stack: Vec<(String, u64)>,
}

/// One push to `Game::events` as the recorder saw it (package P1.1b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pushed {
    /// A plain event (`Game::emit`, `Live::emit`), the text as pushed: `NAME` or `NAME:argument`.
    Plain(String),
    /// The name of a typed event mirrored from the timing log (`mirror_timing_log`).
    Mirrored(String),
}

impl Pushed {
    /// The text as `Game::events` holds it.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Plain(t) | Self::Mirrored(t) => t,
        }
    }
}

/// What the string window did at its own level.
#[derive(Debug, Clone, Default)]
pub struct Recording {
    pub questions: Vec<Asked>,
    pub emits: Vec<Event>,
    /// Something happened inside an emission of the window (a reaction window's question, an event an
    /// ability caused): the abilities are string-only for now, so a core window cannot follow.
    pub nested: bool,
    /// The event types and the questions that happened inside the emissions of the window (for the bail census).
    pub nested_kinds: Vec<String>,
    /// The action cards announced as played inside the window's emissions (`ACTION_CARD_PLAYED`'s `card`), in order.
    pub played: Vec<String>,
    /// The ids of the abilities whose effect ran inside the window (at any emission depth), in order.
    pub effects: Vec<String>,
    /// The ids of the abilities that were eligible in some window of an emission (picked, offered or taken
    /// as the single mandatory one), in order of first sight. An ability whose condition failed is not here.
    pub eligible: Vec<String>,
    /// The dice abilities rolled inside the window: `(hits_on, faces)` of each roll, in order.
    pub dice: Vec<(Option<u32>, Vec<u32>)>,
    /// Something nested happened that no ability explains (a question or an event inside an emission, outside
    /// an effect and outside a window's pick): the core window has no counterpart to follow.
    pub foreign: bool,
    /// The resolver as the window's first emission found it.
    pub cfg: Option<ResolverCfg>,
    /// The trace id of the first event the window emitted (the core numbers its own from there).
    pub first_event_id: Option<u64>,
    /// The destinations a Skilled Retreat found for `(player, system)`, as the string's map computed them (the core is lent
    /// no galaxy): an input of the card, noted by its effect.
    pub withdraw: Vec<(String, String, Vec<String>)>,
    /// The plain events (`Game::emit`, `Live::emit`) the string pushed to `Game::events` while the window ran, as pushed
    /// (`NAME` or `NAME:argument`), in order (package P1.1).
    pub plain: Vec<String>,
    /// The typed events the string mirrored from the timing log into `Game::events` while the window ran (names only).
    pub mirrored: Vec<String>,
    /// Both lists in ONE sequence, in the order of the pushes to `Game::events` (package P1.1b). The string pushes a plain
    /// event at once but copies the typed events of the resolver in batches (`mirror_timing_log`, when the surrounding call
    /// returns), so a plain event pushed after an emission is stored BEFORE that emission's names: only this sequence shows it.
    /// `plain` and `mirrored` are the two views of it.
    pub order: Vec<Pushed>,
    /// The pairs of systems the string's abilities asked the map about (`near`): `(a, b, adjacent)`; the core is lent no galaxy.
    pub adjacent: Vec<(String, String, bool)>,
    effect_depth: u32,
    in_pick: u32,
    /// The emission depth the window started at: a window may itself run inside an emission (an ability's
    /// effect opens a combat), and its own level is that depth, not zero.
    base: u32,
}

impl Recording {
    /// The nested activity in one line: the event kinds and questions that happened inside the window's
    /// emissions, then the eligible abilities (the owner's faction name left out so seats pool).
    #[must_use]
    pub fn signature(&self) -> String {
        let mut kinds = self.nested_kinds.clone();
        kinds.sort();
        let anon = |id: &String| {
            let mut parts: Vec<&str> = id.split(':').collect();
            if parts.len() > 2 {
                parts[1] = "*";
            }
            parts.join(":")
        };
        let mut abilities: Vec<String> = self.eligible.iter().map(anon).collect();
        abilities.sort();
        abilities.dedup();
        let mut played = self.played.clone();
        played.sort();
        let played = if played.is_empty() { String::new() } else { format!(" | cards {}", played.join(",")) };
        format!("{} | {}{played}", kinds.join(","), abilities.join(","))
    }
}

thread_local! {
    // The unit tests of the windows inside the loop compare them one by one; the composed tests switch it on.
    static COMPOSED: Cell<bool> = const { Cell::new(!cfg!(test)) };
    static REC: RefCell<Option<Recording>> = const { RefCell::new(None) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    /// Every push to `Game::events` of the running `Game::step`, windows or not (the D1 dual's plain events of the finished areas).
    static STEP: RefCell<Option<Vec<Pushed>>> = const { RefCell::new(None) };
    static LABEL: Cell<&'static str> = const { Cell::new("nested") };
}

/// Run the core half of the window `label` (the bails of the shared driver step are counted under it).
pub fn with_label<T>(label: &str, body: impl FnOnce() -> T) -> T {
    // The labels are string literals at the call sites; keep a leaked copy only for the few distinct ones.
    let leaked: &'static str = intern_label(label);
    let before = LABEL.with(|l| l.replace(leaked));
    let out = body();
    LABEL.with(|l| l.set(before));
    out
}

/// The label of the window whose core half is running (`"nested"` outside one).
pub(crate) fn current_label() -> &'static str {
    LABEL.with(Cell::get)
}

fn intern_label(label: &str) -> &'static str {
    use std::sync::Mutex;
    static LABELS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let mut all = LABELS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(found) = all.iter().find(|l| **l == label) {
        return found;
    }
    let leaked: &'static str = Box::leak(label.to_owned().into_boxed_str());
    all.push(leaked);
    leaked
}

/// An ability asked the map whether `a` is next to `b` (`galaxy.adjacent`): an input of Dynamo and the Watcher.
pub fn note_adjacent(a: &str, b: &str, adjacent: bool) {
    REC.with(|r| {
        if let Some(rec) = r.borrow_mut().as_mut() {
            if !rec.adjacent.iter().any(|(x, y, _)| x == a && y == b) {
                rec.adjacent.push((a.to_owned(), b.to_owned(), adjacent));
            }
        }
    });
}

/// Begin a recording; `false` (and nothing changes) when one is running already.
pub fn start() -> bool {
    REC.with(|r| {
        let mut r = r.borrow_mut();
        if r.is_some() {
            return false;
        }
        *r = Some(Recording { base: DEPTH.with(Cell::get), ..Recording::default() });
        true
    })
}

/// Whether the running recording has seen nested activity (a question or an event inside an emission) so far: the
/// string side of a window that compares plain log lines leaves them out then, as the core side does.
#[must_use]
pub fn nested_so_far() -> bool {
    REC.with(|r| r.borrow().as_ref().is_some_and(|rec| rec.nested))
}

/// End the recording and take it.
pub fn finish() -> Recording {
    REC.with(|r| r.borrow_mut().take()).unwrap_or_default()
}

/// Whether the plain-event census counts (some area on `Dual`): a push to `Game::events` costs one load otherwise.
fn plain_census_on() -> bool {
    cfg!(feature = "typed-core") && crate::dual::mask().any_dual()
}

/// A plain event was pushed to `Game::events` (`Game::emit`, `Live::emit`, the two `LEADER_UNLOCKED:` pushes): the window
/// recording keeps it, outside a window the census counts it under `(game)`.
pub fn note_plain_event(text: &str) {
    if !plain_census_on() {
        return;
    }
    STEP.with(|s| {
        if let Some(pushes) = s.borrow_mut().as_mut() {
            pushes.push(Pushed::Plain(text.to_owned()));
        }
    });
    let kept = REC.with(|r| match r.borrow_mut().as_mut() {
        Some(rec) => {
            rec.plain.push(text.to_owned());
            rec.order.push(Pushed::Plain(text.to_owned()));
            true
        }
        None => false,
    });
    if !kept {
        census_plain("(game)", &[text.to_owned()]);
    }
}

/// Typed events were mirrored from the timing log into `Game::events` (`mirror_timing_log`).
pub fn note_mirrored_events(names: &[String]) {
    if !plain_census_on() {
        return;
    }
    REC.with(|r| {
        if let Some(rec) = r.borrow_mut().as_mut() {
            rec.mirrored.extend(names.iter().cloned());
            rec.order.extend(names.iter().cloned().map(Pushed::Mirrored));
        }
    });
}

/// Start recording every push of the step that begins (`Game::step` under the D1 dual).
pub fn step_capture_start() {
    STEP.with(|s| *s.borrow_mut() = Some(Vec::new()));
}

/// End the step's recording and take it.
pub fn step_capture_take() -> Vec<Pushed> {
    STEP.with(|s| s.borrow_mut().take()).unwrap_or_default()
}

/// The name of a plain event text: `LEADER_UNLOCKED:sol_hero` is `LEADER_UNLOCKED`.
#[must_use]
pub fn plain_name(text: &str) -> &str {
    text.split(':').next().unwrap_or(text)
}

/// Count `plain` (texts as pushed) under `label` in the report's census.
pub fn census_plain(label: &str, plain: &[String]) {
    if plain.is_empty() {
        return;
    }
    crate::dual::note_plain(label, plain.iter().map(|t| plain_name(t)));
}

/// A question was put (`Table::ask*`).
pub fn note_ask(choice: &Choice, outcome: &Result<ChoiceOption, IllegalChoice>) {
    let depth = DEPTH.with(Cell::get);
    REC.with(|r| {
        if let Some(rec) = r.borrow_mut().as_mut() {
            let answer = outcome.as_ref().ok().map(|o| o.id.clone());
            if rec.base == depth {
                rec.questions.push(Asked { choice: choice.clone(), answer, nested: false });
            } else if depth > rec.base {
                rec.nested = true;
                rec.nested_kinds.push(format!("ask:{}", choice.prompt));
                if rec.effect_depth == 0 && rec.in_pick == 0 {
                    rec.foreign = true;
                }
                rec.questions.push(Asked { choice: choice.clone(), answer, nested: true });
            }
        }
    });
}

/// An ability's effect runs while this guard lives: whatever is asked or emitted meanwhile is the ability's.
pub struct EffectGuard;

impl EffectGuard {
    #[must_use]
    pub fn enter(ability: &str) -> Self {
        REC.with(|r| {
            if let Some(rec) = r.borrow_mut().as_mut() {
                rec.nested = true;
                rec.effects.push(ability.to_owned());
                rec.effect_depth += 1;
            }
        });
        Self
    }
}

impl Drop for EffectGuard {
    fn drop(&mut self) {
        REC.with(|r| {
            if let Some(rec) = r.borrow_mut().as_mut() {
                rec.effect_depth = rec.effect_depth.saturating_sub(1);
            }
        });
    }
}

/// The cards declared in a window are announced while this guard lives (the close of the window, outside any
/// ability's effect): what is asked or emitted meanwhile belongs to the plays, which the core follows.
pub struct CloseGuard;

impl CloseGuard {
    #[must_use]
    pub fn enter() -> Self {
        REC.with(|r| {
            if let Some(rec) = r.borrow_mut().as_mut() {
                rec.nested = true;
                rec.effect_depth += 1;
            }
        });
        Self
    }
}

impl Drop for CloseGuard {
    fn drop(&mut self) {
        REC.with(|r| {
            if let Some(rec) = r.borrow_mut().as_mut() {
                rec.effect_depth = rec.effect_depth.saturating_sub(1);
            }
        });
    }
}

/// A window's pick question is being asked while this guard lives.
pub struct PickGuard;

impl PickGuard {
    #[must_use]
    pub fn enter() -> Self {
        REC.with(|r| {
            if let Some(rec) = r.borrow_mut().as_mut() {
                rec.in_pick += 1;
            }
        });
        Self
    }
}

impl Drop for PickGuard {
    fn drop(&mut self) {
        REC.with(|r| {
            if let Some(rec) = r.borrow_mut().as_mut() {
                rec.in_pick = rec.in_pick.saturating_sub(1);
            }
        });
    }
}

/// Skilled Retreat found these destinations for `player` leaving `system` (the map is the string's).
pub fn note_withdraw(player: &str, system: &str, destinations: &[ti4_model::id::SystemId]) {
    REC.with(|r| {
        if let Some(rec) = r.borrow_mut().as_mut() {
            rec.withdraw.push((player.to_owned(), system.to_owned(), destinations.iter().map(ToString::to_string).collect()));
        }
    });
}

/// A roll was made (`Dice::roll`): kept when an ability's effect made it.
pub fn note_dice(roll: &crate::dice::Roll) {
    REC.with(|r| {
        if let Some(rec) = r.borrow_mut().as_mut() {
            if rec.effect_depth > 0 {
                rec.dice.push((roll.hits_on, roll.faces.clone()));
            }
        }
    });
}

/// The first emission of the recording reads the resolver's configuration (`make` is called once).
pub fn note_resolver(first_event_id: u64, make: impl FnOnce() -> ResolverCfg) {
    REC.with(|r| {
        if let Some(rec) = r.borrow_mut().as_mut() {
            if rec.cfg.is_none() {
                rec.cfg = Some(make());
                rec.first_event_id = Some(first_event_id);
            }
        }
    });
}

/// The abilities one seat found eligible in one window pass (the resolver's `eligible`).
pub fn note_eligible<'a>(abilities: impl IntoIterator<Item = &'a str>) {
    REC.with(|r| {
        if let Some(rec) = r.borrow_mut().as_mut() {
            for id in abilities {
                if !rec.eligible.iter().any(|e| e == id) {
                    rec.eligible.push(id.to_owned());
                }
            }
        }
    });
}

/// An emission begins; the token is the depth it begins at.
#[must_use]
pub fn enter_emit() -> u32 {
    DEPTH.with(|d| {
        let before = d.get();
        d.set(before + 1);
        before
    })
}

/// The emission that began at depth `began` ended (`done`: the finished event, `None` on an error). It is
/// the window's own when it began at the recording's base depth.
pub fn leave_emit(began: u32, done: Option<&Event>) {
    DEPTH.with(|d| d.set(began));
    REC.with(|r| {
        if let Some(rec) = r.borrow_mut().as_mut() {
            if began > rec.base {
                rec.nested = true;
                if rec.effect_depth == 0 {
                    rec.foreign = true;
                }
                if let Some(event) = done {
                    rec.nested_kinds.push(event.event_type.clone());
                    if event.event_type == "ACTION_CARD_PLAYED" {
                        rec.played.push(event.text("card").unwrap_or_default().to_owned());
                    }
                }
            } else if rec.base == began
                && let Some(event) = done
            {
                rec.emits.push(event.clone());
            }
        }
    });
}

/// The events with their trace ids zeroed: the two sides number their events independently.
#[must_use]
pub fn normalized(events: &[Event]) -> Vec<Event> {
    events.iter().cloned().map(|mut e| {
        e.id = 0;
        e
    }).collect()
}

// --- events --------------------------------------------------------------------------------------------

/// The string `Event` of a core record (id zeroed). `None` for a payload the conversion does not cover
/// (a nested tally list or an outcome value).
#[must_use]
/// The text of an agenda outcome in an event payload (`Codec::render_outcome`, with the seats of `r`).
fn outcome_text(r: &Render, outcome: ti4_engine_core::keys::Outcome) -> Option<String> {
    use ti4_engine_core::keys::Outcome;
    Some(match outcome {
        Outcome::Empty => String::new(),
        Outcome::For => "for".to_owned(),
        Outcome::Against => "against".to_owned(),
        Outcome::Seat(s) => r.seats.get(usize::from(s.0))?.clone(),
        Outcome::Planet(x) => x.as_str().to_owned(),
        Outcome::StrategyCard(x) => ti4_engine_core::generated::StrategyCard::as_str(x).to_owned(),
        Outcome::Law(x) => x.as_str().to_owned(),
        Outcome::Secret(x) => x.as_str().to_owned(),
    })
}

pub fn event_of(r: &Render, rec: &EventRec) -> Option<Event> {
    // A tally list is converted for the events whose `units` this adapter knows (`SHIP_MOVED`: owner, unit_type, count).
    if !rec.payload.units.is_empty() && rec.kind != ti4_engine_core::state::EventKind::ShipMoved {
        crate::dual::note_bail_here("an event with a unit tally the adapter does not render (only SHIP_MOVED)");
        return None;
    }
    let mut payload = BTreeMap::new();
    if !rec.payload.units.is_empty() {
        let units: Vec<Value> = rec
            .payload
            .units
            .iter()
            .map(|t| Some(serde_json::json!({"owner": r.seats.get(usize::from(t.owner.0))?.clone(), "unit_type": t.unit.as_str(), "count": u64::from(t.count)})))
            .collect::<Option<_>>()?;
        payload.insert("units".to_owned(), Value::Array(units));
    }
    for (key, val) in rec.payload.items.iter() {
        let v: Value = match *val {
            Val::Seat(s) => r.seats.get(usize::from(s.0))?.clone().into(),
            Val::System(x) => x.as_str().into(),
            Val::Planet(x) => x.as_str().into(),
            Val::Unit(x) => x.as_str().into(),
            Val::Action(x) => x.as_str().into(),
            Val::Strategy(x) => ti4_engine_core::generated::StrategyCard::as_str(x).into(),
            Val::Agenda(x) => x.as_str().into(),
            Val::Leader(x) => x.as_str().into(),
            Val::Tech(x) => x.as_str().into(),
            Val::Int(i) => i.into(),
            Val::Flag(b) => b.into(),
            Val::Code(c) => r.code_text(c).into(),
            Val::Outcome(outcome) => outcome_text(r, outcome)?.into(),
        };
        payload.insert(key_text(r, *key), v);
    }
    for (key, list) in &rec.payload.lists {
        let seat = |s: &ti4_engine_core::ids::Seat| Some(Value::from(r.seats.get(usize::from(s.0))?.clone()));
        let v: Value = match list {
            ti4_engine_core::state::List::Seats(seats) => Value::Array(seats.iter().map(seat).collect::<Option<_>>()?),
            ti4_engine_core::state::List::Planets(planets) => Value::Array(planets.iter().map(|p| Value::from(p.as_str())).collect()),
            // The string engine writes a route as one comma-joined text.
            ti4_engine_core::state::List::Systems(systems) => Value::from(systems.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(",")),
            ti4_engine_core::state::List::Units(units) => Value::Array(
                units
                    .iter()
                    .map(|(s, u)| Some(serde_json::json!({"player": seat(s)?, "unit": u.as_str()})))
                    .collect::<Option<_>>()?,
            ),
        };
        payload.insert(key_text(r, *key), v);
    }
    let mut e = Event::new(0, crate::kernel_sync::event_text(r, rec.kind), payload);
    e.cancelled = rec.cancelled;
    Some(e)
}

/// Write the events a core window staged into the string state, the way the string code stages them
/// (`private:#staged:event:*` marks), so the two states compare equal.
pub fn apply_staged(gs: &mut GameState, r: &Render, staged: &[EventRec]) -> Option<()> {
    for rec in staged {
        let e = event_of(r, rec)?;
        crate::supply::stage_event(gs, &e.event_type, &e.payload);
    }
    Some(())
}

// --- the lockstep driver -------------------------------------------------------------------------------

/// How far a core window got.
#[derive(Debug, Clone)]
pub struct Driven {
    pub questions: Vec<CoreChoice>,
    pub rendered: Vec<Choice>,
    pub finished: bool,
}

/// Run `driver` to the end or to the first question the recording has no answer for, answering each
/// question with the option id the string side was answered with.
///
/// # Errors
/// The driver failed, or an answer id does not name an option of the core's question (reported as the
/// first deviation: the harness records it as a question difference by returning what was rendered).
pub fn drive<W: Window>(
    driver: &mut Driver<W>,
    core: &mut CoreState,
    kernel: &Kernel,
    r: &Render,
    gs: &GameState,
    script: &Recording,
) -> Result<Driven, DriverError> {
    drive_with(driver, core, kernel, r, gs, script, &|_, _, _| {})
}

/// [`drive`], letting the caller complete each rendered question with the display facts only the state
/// knows (`Choice.details`), given the core question and the core state it was asked in.
///
/// # Errors
/// As [`drive`].
pub fn drive_with<W: Window>(
    driver: &mut Driver<W>,
    core: &mut CoreState,
    kernel: &Kernel,
    r: &Render,
    gs: &GameState,
    script: &Recording,
    complete: &dyn Fn(&CoreChoice, &CoreState, &mut Choice),
) -> Result<Driven, DriverError> {
    drive_positioned(driver, core, kernel, None, r, gs, script, complete)
}

/// [`drive_with`] for a family that reads the position it is asked in (production: costs, spots): the question is
/// rendered with the core state and `position`'s kernel (`Typed::new_at`).
///
/// # Errors
/// As [`drive`].
#[allow(clippy::too_many_arguments)]
pub fn drive_positioned<W: Window>(
    driver: &mut Driver<W>,
    core: &mut CoreState,
    kernel: &Kernel,
    position: Option<&'static Kernel>,
    r: &Render,
    gs: &GameState,
    script: &Recording,
    complete: &dyn Fn(&CoreChoice, &CoreState, &mut Choice),
) -> Result<Driven, DriverError> {
    let (phase, round) = (render::generic::core_phase(gs.phase), gs.round);
    let mut out = Driven { questions: Vec::new(), rendered: Vec::new(), finished: false };
    match crate::reactions_core::follow(r, gs, script) {
        Ok(Some(mut fol)) => {
            fol.arm(r, gs, core);
            driver.resolver = fol.resolver;
            driver.table = fol.table;
            if let Some(id) = script.first_event_id {
                driver.next_id = id;
            }
        }
        Ok(None) => {}
        Err(why) => {
            crate::dual::note_bail(LABEL.with(Cell::get), &format!("nested ({why})"));
            return Err(DriverError::NotAsking);
        }
    }
    // The core draws an ability's die from the game's stream; a window that was lent none cannot reproduce the roll.
    if script.nested && !script.dice.is_empty() && !driver.is_seeded() {
        crate::dual::note_bail(LABEL.with(Cell::get), "nested (dice rolled by an ability in a window lent no stream)");
        return Err(DriverError::NotAsking);
    }
    let mut status = driver.run(core, kernel)?;
    loop {
        if let Some(why) = driver.unknown() {
            let why = match why {
                ti4_engine_core::kernel::ability::Unknown::BriberyGoods => "Bribery's goods (the Keleres agent)",
                ti4_engine_core::kernel::ability::Unknown::CardCost => "a card cost the core does not judge",
                ti4_engine_core::kernel::ability::Unknown::HarnessOrInfiltrate => "Harness Energy / Infiltrate",
                ti4_engine_core::kernel::ability::Unknown::NomadAgentExhaustion => "Nomad Temporal Command Suite (agent exhaustion)",
                ti4_engine_core::kernel::ability::Unknown::ImmediateCardPlay => "an action card played at once",
                ti4_engine_core::kernel::ability::Unknown::UnportedCard => "an action card the core does not play",
                ti4_engine_core::kernel::ability::Unknown::MissingFact => "a fact the adapter did not compute for the window",
                ti4_engine_core::kernel::ability::Unknown::PendingDestructions => "an action card that staged destructions",
                ti4_engine_core::kernel::ability::Unknown::DataSkimmer => "a discard the Data Skimmer could take",
                ti4_engine_core::kernel::ability::Unknown::PlayFacts => "an action card the core does not play",
                ti4_engine_core::kernel::ability::Unknown::ReflectiveShielding => "Reflective Shielding played inside a window (its hits are drained by the sustain step)",
            };
            crate::dual::note_bail(LABEL.with(Cell::get), &format!("nested ({why})"));
            return Err(DriverError::NotAsking);
        }
        match status {
            Status::Done => {
                out.finished = true;
                compare_ability_dice(core, script);
                compare_nested_events(r, driver, script);
                return Ok(out);
            }
            Status::Asking(choice) => {
                let typed = match position {
                    Some(k) => Typed::new_at(r, render::Position { state: std::rc::Rc::new(core.clone()), kernel: k }, choice.clone(), phase, round),
                    None => Typed::new(r, choice.clone(), phase, round),
                };
                out.questions.push(choice.clone());
                let mut rendered = typed.rendered;
                complete(&choice, core, &mut rendered);
                // A question the string builds with no `DecisionContext` (Sling Relay, Integrated Economy) is compared
                // without one: a typed `Choice` always carries its context, and the string adds none to its own.
                if script.questions.get(out.questions.len() - 1).is_some_and(|q| q.choice.context.is_none()) {
                    rendered.context = None;
                }
                if let Some(facts) = driver.nested_facts() {
                    // The question was put in a state the driver has since rolled back: its display facts read that one.
                    let at = driver.nested_state().unwrap_or(core);
                    complete_nested(r, &choice, facts, gs, at, &mut rendered);
                    // The previews of a reaction window ("what each single-card slot would do now") are display facts
                    // the string builds from its position (`reaction_preview`); the core renders none, so they are the
                    // recorded question's own (the same convention as the option previews, B1g-10).
                    if let (Some(ctx), Some(recorded)) = (rendered.context.as_mut(), script.questions.get(out.questions.len() - 1)) {
                        ctx.previews = recorded.choice.context.as_ref().map(|c| c.previews.clone()).unwrap_or_default();
                    }
                }
                out.rendered.push(rendered);
                let Some(id) = script.questions.get(out.questions.len() - 1).and_then(|q| q.answer.clone()) else {
                    return Ok(out);
                };
                // An id the family cannot read back without the position (a sustain names a site, not
                // the unit type) is matched against the rendered options of this very question.
                let pick = render::parse_pick(r, &choice.ask, &id).or_else(|| {
                    choice
                        .opts
                        .iter()
                        .find(|o| render::option(r, &choice.ask, o).id == id)
                        .map(|o| ti4_engine_core::choice::Pick::Opt(*o))
                });
                let Some(pick) = pick else {
                    return Ok(out);
                };
                status = driver.resume(core, kernel, pick)?;
            }
        }
    }
}

/// Bail (under the label of the window being run) when the nested activity of the string run cannot be followed.
pub(crate) fn nested_check(script: &Recording) -> Option<()> {
    if !script.nested {
        return Some(());
    }
    match script.unported(&crate::reactions_core::table()) {
        Some(why) => {
            crate::dual::note_bail(LABEL.with(Cell::get), &format!("nested ({why})"));
            None
        }
        None if script.cfg.is_none() => {
            crate::dual::note_bail(LABEL.with(Cell::get), "nested (nested activity without a resolver)");
            None
        }
        None => Some(()),
    }
}

/// The events the abilities emitted inside the window (the string's `nested_kinds` without the questions) against
/// the core's journal without the window's own emissions: the same kinds, in any order of equal kinds.
fn compare_nested_events<W: Window>(r: &Render, driver: &Driver<W>, script: &Recording) {
    if !script.nested {
        return;
    }
    let mut core: Vec<String> = driver.resolver.applied_events().iter().map(|e| crate::kernel_sync::event_text(r, e.kind)).collect();
    for own in &driver.emitted {
        let text = crate::kernel_sync::event_text(r, own.kind);
        if let Some(at) = core.iter().position(|k| *k == text) {
            core.remove(at);
        }
    }
    core.sort();
    let mut string: Vec<String> = script.nested_kinds.iter().filter(|k| !k.starts_with("ask:")).cloned().collect();
    string.sort();
    crate::dual::compare_value(crate::dual::Area::Driver, "nested.events", &string, &core);
}

/// The dice the abilities of the string run rolled, against the ones the core abilities rolled (the ledger rows
/// of purpose `Ability`): the faces, in order. Equal faces from one stream means equal draws.
fn compare_ability_dice(core: &CoreState, script: &Recording) {
    if !script.nested || (script.dice.is_empty() && !core.loaded.has(Slice::Ledger)) {
        return;
    }
    let mut core_dice: Vec<(Option<u32>, Vec<u32>)> = Vec::new();
    if core.loaded.has(Slice::Ledger) {
        for r in core.ledger().dice.iter() {
            if matches!(r.purpose, ti4_engine_core::state::RollPurpose::Ability(_)) {
                core_dice.push((r.hits_on.map(u32::from), r.faces.iter().map(|f| u32::from(*f)).collect()));
            }
        }
    }
    crate::dual::compare_value(crate::dual::Area::Driver, "nested.ability_dice", &script.dice, &core_dice);
}

/// The footprint of a window that follows nested activity: what the abilities of the table read and write.
#[must_use]
pub(crate) const fn ability_fp() -> Footprint {
    Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
        &[Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Cards, Slice::Pending],
    )
}

/// ... and the streams, when the abilities draw (the window has to be lent them: `Sources::rng`).
const ABILITY_DICE_FP: Footprint = Footprint::new(&[Slice::Rng, Slice::Ledger], &[Slice::Rng, Slice::Ledger]);

/// ... and what the play of an action card touches (the reaction slot's play; the decks for Veto and Insider
/// Information, the agenda for the Riders).
const CARD_PLAY_FP: Footprint = Footprint::new(
    &[Slice::Decks, Slice::Cards, Slice::Agenda, Slice::Marks, Slice::Transient, Slice::Pending, Slice::Strategy],
    &[Slice::Decks, Slice::Cards, Slice::Agenda, Slice::Marks, Slice::Transient, Slice::Pending, Slice::Strategy],
);

/// `base`, widened by what the table's abilities touch when the string run had nested activity.
#[must_use]
pub(crate) fn nested_fp(base: Footprint, script: &Recording) -> Footprint {
    if !script.nested {
        return base;
    }
    let mut fp = base.union(ability_fp());
    if script.effects.iter().any(|e| e.starts_with("reaction:")) {
        fp = fp.union(CARD_PLAY_FP);
    }
    if script.dice.is_empty() { fp } else { fp.union(ABILITY_DICE_FP) }
}

/// The display facts of a question asked inside an emission (a reaction window's pick, an ability's question):
/// the trigger of its window, the `active` detail of a pick, the battle or invasion the position is in. What
/// the string builds from the event in hand and `TimingContext::ask_seeing`.
fn complete_nested(
    r: &Render,
    choice: &CoreChoice,
    facts: &ti4_engine_core::kernel::NestedFacts,
    gs: &GameState,
    core: &ti4_engine_core::state::CoreState,
    rendered: &mut Choice,
) {
    if let Some(ctx) = rendered.context.take() {
        // The window's own pick carries its trigger; a question an ability asks carries one only when the ability
        // puts it there (`with_trigger_of`).
        let ctx = if with_trigger_of(choice) {
            let trigger = crate::kernel_sync::convert::trigger_text(
                r,
                &ti4_engine_core::kernel::trigger::Trigger::from_event(&facts.event, facts.relation, &facts.chain),
            );
            // A decision raised by an agenda being revealed or resolved carries the printed agenda as display-only
            // `details.agenda` (`Table::ask_seeing`), unless the producer attached it already.
            if let Some(agenda) = trigger.agenda.as_deref()
                && !rendered.details.contains_key("agenda")
            {
                rendered.details.insert(
                    "agenda".to_owned(),
                    crate::strategy_cards::agenda_details(ti4_content::ContentStore::embedded(), agenda),
                );
            }
            ctx.with_trigger(trigger)
        } else {
            ctx
        };
        rendered.context = Some(nested_position(choice, ctx, gs));
    }
    crate::render::leaders::complete_round_agent(r, choice, rendered);
    if matches!(choice.ask, ti4_engine_core::choice::Ask::Reaction(ti4_engine_core::choice::reaction::Ask::Legionnaire { .. })) {
        // `strategy_cards::commodity_standing`: display only, the asked seat's commodities, their printed limit and trade goods.
        if let Some(p) = core.players().by_seat(choice.actor) {
            rendered.details.insert(
                "commodities".to_owned(),
                serde_json::json!({"have": p.econ.commodities, "max": ti4_engine_core::rules::strategy::commodity_limit(p.faction), "trade_goods": p.econ.trade_goods}),
            );
        }
    }
    if is_window_pick(choice) {
        slot_options(r, choice, facts, rendered);
    }
    if is_window_pick(choice) && gs.phase == ti4_model::state::Phase::Action {
        if let Some(active) = &gs.active {
            rendered.details.insert(crate::timing::ACTIVE_DETAIL.to_owned(), active.to_string().into());
        }
    }
}

/// Whether a nested question carries the trigger of its window in its context: the window's own pick, and the
/// questions whose string ability builds one (`DecisionTrigger::from_event`).
fn with_trigger_of(choice: &CoreChoice) -> bool {
    use ti4_engine_core::choice::reaction::Ask as RAsk;
    match choice.ask {
        ti4_engine_core::choice::Ask::Reaction(a) => !matches!(a, RAsk::Legionnaire { .. } | RAsk::Cavalry | RAsk::Assimilate | RAsk::Watcher { .. } | RAsk::Salvage),
        ti4_engine_core::choice::Ask::Cards(ti4_engine_core::choice::cards::Ask::PlayCard { .. }) => true,
        _ => false,
    }
}

/// The battle and invasion a nested question is asked in. The string puts a payment an ability asks (`pay_seeing`, the
/// table's own `ask_seeing`) in the position of an invasion only, never of a battle; the other nested questions
/// (`TimingContext::ask_seeing`) name both.
fn nested_position(choice: &CoreChoice, ctx: crate::decision_context::DecisionContext, gs: &GameState) -> crate::decision_context::DecisionContext {
    if matches!(choice.ask, ti4_engine_core::choice::Ask::Production(ti4_engine_core::choice::production::Ask::Pay(_))) {
        ctx.about_invasion(gs)
    } else {
        ctx.about_battle(gs).about_invasion(gs)
    }
}

/// Whether a question is the combat area's own (a casualty to choose), not one a reaction window or an ability asks inside it.
fn is_combat_ask(choice: &CoreChoice) -> bool {
    matches!(choice.ask, ti4_engine_core::choice::Ask::Combat(_))
}

/// Whether a nested question is the window's own pick (the one that carries the `active` detail of the action phase).
fn is_window_pick(choice: &CoreChoice) -> bool {
    matches!(choice.ask, ti4_engine_core::choice::Ask::Reaction(ti4_engine_core::choice::reaction::Ask::Window { .. }))
}

/// The options of a reaction window that are the standing card slot show what the seat could play: the label
/// ("Play X", "Choose an action card…") and the card payload (`reaction_card_options` and `pick_with_context`).
fn slot_options(
    r: &Render,
    choice: &CoreChoice,
    facts: &ti4_engine_core::kernel::NestedFacts,
    rendered: &mut Choice,
) {
    use ti4_engine_core::choice::reaction::{AbilityKind, Opt as ROpt};
    let content = ti4_content::ContentStore::embedded();
    for (i, o) in choice.opts.iter().enumerate() {
        let ti4_engine_core::choice::Opt::Reaction(ROpt::Ability(id)) = o else { continue };
        if id.kind != AbilityKind::Reaction {
            continue;
        }
        let cards = &facts.offered;
        let name_of = |c: &ti4_engine_core::generated::ActionCard| crate::action_cards::name_of(content, &ti4_model::id::ActionCardId::new(c.as_str()));
        let mut names: Vec<String> = Vec::new();
        for c in cards.iter() {
            let n = name_of(c);
            if !names.contains(&n) {
                names.push(n);
            }
        }
        let Some(option) = rendered.options.get_mut(i) else { continue };
        option.payload.insert("card_names".to_owned(), serde_json::json!(names));
        if let Some(first) = cards.first().filter(|f| cards.iter().all(|c| name_of(c) == name_of(f))) {
            let name = name_of(first);
            option.label = format!("Play {name}");
            option.payload.insert("card".to_owned(), first.as_str().into());
            option.payload.insert("card_name".to_owned(), name.into());
        } else if cards.len() > 1 {
            option.label = "Choose an action card…".to_owned();
        }
    }
    let _ = r;
}

/// The tables a core half needs, for one position. `None`: more than 16 owners.
pub(crate) fn tables(gs: &GameState) -> Option<(Ctx, Render)> {
    tables_with(gs, &[])
}

/// [`tables`] with owners the position does not hold yet in the table (the neutral owner of a card that places units for it).
pub(crate) fn tables_with(gs: &GameState, extra: &[&str]) -> Option<(Ctx, Render)> {
    let cx = Ctx::with_extra(gs, extra).ok()?;
    let r = Render::new(cx.owners.ids().to_vec());
    Some((cx, r))
}

/// [`tables`] for the tests of other modules.
#[cfg(test)]
pub(crate) fn tables_for_tests(gs: &GameState) -> Option<(Ctx, Render)> {
    tables(gs)
}

/// One ported state write: `f` on the core state of `gs` over the slices of `fp`, with the seat of `player`
/// when one is named. `None` (nothing changed) when the position is outside what the core holds (more than 16
/// owners, a player that has no seat, a value the codecs refuse). The mutation halves (B5 `explore_write`, B7
/// `leaders_core`, A4 `objectives_write`) lend through here.
pub(crate) fn lend<T>(
    gs: &mut GameState,
    fp: Footprint,
    player: Option<&PlayerId>,
    f: impl FnOnce(&mut CoreState, Option<ti4_engine_core::Seat>) -> T,
) -> Option<T> {
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    let who = match player {
        Some(p) => Some(r.seat(p.as_str()).why(crate::dual::reason::NO_SEAT)?),
        None => None,
    };
    with_core(gs, &mut cx, Sources::default(), fp, |core, _| f(core, who)).ok()
}

/// What a window of the mutation halves cannot follow: a question asked inside a battle or an invasion (its
/// context carries the battle), diplomacy promises a favour keeps, a nested emission.
pub(crate) fn outside(gs: &GameState, script: &Recording) -> bool {
    outside_battle(gs, script) || gs.diplomacy.enabled
}

/// [`outside`] without the diplomacy clause (the structured negotiation window is the diplomacy feature itself).
pub(crate) fn outside_battle(gs: &GameState, script: &Recording) -> bool {
    script.nested || gs.active_space_combat.is_some() || gs.active_invasion.is_some()
}

const SUSTAIN_COMMANDER: Footprint =
    Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Pending], &[Slice::Players]);

/// The core half of `combat::ask_sustain_commander`. `None` when the position is outside what the core
/// holds: more than 16 owners, a value the codecs refuse, or the commander's ability held another way than
/// an unlocked leader (a granted mark, Mahact's Imperia, an "An" promissory note: package B7's
/// `has_commander_ability`).
#[must_use]
pub fn sustain_commander(
    gs: &mut GameState,
    player: &PlayerId,
    string_pays: bool,
    script: &Recording,
) -> Option<CoreWindow<bool>> {
    let unlocked = gs
        .player(player)
        .is_some_and(|p| p.leaders.get(ti4_model::id::text("letnevcommander")) == Some(&ti4_model::state::LeaderStatus::Unlocked));
    if string_pays && !unlocked {
        crate::dual::note_bail_here("the Letnev commander's payment held another way than as an unlocked leader");
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let source = r.code("letnevcommander");
    let before = gs.clone();
    let kernel = Kernel::plain();
    let resolver = Resolver::new(&[seat], Some(seat));
    let mut driver = Driver::new(SustainCommander::new(seat, source), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(SUSTAIN_COMMANDER, script), |core, _| {
        let driven = drive(&mut driver, core, &kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(events), questions: driven.rendered })
}

/// Seats (bit per seat) whose action card draw a faction hook changes: every seat with Yssaril in the game
/// (Scheming), else the seats a draw bonus applies to.
#[must_use]
pub fn draw_hooks(gs: &GameState, r: &Render) -> u16 {
    let content = ti4_content::ContentStore::embedded();
    let yssaril = gs.players.iter().any(|p| p.faction.as_str() == "yssaril");
    let mut hooked = 0;
    for (at, id) in r.seats.iter().enumerate().take(16) {
        let player = PlayerId::new(id.as_str());
        let bonus = [1, 2, 3].iter().any(|n| crate::factions::hooks_economy::action_card_draw_bonus(gs, content, &player, *n) != 0);
        if yssaril || bonus {
            hooked |= 1 << at;
        }
    }
    hooked
}

const AGENDA_EFFECT: Footprint = Footprint::new(
    &[
        Slice::Meta,
        Slice::Seats,
        Slice::Players,
        Slice::Board,
        Slice::Objectives,
        Slice::Decks,
        Slice::Ledger,
        Slice::Agenda,
        Slice::Explore,
        Slice::Marks,
        Slice::Pending,
    ],
    &[Slice::Meta, Slice::Players, Slice::Board, Slice::Objectives, Slice::Decks, Slice::Ledger, Slice::Agenda, Slice::Explore, Slice::Marks],
);

/// [`AGENDA_EFFECT`] with the map: the effects that read the galaxy.
const AGENDA_EFFECT_MAP: Footprint = Footprint::new(
    &[
        Slice::Meta,
        Slice::Seats,
        Slice::Players,
        Slice::Board,
        Slice::Map,
        Slice::Objectives,
        Slice::Decks,
        Slice::Ledger,
        Slice::Agenda,
        Slice::Explore,
        Slice::Marks,
        Slice::Pending,
    ],
    &[Slice::Meta, Slice::Players, Slice::Board, Slice::Objectives, Slice::Decks, Slice::Ledger, Slice::Agenda, Slice::Explore, Slice::Marks],
);

/// What the string agenda effect was called with besides the state: the ballot's voters and the game's sources.
#[derive(Debug, Clone)]
pub struct EffectArgs<'a> {
    pub for_voters: &'a [PlayerId],
    pub against_voters: &'a [PlayerId],
    pub sources: ti4_model::content_types::SourceSet,
    /// The game's map, when the caller has one.
    pub galaxy: Option<&'a ti4_content::galaxy::Galaxy>,
    /// Where the random streams stood before the effect, when the dice are ones the core can reproduce (no test
    /// faces, no pending restore): the effects that roll (Executive Sanctions, Ixthian Artifact) need it.
    pub rolls: Option<&'a crate::rng::RngPositions>,
}

/// What an agenda effect leaves besides the state: whether it resolved, the faces it rolled and where the streams
/// stand (`None`: the effect rolls nothing).
pub type EffectOut = (bool, Vec<u32>, Option<crate::rng::RngPositions>);

/// The core half of `agenda_effects::resolve_with` for the effects the core has a window for
/// (`rules::agenda::effects`). `None` for any other agenda, an outcome that does not parse, or a position the
/// codecs refuse.
#[must_use]
pub fn agenda_effect(gs: &mut GameState, agenda: &str, outcome: &str, a: &EffectArgs<'_>, script: &Recording) -> Option<CoreWindow<EffectOut>> {
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::rules::agenda::effects::Resolve;
    use ti4_engine_core::rules::agenda::table_effects::EffectFacts;
    if script.nested {
        crate::dual::note_bail_here("nested activity in the agenda effect window");
        return None;
    }
    let agenda = Agenda::from_id(agenda).why(crate::dual::reason::UNKNOWN_ID)?;
    // A discard in the action phase can be taken by the Ral Nel breakthrough; the core knows no such rule.
    if gs.phase == ti4_model::state::Phase::Action {
        crate::dual::note_bail_here("an agenda effect in the action phase (a discard the Ral Nel Data Skimmer could take)");
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    // An outcome that does not parse is still fine for the arms that do nothing; every other arm needs it.
    let outcome = cx.keys.parse_outcome(outcome, Some(agenda.data().elects)).unwrap_or(ti4_engine_core::keys::Outcome::Empty);
    let seats = |ids: &[PlayerId]| -> Option<ti4_engine_core::containers::SmallList<ti4_engine_core::Seat, 8>> {
        let seats = ids.iter().map(|p| r.seat(p.as_str())).collect::<Option<Vec<_>>>().why(crate::dual::reason::NO_SEAT)?;
        (seats.len() <= 8).then(|| ti4_engine_core::containers::SmallList::from_slice(&seats))
    };
    let mut mahact_held = [0; 8];
    for (at, held) in mahact_held.iter_mut().enumerate() {
        if let Some(id) = r.seats.get(at) {
            *held = gs.tokens_held_by_mahact(&PlayerId::new(id.as_str()));
        }
    }
    let facts = EffectFacts {
        for_voters: seats(a.for_voters)?,
        against_voters: seats(a.against_voters)?,
        mahact_held,
        scope: crate::core_adapter::scope(a.sources),
        default_scope: crate::core_adapter::scope(ti4_model::content_types::DEFAULT),
        source: r.code("agenda"),
        pool: r.code("reinforcements"),
        has_galaxy: a.galaxy.is_some(),
        defense_act: r.code("defense_act"),
        hooked: draw_hooks(gs, &r),
        drop_unofficial: gs.rules_version >= ti4_model::state::RULES_OFFICIAL_ACTION_DECK,
        blanked: crate::laws::blanked(gs),
    };
    let window = Resolve::of_facts(agenda, outcome, &facts)?;
    if window.draws() && gs.deck_reserve.is_some() {
        crate::dual::note_bail_here("deck reserve");
        return None;
    }
    // The dice: only streams the core can reproduce. (Ral Nel's Data Skimmer takes a discard only in the action phase,
    // which the window declines above.)
    let rolls = if window.rolls() {
        let Some(before) = a.rolls else {
            crate::dual::note_bail("agenda_effects::resolve_with", "irreproducible dice (test faces or a stream restore)");
            return None;
        };
        Some(before)
    } else {
        None
    };
    let before = gs.clone();
    let kernel = Kernel::plain();
    let mut driver = Driver::new(window, Resolver::new(&[], None), AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let mut footprint = if driver.window.needs_map() { AGENDA_EFFECT_MAP } else { AGENDA_EFFECT };
    if rolls.is_some() {
        footprint = footprint.union(Footprint::new(&[Slice::Rng], &[Slice::Rng]));
    }
    let src = Sources { galaxy: a.galaxy.filter(|_| driver.window.needs_map()), rng: rolls };
    if rolls.is_some() {
        driver = driver.with_seeds(std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?)));
    }
    let mut rng_after = None;
    let driven = with_core(gs, &mut cx, src, footprint, |core, _| {
        let driven = drive(&mut driver, core, &kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        if rolls.is_some() {
            rng_after = Some(core.rng().clone());
        }
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    if driver.window.unsupported() {
        crate::dual::note_bail_here("an agenda effect the core window does not carry (a rider or draw it does not pay)");
        return None;
    }
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let resolved = driver.window.done() == ti4_engine_core::rules::agenda::effects::Done::Resolved;
    let positions = match rng_after {
        Some(after) => Some(crate::core_sync::codec::rng::positions(&cx, &after).ok()?),
        // An effect that rolls nothing (the other half of the card) leaves the streams where they were.
        None => a.rolls.cloned(),
    };
    Some(CoreWindow { run: Run::new((resolved, driver.window.faces().to_vec(), positions)), questions: driven.rendered })
}

const OFFER_DISCARD: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Agenda, Slice::Pending], &[Slice::Agenda]);

/// The core half of `laws::offer_discard`: the owner's question and the discard. `None` for a law the core does
/// not know, one nobody holds (or a blanked table: the string asks nothing then), or a position the codecs
/// refuse. The display facts under the question (`details`: the card, the facts and the captions) are the
/// callers' text and are taken from the recorded question.
#[must_use]
pub fn offer_discard(gs: &mut GameState, alias: &str, prompt: &str, script: &Recording) -> Option<CoreWindow<bool>> {
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::rules::agenda::offer_discard::OfferDiscard;
    if script.nested {
        crate::dual::note_bail_here("nested activity in the discard offer");
        return None;
    }
    let law = Agenda::from_id(alias).why(crate::dual::reason::UNKNOWN_ID)?;
    // A law nobody holds, or every law blanked for the turn (Law's Order), is no offer: the string asks nothing and changes
    // nothing, so the window that is compared is the one that opens no question (package W1: 532 of 552 calls of the golden
    // games took this path and were bailed before).
    let Some(owner_id) = crate::laws::elected(gs, alias).map(|o| PlayerId::new(o.clone())).filter(|o| gs.player(o).is_some()) else {
        return Some(CoreWindow { run: Run::new(false), questions: Vec::new() });
    };
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    let owner = r.seat(owner_id.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let window = OfferDiscard::new(law, owner, r.code(prompt), r.code(alias));
    let before = gs.clone();
    let kernel = Kernel::plain();
    let mut driver = Driver::new(window, Resolver::new(&[owner], Some(owner)), AbilityTable::default(), 1);
    let recorded = script.questions.first().map(|q| q.choice.details.clone());
    let driven = with_core(gs, &mut cx, Sources::default(), OFFER_DISCARD, |core, _| {
        drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, rendered| {
            if let Some(details) = &recorded {
                rendered.details = details.clone();
            }
        })
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    Some(CoreWindow { run: Run::new(driver.window.taken()), questions: driven.rendered })
}

/// The core half of `laws::repeal`. `None` for a law the core does not know or a position the codecs refuse.
#[must_use]
pub fn laws_repeal(gs: &mut GameState, alias: &str, script: &Recording) -> Option<CoreWindow<bool>> {
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::rules::agenda::effects::Resolve;
    if script.nested {
        crate::dual::note_bail_here("nested activity in the repeal");
        return None;
    }
    let law = Agenda::from_id(alias).why(crate::dual::reason::UNKNOWN_ID)?;
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    let before = gs.clone();
    let kernel = Kernel::plain();
    let mut driver = Driver::new(Resolve::repeal(law), Resolver::new(&[], None), AbilityTable::default(), 1);
    let driven = with_core(gs, &mut cx, Sources::default(), AGENDA_EFFECT, |core, _| drive(&mut driver, core, &kernel, &r, &before, script)).why(crate::dual::reason::NOT_LENDABLE)?.why(crate::dual::reason::DRIVER_REFUSED)?;
    Some(CoreWindow { run: Run::new(driver.window.repealed()), questions: driven.rendered })
}

const DEADLY_PLOT: Footprint =
    Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Pending], &[Slice::Board, Slice::Transient]);

/// The core half of `action_cards::deadly_plot` (the effect; the guard is the reaction window's).
#[must_use]
pub fn deadly_plot(gs: &mut GameState, holder: &PlayerId, script: &Recording) -> Option<CoreWindow<()>> {
    use ti4_engine_core::rules::agenda::deadly_plot::DeadlyPlot;
    if script.nested {
        crate::dual::note_bail_here("nested activity in Deadly Plot");
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    let seat = r.seat(holder.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let before = gs.clone();
    let kernel = Kernel::plain();
    let mut driver = Driver::new(DeadlyPlot::new(seat), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
    let driven = with_core(gs, &mut cx, Sources::default(), DEADLY_PLOT, |core, _| drive(&mut driver, core, &kernel, &r, &before, script)).why(crate::dual::reason::NOT_LENDABLE)?.why(crate::dual::reason::DRIVER_REFUSED)?;
    Some(CoreWindow { run: Run::new(()), questions: driven.rendered })
}

const SUSTAIN_OFFER: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Board, Slice::Players, Slice::Transient, Slice::Marks, Slice::Pending],
);

/// What the string `offer_sustain` was called with.
#[derive(Debug, Clone, Copy)]
pub struct SustainArgs<'a> {
    pub player: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub producer: &'a PlayerId,
    pub hits: usize,
    pub ability: bool,
    pub in_combat: bool,
    /// The decline carries `non_fighters_first` (the loss is bound and a non-fighter exists).
    pub non_fighters_first: bool,
    /// The commander's payment is asked (an unlocked Letnev commander is held).
    pub string_pays: bool,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
}

/// The core half of `combat::offer_sustain`. `None` for what the core window does not express: staged
/// planet sustainers, games below `RULES_FACTION_EFFECTS`, a commander held another way than as an
/// unlocked leader, or a position the codecs refuse.
#[must_use]
pub fn sustain_offer(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: SustainArgs<'_>,
    script: &Recording,
) -> Option<CoreWindow<usize>> {
    use ti4_engine_core::rules::combat::sustain_offer::{Offer, SustainOffer};
    if gs.rules_version < ti4_model::state::RULES_FACTION_EFFECTS {
        crate::dual::note_bail_here("a game below the faction effects");
        return None;
    }
    let unlocked = gs
        .player(a.player)
        .is_some_and(|p| p.leaders.get(ti4_model::id::text("letnevcommander")) == Some(&ti4_model::state::LeaderStatus::Unlocked));
    if a.string_pays && !unlocked {
        crate::dual::note_bail_here("the Letnev commander's payment held another way than as an unlocked leader");
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let (player, producer) = (r.seat(a.player.as_str()).why(crate::dual::reason::NO_SEAT)?, r.seat(a.producer.as_str()).why(crate::dual::reason::NO_SEAT)?);
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let source = r.code("letnevcommander");
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    let offer = Offer {
        player,
        producer,
        system,
        ability: a.ability,
        in_combat: a.in_combat,
        non_fighters_first: a.non_fighters_first,
        source,
        staging: crate::supply::staging_enabled(&before),
        announce: a.announce,
    };
    let resolver = Resolver::new(&[player], Some(player));
    let mut driver = Driver::new(SustainOffer::new(offer, u16::try_from(a.hits).ok()?), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(SUSTAIN_OFFER, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|c, st, rendered| {
            if let ti4_engine_core::choice::Ask::Combat(ask) = &c.ask {
                rendered.details.insert("sustainers".to_owned(), crate::core_combat::sustainers_detail(&env.kernel, st, ask, c.actor));
            }
        });
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    // `remember_sustain_target` is the core window's own (`sustain_offer::remember_target`, a typed mark).
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(usize::from(driver.window.hits)).with_events_vec(events), questions: driven.rendered })
}

const ASSIGN_HITS: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Board, Slice::Transient, Slice::Marks, Slice::Pending],
);

/// What the string `assign_remaining` was called with.
#[derive(Debug, Clone, Copy)]
pub struct AssignArgs<'a> {
    pub player: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub producer: &'a PlayerId,
    pub hits: usize,
    pub ability: bool,
    pub non_fighters_first: bool,
    pub cause: &'a str,
    pub cause_kind: crate::decision_context::HitCause,
    pub during_space_combat: bool,
    /// `supply::staging_enabled`.
    pub announce_staging: bool,
    /// The resolver carries the timing machinery.
    pub announce: bool,
    /// The player is a neutral owner (its casualty order is the reference card's, string-only).
    pub neutral: bool,
}

/// The core half of `combat::assign_remaining`. `None` for neutral owners, reaction activity inside the
/// loss announcement, or a position the codecs refuse.
#[must_use]
pub fn assign_hits(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: AssignArgs<'_>,
    script: &Recording,
) -> Option<CoreWindow<bool>> {
    use ti4_engine_core::rules::combat::assign::{Assign, AssignHits};
    if a.neutral {
        crate::dual::note_bail_here(crate::dual::reason::NEUTRAL);
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let player = r.seat(a.player.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    let facts = Assign {
        player,
        system,
        ability: a.ability,
        non_fighters_first: a.non_fighters_first,
        subtype: r.code("assign_casualty"),
        cause: r.code(a.cause),
        during_space_combat: a.during_space_combat,
        staging: a.announce_staging,
        announce: a.announce,
    };
    let resolver = Resolver::new(&[player], Some(player));
    let mut driver = Driver::new(AssignHits::new(facts, u16::try_from(a.hits).ok()?), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let producer = a.producer.clone();
    let restriction = if a.non_fighters_first {
        crate::decision_context::HitRestriction::NonFighter
    } else {
        crate::decision_context::HitRestriction::Any
    };
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(ASSIGN_HITS, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|choice, _, rendered| {
            // Only the casualty question describes a hit: a reaction window the loss opens inside it (its pick) does not.
            if !is_combat_ask(choice) {
                return;
            }
            if let Some(context) = rendered.context.take() {
                rendered.context = Some(context.with_hit(crate::decision_context::HitDetail {
                    cause: a.cause_kind,
                    destroy: false,
                    restriction,
                    producer: Some(producer.clone()),
                }));
            }
        });
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(events), questions: driven.rendered })
}

const COMMIT_GROUND: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Board, Slice::Pending],
);

/// The core half of `invasion::commit_ground_forces`: the planets committed to. `None` for what the core window
/// does not express: a Keleres seat (the Omniopiares toll), a ship-form unit flipping on landing, nested
/// activity, or a position the codecs refuse.
#[must_use]
pub fn commit_ground(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    invader: &PlayerId,
    system: &ti4_model::id::SystemId,
    script: &Recording,
) -> Option<CoreWindow<Vec<ti4_model::id::PlanetId>>> {
    use ti4_engine_core::rules::invasion::commit::CommitGround;
    if gs.players.iter().any(|p| p.faction.as_str().starts_with("keleres")) {
        crate::dual::note_bail_here("a Keleres seat (non-lobby)");
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let seat = r.seat(invader.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, sources).why("the invasion environment could not be built from the position")?;
    let resolver = Resolver::new(&[seat], Some(seat));
    let mut driver = Driver::new(CommitGround::new(seat, sys), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(COMMIT_GROUND, script), |core, _| {
        let driven = drive(&mut driver, core, &env.kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    if driver.window.unsupported {
        crate::dual::note_bail_here("a landing the core commit window does not carry");
        return None;
    }
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let planets = driver.window.committed.iter().map(|p| ti4_model::id::PlanetId::new(p.as_str())).collect();
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(planets).with_events_vec(events), questions: driven.rendered })
}

const ABSORB_GROUND: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Board, Slice::Pending],
);

/// The core half of `invasion::absorb_ground_with_origin`. `None` for a neutral owner, nested activity, or a
/// position the codecs refuse.
#[must_use]
#[allow(clippy::too_many_arguments, reason = "the string function's own")]
pub fn absorb_ground(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    player: &PlayerId,
    system: &ti4_model::id::SystemId,
    planet: &ti4_model::id::PlanetId,
    hits: usize,
    unit_ability: bool,
    script: &Recording,
) -> Option<CoreWindow<()>> {
    use ti4_engine_core::rules::invasion::absorb::AbsorbGround;
    if crate::neutral_units::is_neutral(player) {
        crate::dual::note_bail_here(crate::dual::reason::NEUTRAL);
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let pl = <ti4_engine_core::generated::Planet as IdStr>::from_id(planet.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, sources).why("the invasion environment could not be built from the position")?;
    let resolver = Resolver::new(&[seat], Some(seat));
    let window = AbsorbGround::new(seat, sys, pl, u16::try_from(hits).ok()?, unit_ability, crate::supply::staging_enabled(&before));
    let mut driver = Driver::new(window, resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(ABSORB_GROUND, script), |core, _| {
        let driven = drive(&mut driver, core, &env.kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(()).with_events_vec(events), questions: driven.rendered })
}

const CUSTODIANS: Footprint = Footprint {
    reads: ti4_engine_core::state::SliceSet::of(&[
        Slice::Meta,
        Slice::Seats,
        Slice::Players,
        Slice::Board,
        Slice::Transient,
        Slice::Marks,
        Slice::Agenda,
        Slice::Cards,
        Slice::Pending,
        Slice::Ledger,
        Slice::Explore,
        Slice::Objectives,
    ]),
    writes: ti4_engine_core::state::SliceSet::of(&[Slice::Meta, Slice::Players, Slice::Board, Slice::Ledger, Slice::Pending]),
};

/// The core half of `InvasionWindow::answer_custodians` (27.2, 27.3), recorded as the pending question and its
/// answer (and, for the paid removal, the payment questions after it). The window ends owing the cost; the
/// payment is the production package's `PayStep` window, run here (an area never imports another) until the bill
/// is paid, and then the token comes down. `None` (the bail-outs): nested activity, a Naalu seat (the Matriarch's
/// temporary ground forces are still string-only), a paid removal at a table with a faction the production
/// window does not carry yet (`production_view::unported_window`), and a position the codecs refuse.
#[must_use]
pub fn custodians(
    gs: &mut GameState,
    _sources: ti4_model::content_types::SourceSet,
    invader: &PlayerId,
    system: &ti4_model::id::SystemId,
    affordable: bool,
    script: &Recording,
) -> Option<CoreWindow<()>> {
    use std::rc::Rc;
    use ti4_engine_core::query::economy::Spend as CoreSpend;
    use ti4_engine_core::rules::invasion::stages::{lift, Custodians};
    use ti4_engine_core::rules::production::pay::{PayStep, Stage as PayStage};
    const LABEL: &str = "invasion::answer_custodians";
    if gs.players.iter().any(|p| p.faction.as_str() == "naalu") {
        crate::dual::note_bail(LABEL, "naalu seat");
        return None;
    }
    let paid_yes = script.questions.first().and_then(|q| q.answer.as_deref()) == Some("yes")
        && crate::invasion::custodians_cost(gs, ti4_content::ContentStore::embedded(), invader) != 0;
    if paid_yes && crate::production_view::unported_window(gs, invader) {
        crate::dual::note_bail(LABEL, "paid removal at a table with a faction the production window does not carry");
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let seat = r.seat(invader.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let (phase, round) = (render::generic::core_phase(gs.phase), gs.round);
    let kernel = crate::production_view::kernel();
    let mut driver = Driver::new(Custodians::new(seat, sys, affordable), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
    let mut rendered = Vec::new();
    let mut staged = Vec::new();
    with_core(gs, &mut cx, Sources::default(), nested_fp(CUSTODIANS, script), |core, _| -> Result<(), DriverError> {
        let Status::Asking(choice) = driver.run(core, kernel)? else { return Ok(()) };
        rendered.push(Typed::new(&r, choice.clone(), phase, round).rendered);
        let Some(pick) = script.questions.first().and_then(|q| q.answer.clone()).and_then(|id| render::parse_pick(&r, &choice.ask, &id)) else {
            return Ok(());
        };
        driver.resume(core, kernel, pick)?;
        if let Some(cost) = driver.window.owes {
            let (mut paid, mut next) = (0_i64, 1_usize);
            while paid < cost {
                let step = PayStep { who: seat, kind: CoreSpend::Influence, cost, used_credit: 0, owed: cost, paid, took: None, stage: PayStage::Paying };
                let mut pay = Driver::new(step, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
                if let Status::Asking(q) = pay.run(core, kernel)? {
                    let position = render::Position { state: Rc::new(core.clone()), kernel };
                    rendered.push(Typed::new_at(&r, position, q.clone(), phase, round).rendered);
                    let answer = script.questions.get(next).and_then(|q| q.answer.clone());
                    next += 1;
                    let Some(pick) = answer.and_then(|id| render::parse_pick(&r, &q.ask, &id)) else { return Ok(()) };
                    pay.resume(core, kernel, pick)?;
                }
                match pay.window.took {
                    Some(worth) => paid += worth,
                    None => return Ok(()),
                }
            }
            lift(core, seat);
        }
        staged = std::mem::take(&mut core.pending_mut().staged);
        Ok(())
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(()).with_events_vec(events), questions: rendered })
}

/// The question of the `Fighting` stage as the core `FightRound` window puts it, for `InvasionWindow::
/// ask_fight_round`'s dual. `None`: more than 16 owners or an id the tables do not know.
#[must_use]
pub fn fight_round_ask(
    gs: &GameState,
    sources: ti4_model::content_types::SourceSet,
    invader: &PlayerId,
    system: &ti4_model::id::SystemId,
    planet: &ti4_model::id::PlanetId,
) -> Option<Choice> {
    use ti4_engine_core::rules::invasion::stages::FightRound;
    let (cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    let seat = r.seat(invader.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let pl = <ti4_engine_core::generated::Planet as IdStr>::from_id(planet.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let _ = (&cx, sources);
    let choice = FightRound::new(seat, sys, pl).question();
    let (phase, round) = (render::generic::core_phase(gs.phase), gs.round);
    Some(Typed::new(&r, choice, phase, round).rendered)
}

/// What the string barrage's fighter removal was called with.
/// What the string barrage's fighter removal was called with.
#[derive(Debug, Clone, Copy)]
pub struct FightersArgs<'a> {
    pub target: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub hits: usize,
    pub cause: &'a str,
    pub staging: bool,
    pub announce: bool,
}

/// The core half of the barrage's fighter removal (never asks).
#[must_use]
pub fn destroy_fighters(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: FightersArgs<'_>,
    script: &Recording,
) -> Option<CoreWindow<bool>> {
    use ti4_engine_core::rules::combat::destroy_fighters::{DestroyFighters, Fighters};
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let target = r.seat(a.target.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    let f = Fighters {
        target,
        system,
        hits: u16::try_from(a.hits).ok()?,
        cause: r.code(a.cause),
        staging: a.staging,
        announce: a.announce,
    };
    let resolver = Resolver::new(&[target], Some(target));
    let mut driver = Driver::new(DestroyFighters::new(f), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(ASSIGN_HITS, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(events), questions: driven.rendered })
}

/// What the string `CombatWindow::assault_cannon` was called with.
#[derive(Debug, Clone, Copy)]
pub struct AssaultCannonArgs<'a> {
    pub attacker: &'a PlayerId,
    pub defender: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    /// The resolver carries the timing machinery.
    pub announce: bool,
}

/// The core half of `CombatWindow::assault_cannon`. `None` for a neutral side (the casualty order of the
/// reference card is string-only), reaction activity inside a loss announcement, or a position the codecs
/// refuse.
#[must_use]
pub fn assault_cannon(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: AssaultCannonArgs<'_>,
    script: &Recording,
) -> Option<CoreWindow<bool>> {
    use ti4_engine_core::rules::combat::assault_cannon::{AssaultCannon, Facts};
    if crate::neutral_units::is_neutral(a.attacker) || crate::neutral_units::is_neutral(a.defender) {
        crate::dual::note_bail_here(crate::dual::reason::NEUTRAL);
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let attacker = r.seat(a.attacker.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let defender = r.seat(a.defender.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    let facts = Facts {
        attacker,
        defender,
        system,
        subtype: r.code("assault_cannon_destroy"),
        cause: r.code("technology:assault_cannon"),
        announce: a.announce,
    };
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let mut driver = Driver::new(AssaultCannon::new(facts), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let (att_id, def_id) = (a.attacker.clone(), a.defender.clone());
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(ASSIGN_HITS, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|choice, _, rendered| {
            if !is_combat_ask(choice) {
                return;
            }
            let producer = if choice.actor == attacker { def_id.clone() } else { att_id.clone() };
            if let Some(context) = rendered.context.take() {
                rendered.context = Some(context.with_hit(crate::decision_context::HitDetail {
                    cause: crate::decision_context::HitCause::AssaultCannon,
                    destroy: true,
                    restriction: crate::decision_context::HitRestriction::NonFighter,
                    producer: Some(producer),
                }));
            }
        });
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(events), questions: driven.rendered })
}

/// What the string `CombatWindow::begin_round` was called with.
#[derive(Debug, Clone, Copy)]
pub struct BeginRoundArgs<'a> {
    pub attacker: &'a PlayerId,
    pub defender: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub round: u32,
    /// The resolver carries the timing machinery.
    pub announce: bool,
}

/// Whether the round opening holds something the core window does not carry: diplomacy (attack promises),
/// a Naaz or Letnev side (Voltron repair, Supercharge, Munitions, the Eidolon flip, the self-repairing
/// flagship). A Letnev round agent (own or copied) is an ability of the table (the copy bails when it is eligible).
#[must_use]
pub fn round_opening_unported(
    gs: &GameState,
    content: &ti4_content::ContentStore,
    a: &BeginRoundArgs<'_>,
) -> bool {
    round_opening_why(gs, content, a).is_some()
}

/// [`round_opening_unported`] with the reason.
#[must_use]
pub fn round_opening_why(gs: &GameState, content: &ti4_content::ContentStore, a: &BeginRoundArgs<'_>) -> Option<&'static str> {
    let _ = content;
    if gs.diplomacy.enabled {
        return Some("diplomacy (attack promises)");
    }
    for id in [a.attacker, a.defender] {
        match gs.player(id).map(|p| p.faction.as_str()) {
            None => return Some("a side that is not a seated player"),
            Some("naaz") => return Some("a Naaz side (Voltron repair, Supercharge, the Eidolon flip)"),
            Some("letnev") => return Some("a Letnev side (Munitions, the self-repairing flagship)"),
            Some(_) => {}
        }
    }
    None
}

/// The core half of `CombatWindow::begin_round`. `None` for the positions of [`round_opening_unported`],
/// neutral sides, reaction activity inside the announcements, or a position the codecs refuse.
#[must_use]
pub fn begin_round(
    gs: &mut GameState,
    content: &ti4_content::ContentStore,
    sources: ti4_model::content_types::SourceSet,
    a: BeginRoundArgs<'_>,
    script: &Recording,
) -> Option<CoreWindow<bool>> {
    use ti4_engine_core::rules::combat::begin_round::{BeginRound, Facts};
    if crate::neutral_units::is_neutral(a.attacker) || crate::neutral_units::is_neutral(a.defender) {
        crate::dual::note_bail_here(crate::dual::reason::NEUTRAL);
        return None;
    }
    if let Some(why) = round_opening_why(gs, content, &a) {
        crate::dual::note_bail_here(why);
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let attacker = r.seat(a.attacker.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let defender = r.seat(a.defender.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    let round_seq_listens = crate::factions::borrowed_round_agents::has_round_copy(gs, content, "letnevagent")
        || crate::factions::borrowed_round_agents::has_owner_space_round_agent(gs);
    let facts = Facts { attacker, defender, system, round: a.round, phase: cx.texts.intern(Slice::Transient, "pre_roll").why("the text table of the typed state is full")?, announce: a.announce, round_seq_listens };
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let mut driver = Driver::new(BeginRound::new(facts), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(BEGIN_ROUND, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(events), questions: driven.rendered })
}

/// The environment of a combat window could not be built from the position.
const WINDOW_ENV: &str = "the combat environment could not be built from the position (a value the codecs refuse)";

const BEGIN_ROUND: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Meta, Slice::Transient, Slice::Pending],
);

/// Whether a retreat holds something the core window does not carry: a Last Bastion seat (its retreat note; dropped by
/// decision). Since package W3 the window carries the units that flip between their ship and ground form, a Voltron
/// fighting from its planet and a unit that moves like a ship without being one (Floating Factory).
#[must_use]
pub fn retreat_unported(
    gs: &GameState,
    content: &ti4_content::ContentStore,
    sources: ti4_model::content_types::SourceSet,
    player: &PlayerId,
    system: &ti4_model::id::SystemId,
    destination: &ti4_model::id::SystemId,
) -> bool {
    retreat_why(gs, content, sources, player, system, destination).is_some()
}

/// [`retreat_unported`] with the reason.
#[must_use]
pub fn retreat_why(
    gs: &GameState,
    content: &ti4_content::ContentStore,
    sources: ti4_model::content_types::SourceSet,
    player: &PlayerId,
    system: &ti4_model::id::SystemId,
    destination: &ti4_model::id::SystemId,
) -> Option<&'static str> {
    // The Last Bastion keeps a note of the retreat (`bastion_note_retreat` is not ported: its list is sorted by
    // the text of the unit type, package CA).
    if gs.players.iter().any(|p| p.faction.as_str() == "bastion") {
        return Some("a Last Bastion seat (dropped by decision)");
    }
    // The dual-form units (Eidolon / Z-Grav Eidolon), an Eidolon Maximum on a planet and a Floating Factory are carried by the
    // core retreat window since package W3.
    let _ = (content, sources, player, system, destination);
    None
}

/// What the string `retreat_to` was called with.
#[derive(Debug, Clone, Copy)]
pub struct RetreatArgs<'a> {
    pub player: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub destination: &'a ti4_model::id::SystemId,
}

/// The core half of `combat::retreat_to`; the value is the number of units lost for lack of capacity. `None`
/// for the positions of [`retreat_unported`], neutral players, or a position the codecs refuse.
#[must_use]
pub fn retreat_to(
    gs: &mut GameState,
    content: &ti4_content::ContentStore,
    sources: ti4_model::content_types::SourceSet,
    a: RetreatArgs<'_>,
    script: &Recording,
) -> Option<CoreWindow<usize>> {
    use ti4_engine_core::rules::combat::retreat::{Facts, Retreat};
    if crate::neutral_units::is_neutral(a.player) {
        crate::dual::note_bail_here(crate::dual::reason::NEUTRAL);
        return None;
    }
    if let Some(why) = retreat_why(gs, content, sources, a.player, a.system, a.destination) {
        crate::dual::note_bail_here(why);
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let player = r.seat(a.player.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let destination = <ti4_engine_core::generated::System as IdStr>::from_id(a.destination.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let staging = crate::supply::staging_enabled(gs);
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    let facts = Facts {
        player,
        system,
        destination,
        staging,
        cause: cx.texts.intern(Slice::Transient, "retreat_capacity").why("the text table of the typed state is full")?,
        pool: r.code("reinforcements"),
    };
    let resolver = Resolver::new(&[player], Some(player));
    let mut driver = Driver::new(Retreat::new(facts), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let mut stranded = 0;
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(RETREAT, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        stranded = driver.window.stranded;
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let _ = driven.finished;
    Some(CoreWindow { run: Run::new(stranded).with_events_vec(events), questions: driven.rendered })
}

const RETREAT: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Board, Slice::Transient, Slice::Marks, Slice::Pending],
);

/// What the string `CombatWindow::barrage_step` was called with.
#[derive(Debug, Clone, Copy)]
pub struct BarrageArgs<'a> {
    pub attacker: &'a PlayerId,
    pub defender: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub round: u32,
    pub occurrence: u64,
    /// `supply::staging_enabled`.
    pub staging: bool,
    /// The resolver carries the timing machinery.
    pub announce: bool,
    /// The dice and streams are the ones the core can reproduce (no test faces, no pending restore).
    pub reproducible: bool,
}

/// What the barrage step leaves besides the state: the seats that scored the feat, the dice of the step
/// (hit number, faces) and where the streams stand.
pub type BarrageOut = (Vec<PlayerId>, Vec<(u32, Vec<u32>)>, crate::rng::RngPositions);

/// The seeds of the Dice stream (the only one a window draws from here): `SHA-256(seed || "dice")`.
pub(crate) struct DiceSeeds(pub(crate) String);

/// The streams lent to a window that rolls: where they stood before the string effect.
pub(crate) struct Lent {
    pub(crate) before: crate::rng::RngPositions,
}

impl ti4_engine_core::state::SeedSource for DiceSeeds {
    fn seed_for(&self, seed: u64, domain: ti4_engine_core::state::RngDomain) -> [u8; 32] {
        if domain == ti4_engine_core::state::RngDomain::Dice {
            crate::rng::GameRng::derive_seed(seed, &self.0)
        } else if domain == ti4_engine_core::state::RngDomain::Fracture {
            // Brilliance's roll for the Fracture (`fracture::breakthrough_roll`).
            crate::rng::GameRng::derive_seed(seed, "fracture-breakthrough")
        } else {
            [0; 32]
        }
    }
}

/// Whether the barrage step holds something the core window does not carry: a reroll question (Jol-Nar
/// commander, Heart of Ixth), a Waylay played this round, an Argent side (`afb_excess`).
#[must_use]
pub fn barrage_unported(gs: &GameState, a: &BarrageArgs<'_>) -> bool {
    barrage_why(gs, a).is_some()
}

/// [`barrage_unported`] with the reason.
#[must_use]
pub fn barrage_why(gs: &GameState, a: &BarrageArgs<'_>) -> Option<&'static str> {
    let heart = ti4_model::id::RelicId::new("heartofixth");
    if gs.players.iter().any(|p| crate::relics::holds(gs, &p.id, &heart)) {
        return Some("the Heart of Ixth (Thunder's Edge relic)");
    }
    for id in [a.attacker, a.defender] {
        let Some(p) = gs.player(id) else { return Some("a side that is not a seated player") };
        if p.faction.as_str() == "argent" {
            return Some("an Argent side (the excess hits of the barrage)");
        }
        if p.waylay_barrage_round.is_some_and(|r| r == gs.combat_round_seq) {
            return Some("Waylay played in this barrage");
        }
        if crate::promissory::has_commander_ability(gs, id, "jolnarcommander") {
            return Some("the Jol-Nar commander (a reroll question in the barrage)");
        }
    }
    None
}

/// A Waylay is played in the window a barrage opens before each roll (`ANTI_FIGHTER_BARRAGE_STARTED`), so the position
/// cannot tell: the hits of that roll go to every ship (the target's owner assigns them, sustain first), which the core
/// barrage step does not carry. A documented bail, counted.
fn waylay_check(script: &Recording) -> Option<()> {
    if script.played.iter().any(|card| card == "waylay") {
        crate::dual::note_bail(LABEL.with(Cell::get), "Waylay played in the barrage (the hits go to every ship)");
        return None;
    }
    Some(())
}

/// The core half of `CombatWindow::barrage_step`. `rng_before` is where the streams stood before the string
/// step. `None` for the positions of [`barrage_unported`], neutral sides, irreproducible dice, reaction
/// activity in the announcements, or a position the codecs refuse.
#[must_use]
pub fn barrage_step(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: BarrageArgs<'_>,
    rng_before: &crate::rng::RngPositions,
    script: &Recording,
) -> Option<CoreWindow<BarrageOut>> {
    use ti4_engine_core::rules::combat::barrage::{BarrageStep, Facts};
    if !a.reproducible {
        crate::dual::note_bail_here("irreproducible dice (test faces or a stream restore)");
        return None;
    }
    if crate::neutral_units::is_neutral(a.attacker) || crate::neutral_units::is_neutral(a.defender) {
        crate::dual::note_bail_here(crate::dual::reason::NEUTRAL);
        return None;
    }
    if let Some(why) = barrage_why(gs, &a) {
        crate::dual::note_bail_here(why);
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    waylay_check(script)?;
    let attacker = r.seat(a.attacker.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let defender = r.seat(a.defender.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    let mut unit_codes = Vec::new();
    for p in before.system_state(a.system).units.iter().chain(before.system_state(a.system).planet_units.values().flatten()) {
        let unit = <ti4_engine_core::generated::Unit as IdStr>::from_id(p.type_id.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
        if !unit_codes.iter().any(|(u, _)| *u == unit) {
            unit_codes.push((unit, cx.texts.intern(Slice::Transient, p.type_id.as_str()).why("the text table of the typed state is full")?));
        }
    }
    let facts = Facts {
        attacker,
        defender,
        system,
        round: a.round,
        occurrence: a.occurrence,
        staging: a.staging,
        announce: a.announce,
        phase: cx.texts.intern(Slice::Transient, "barrage").why("the text table of the typed state is full")?,
        cause: r.code("unit_ability:anti_fighter_barrage"),
        kind: r.code("anti_fighter_barrage"),
        extra: cx.texts.intern(Slice::Transient, "extra barrage").why("the text table of the typed state is full")?,
        unit_codes,
    };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let mut driver = Driver::new(BarrageStep::new(facts), resolver, AbilityTable::default(), 1).with_seeds(seeds);
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy: None, rng: Some(rng_before) };
    let driven = with_core(gs, &mut cx, src, nested_fp(BARRAGE_STEP, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        rng_after = Some(core.rng().clone());
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let positions = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let feat = driver.window.feat.iter().map(|s| Some(PlayerId::new(r.seats.get(usize::from(s.0))?.clone()))).collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    let value: BarrageOut = (feat, driver.window.drawn.clone(), positions);
    Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions: driven.rendered })
}

const BARRAGE_STEP: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
    &[Slice::Players, Slice::Board, Slice::Transient, Slice::Pending, Slice::Rng, Slice::Ledger],
);

/// What the string `CombatWindow::roll_round` was called with.
#[derive(Debug, Clone, Copy)]
pub struct RollArgs<'a> {
    pub attacker: &'a PlayerId,
    pub defender: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    /// The dice and streams are the ones the core can reproduce (no test faces, no pending restore).
    pub reproducible: bool,
    /// Munitions Reserves after the roll is offered to this side (rules version and the ability), attacker first.
    pub munitions_after: [bool; 2],
    /// The resolver carries the timing machinery.
    pub announce: bool,
}

/// What the roll step leaves besides the state: the assignment queue (player, hits, producer, non-fighters
/// only, producer assigns), each side's damaged ships (sorted type ids), the step's dice (hit number, faces)
/// and where the streams stand.
pub type RollOut = (
    Vec<(PlayerId, usize, PlayerId, bool, bool)>,
    Vec<(PlayerId, Vec<String>)>,
    Vec<(u32, Vec<u32>)>,
    crate::rng::RngPositions,
);

const ROLL_STEP: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
    &[Slice::Players, Slice::Board, Slice::Transient, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
);

/// The core half of `CombatWindow::roll_round` (see `rules::combat::roll` for what it does not carry; the
/// caller excludes the positions with a reroll question). `None` for neutral sides, irreproducible dice,
/// reaction activity, or a position the codecs refuse.
#[must_use]
pub fn roll_step(
    gs: &mut GameState,
    content: &ti4_content::ContentStore,
    sources: ti4_model::content_types::SourceSet,
    a: RollArgs<'_>,
    rng_before: &crate::rng::RngPositions,
    script: &Recording,
) -> Option<CoreWindow<RollOut>> {
    use ti4_engine_core::rules::combat::roll::{Facts, RollStep};
    if !a.reproducible {
        crate::dual::note_bail_here("irreproducible dice (test faces or a stream restore)");
        return None;
    }
    if crate::neutral_units::is_neutral(a.attacker) || crate::neutral_units::is_neutral(a.defender) {
        crate::dual::note_bail_here(crate::dual::reason::NEUTRAL);
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let attacker = r.seat(a.attacker.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let defender = r.seat(a.defender.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    let mut names = Vec::new();
    let mut texts: Vec<(ti4_engine_core::state::Code, String)> = Vec::new();
    {
        let view = crate::core_combat::CombatView::of(&before, content, sources)?;
        for (seat, id) in [(attacker, a.attacker), (defender, a.defender)] {
            for (value, tag, _, types) in view.fleet_groups(id, a.system)? {
                let mut kinds: Vec<(&str, u32)> = types.iter().map(|(u, n)| (u.as_str(), *n)).collect();
                kinds.sort_unstable();
                let joined = kinds.iter().map(|(u, _)| *u).collect::<Vec<_>>().join("+");
                let state = cx.texts.intern(Slice::Transient, &joined).why("the text table of the typed state is full")?;
                texts.push((state, joined.clone()));
                let types = kinds
                    .iter()
                    .map(|(u, n)| Some((<ti4_engine_core::generated::Unit as IdStr>::from_id(u).why(crate::dual::reason::UNKNOWN_ID)?, *n)))
                    .collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
                names.push(ti4_engine_core::rules::combat::roll::Name { seat, value, tag, state, shown: r.code(&joined), types });
            }
        }
    }
    let facts = Facts {
        attacker,
        defender,
        system,
        phase: cx.texts.intern(Slice::Transient, "resolving_hits").why("the text table of the typed state is full")?,
        names,
        munitions_after: a.munitions_after,
        announce: a.announce,
        cause: r.code("space_combat"),
        hacan: r.code("hacan_flagship"),
    };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let mut driver = Driver::new(RollStep::new(facts), resolver, AbilityTable::default(), 1).with_seeds(seeds);
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy: None, rng: Some(rng_before) };
    let held = |seat: ti4_engine_core::ids::Seat| before.player(&PlayerId::new(r.seats[usize::from(seat.0)].clone())).map_or(0, |p| i64::from(p.trade_goods.max(0)));
    let driven = with_core(gs, &mut cx, src, nested_fp(ROLL_STEP, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|c, st, rendered| {
            roll_details(content, &texts, &held, c, st, rendered);
            if let ti4_engine_core::choice::Ask::Combat(ask @ ti4_engine_core::choice::combat::Ask::CancelHit { .. }) = &c.ask {
                rendered.details.insert("sustainers".to_owned(), crate::core_combat::sustainers_detail(&env.kernel, st, ask, c.actor));
            }
        });
        staged = std::mem::take(&mut core.pending_mut().staged);
        rng_after = Some(core.rng().clone());
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let positions = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let player = |s: ti4_engine_core::ids::Seat| Some(PlayerId::new(r.seats.get(usize::from(s.0))?.clone()));
    let w = &driver.window;
    let queue = w
        .queue
        .iter()
        .map(|q| Some((player(q.player)?, q.hits as usize, player(q.producer)?, q.non_fighters_only, false)))
        .collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    let mut damaged = w
        .damaged
        .iter()
        .map(|(s, kinds)| {
            let mut ids: Vec<String> = kinds.iter().map(|u| u.as_str().to_owned()).collect();
            ids.sort();
            Some((player(*s)?, ids))
        })
        .collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    damaged.sort();
    let value: RollOut = (queue, damaged, w.drawn.clone(), positions);
    Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions: driven.rendered })
}

/// The display facts of a roll step's questions (`Choice.details`): the staged dice, which one is asked
/// about, and Munitions Reserves' printed card and cost.
fn roll_details(
    content: &ti4_content::ContentStore,
    texts: &[(ti4_engine_core::state::Code, String)],
    held: &dyn Fn(ti4_engine_core::ids::Seat) -> i64,
    c: &CoreChoice,
    st: &CoreState,
    rendered: &mut Choice,
) {
    use ti4_engine_core::choice::combat::Ask as CAsk;
    let ti4_engine_core::choice::Ask::Combat(ask) = &c.ask else { return };
    let set = st.transient().combat.reroll_staging.iter().find(|(s, _)| *s == c.actor).map(|(_, set)| set);
    let dice: Vec<serde_json::Value> = set
        .map(|set| {
            set.rolls
                .iter()
                .enumerate()
                .flat_map(|(unit, entry)| {
                    let name = texts.iter().find(|(code, _)| *code == entry.unit).map(|(_, t)| t.clone()).unwrap_or_default();
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
            // The walk asks every die in order: the asked die's place is the number of dice before it.
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
            rendered.details.insert("ability".to_owned(), crate::faction_abilities::ability_card(content, "munitions"));
            rendered.details.insert("cost".to_owned(), serde_json::json!({ "trade_goods": crate::faction_abilities::MUNITIONS_COST, "have": held(c.actor) }));
            rendered.details.insert("dice".to_owned(), serde_json::Value::Array(dice));
            rendered.details.insert("misses".to_owned(), misses.into());
        }
        _ => {}
    }
}

/// A pending hit of the string window (`Pending`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendS {
    pub player: PlayerId,
    pub hits: usize,
    pub producer: PlayerId,
    pub non_fighters_only: bool,
    pub producer_assigns: bool,
}

/// The string window's `Stage` as the flow window sees it (`Done`: a concluded fight).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepS {
    Opening(u32),
    Announcing(u32, PlayerId, Vec<PlayerId>),
    Rolling(u32),
    AfterBarrage(u32),
    Queue(bool, u32, Vec<PendS>),
    Retreating(u32, Vec<PlayerId>),
    Done,
}

/// What one stage transition of the string window was called with: the position and the facts the string
/// rules answer (see `rules::combat::flow::Facts`).
#[derive(Debug, Clone)]
pub struct FlowArgs<'a> {
    pub attacker: &'a PlayerId,
    pub defender: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub announce: bool,
    pub step: StepS,
    pub over: bool,
    pub asking_can_retreat: bool,
    pub leaver_destinations: usize,
    pub front_alive: usize,
    pub offered_before: bool,
    pub sustainers: bool,
    pub pending_retreats: &'a [PlayerId],
    /// 0 round, 1 combat start, 2 round end (`HitPhase`).
    pub phase: u8,
    pub resuming: bool,
}

/// What a transition leaves: the next stage, whether `settle_stages` stops, the hit phase, the resume flag,
/// the retreats announced, and whether the front player's once-per-round announcement was made now.
pub type FlowOut = (StepS, bool, u8, bool, Vec<PlayerId>, bool);

const FLOW_STEP: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Pending],
);

/// The core half of one stage transition (`announcing_step`, `retreating_step`, `hit_queue_head`). `None` for
/// neutral sides, reaction activity in the announcements, or a position the codecs refuse.
#[must_use]
pub fn flow_step(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: &FlowArgs<'_>,
    script: &Recording,
) -> Option<CoreWindow<FlowOut>> {
    use ti4_engine_core::rules::combat::flow::{Facts, Flow, Pend, Phase, Step};
    if crate::neutral_units::is_neutral(a.attacker) || crate::neutral_units::is_neutral(a.defender) {
        crate::dual::note_bail_here(crate::dual::reason::NEUTRAL);
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let seat = |p: &PlayerId| r.seat(p.as_str());
    let seats = |l: &[PlayerId]| l.iter().map(seat).collect::<Option<Vec<_>>>();
    let (attacker, defender) = (seat(a.attacker)?, seat(a.defender)?);
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let step = match &a.step {
        StepS::Opening(n) => Step::Opening { round: *n },
        StepS::Announcing(n, who, done) => Step::Announcing { round: *n, asking: seat(who)?, announced: seats(done)? },
        StepS::Rolling(n) => Step::Rolling { round: *n },
        StepS::AfterBarrage(n) => Step::AfterBarrage { round: *n },
        StepS::Queue(assigning, n, queue) => Step::Queue {
            assigning: *assigning,
            round: *n,
            queue: queue
                .iter()
                .map(|p| Some(Pend { player: seat(&p.player)?, hits: u32::try_from(p.hits).ok()?, producer: seat(&p.producer)?, non_fighters_only: p.non_fighters_only, producer_assigns: p.producer_assigns }))
                .collect::<Option<_>>()?,
        },
        StepS::Retreating(n, leaving) => Step::Retreating { round: *n, leaving: seats(leaving)? },
        StepS::Done => Step::Done,
    };
    let facts = Facts {
        attacker,
        defender,
        system,
        announce: a.announce,
        over: a.over,
        asking_can_retreat: a.asking_can_retreat,
        leaver_destinations: a.leaver_destinations,
        front_alive: a.front_alive,
        offered_before: a.offered_before,
        sustainers: a.sustainers,
        pending_retreats: seats(a.pending_retreats)?,
        phase: match a.phase {
            1 => Phase::CombatStart,
            2 => Phase::RoundEnd,
            _ => Phase::Round,
        },
        resuming: a.resuming,
    };
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let mut driver = Driver::new(Flow::new(facts, step), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(FLOW_STEP, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let player = |s: ti4_engine_core::ids::Seat| Some(PlayerId::new(r.seats.get(usize::from(s.0))?.clone()));
    let players = |l: &[ti4_engine_core::ids::Seat]| l.iter().map(|s| player(*s)).collect::<Option<Vec<_>>>();
    let out = driver.window.out.clone()?;
    let next = match &out.next {
        Step::Opening { round } => StepS::Opening(*round),
        Step::Announcing { round, asking, announced } => StepS::Announcing(*round, player(*asking)?, players(announced)?),
        Step::Rolling { round } => StepS::Rolling(*round),
        Step::AfterBarrage { round } => StepS::AfterBarrage(*round),
        Step::Queue { assigning, round, queue } => StepS::Queue(
            *assigning,
            *round,
            queue
                .iter()
                .map(|p| Some(PendS { player: player(p.player)?, hits: p.hits as usize, producer: player(p.producer)?, non_fighters_only: p.non_fighters_only, producer_assigns: p.producer_assigns }))
                .collect::<Option<_>>()?,
        ),
        Step::Retreating { round, leaving } => StepS::Retreating(*round, players(leaving)?),
        Step::Done => StepS::Done,
    };
    let phase = match out.phase {
        Phase::Round => 0,
        Phase::CombatStart => 1,
        Phase::RoundEnd => 2,
    };
    let value: FlowOut = (next, out.returned, phase, out.resuming, players(&out.pending_retreats)?, out.offered_now);
    Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions: driven.rendered })
}

/// What the string `CombatWindow::announce_end` holds.
#[derive(Debug, Clone, Copy)]
pub struct EndArgs<'a> {
    pub attacker: &'a PlayerId,
    pub defender: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub rounds: u32,
    pub winner: Option<&'a PlayerId>,
    pub declared_draw: bool,
    pub fled: &'a [(PlayerId, Vec<String>)],
    pub at_start: &'a std::collections::BTreeMap<PlayerId, Vec<String>>,
    /// The resolver carries the timing machinery.
    pub announce: bool,
}

/// The core half of `CombatWindow::announce_end`: `SPACE_COMBAT_ENDED` with its lists. `None` for neutral
/// sides, ship types the core has no unit for, reaction activity in the announcement, or a position the
/// codecs refuse (the caller already excludes the flip, the blockaded dock and the faction notes).
#[must_use]
pub fn announce_end(
    gs: &mut GameState,
    _content: &ti4_content::ContentStore,
    sources: ti4_model::content_types::SourceSet,
    a: EndArgs<'_>,
    script: &Recording,
) -> Option<CoreWindow<bool>> {
    use ti4_engine_core::rules::combat::end_announce::{Codes, EndAnnounce, Facts};
    if crate::neutral_units::is_neutral(a.attacker) || crate::neutral_units::is_neutral(a.defender) {
        crate::dual::note_bail_here(crate::dual::reason::NEUTRAL);
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let seat = |p: &PlayerId| r.seat(p.as_str());
    let (attacker, defender) = (seat(a.attacker)?, seat(a.defender)?);
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let units = |kinds: &[String]| -> Option<Vec<ti4_engine_core::generated::Unit>> {
        kinds.iter().map(|k| <ti4_engine_core::generated::Unit as IdStr>::from_id(k)).collect()
    };
    let fled = a.fled.iter().map(|(p, k)| Some((seat(p)?, units(k)?))).collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    let at_start = a.at_start.iter().map(|(p, k)| Some((seat(p)?, units(k)?))).collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    let winner = match a.winner {
        Some(w) => Some(seat(w)?),
        None => None,
    };
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    let codes = Codes {
        losers: r.code("losers"),
        retreated: r.code("retreated"),
        reasons: [r.code("retreat"), r.code("draw"), r.code("destruction"), r.code("round_limit")],
    };
    let facts = Facts { attacker, defender, system, rounds: a.rounds, winner, declared_draw: a.declared_draw, fled, at_start, codes, announce: a.announce };
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let mut driver = Driver::new(EndAnnounce::new(facts), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(BEGIN_ROUND, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(events), questions: driven.rendered })
}

/// What the string Raid Formation hook was called with (`hooks_combat::afb_excess`).
#[derive(Debug, Clone, Copy)]
pub struct RaidArgs<'a> {
    pub producer: &'a PlayerId,
    pub target: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub excess: usize,
}

/// The core half of the barrage's excess hits. `None` unless the producer is Argent (the only module with
/// the hook), for neutral sides, reaction activity, or a position the codecs refuse.
#[must_use]
pub fn raid_formation(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: RaidArgs<'_>,
    script: &Recording,
) -> Option<CoreWindow<bool>> {
    use ti4_engine_core::rules::combat::raid_formation::{Facts, RaidFormation};
    if gs.player(a.producer).is_none_or(|p| p.faction.as_str() != "argent") {
        crate::dual::note_bail_here("a producer that is not an Argent seat");
        return None;
    }
    if crate::neutral_units::is_neutral(a.target) {
        crate::dual::note_bail_here(crate::dual::reason::NEUTRAL);
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let producer = r.seat(a.producer.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let target = r.seat(a.target.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    let facts = Facts { producer, target, system, excess: u16::try_from(a.excess).ok()?, subtype: r.code("raid_formation_damage") };
    let resolver = Resolver::new(&[producer], Some(producer));
    let mut driver = Driver::new(RaidFormation::new(facts), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(ASSIGN_HITS, script), |core, _| {
        // The string asks it in the battle it is fought in (`TimingContext::ask_seeing`): the position names it.
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, rendered| {
            if let Some(context) = rendered.context.take() {
                rendered.context = Some(context.about_battle(&before));
            }
        });
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(events), questions: driven.rendered })
}

const LAND_ONE: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Board, Slice::Transient, Slice::Pending],
);

/// The landing with the Omniopiares toll before it: the payment is the production package's `PayStep`.
const LAND_ONE_TOLL: Footprint = Footprint {
    reads: ti4_engine_core::state::SliceSet::of(&[
        Slice::Meta,
        Slice::Seats,
        Slice::Players,
        Slice::Board,
        Slice::Transient,
        Slice::Marks,
        Slice::Agenda,
        Slice::Cards,
        Slice::Pending,
        Slice::Ledger,
        Slice::Explore,
        Slice::Objectives,
    ]),
    writes: ti4_engine_core::state::SliceSet::of(&[Slice::Meta, Slice::Players, Slice::Board, Slice::Transient, Slice::Ledger, Slice::Pending]),
};

/// The planet a recorded commit answer (`commit|<n>|<planet>`, `deploy_commit|<planet>`) names.
fn answered_planet(script: &Recording) -> Option<ti4_model::id::PlanetId> {
    let id = script.questions.first()?.answer.as_deref()?;
    if !(id.starts_with("commit|") || id.starts_with("deploy_commit|")) {
        return None;
    }
    id.rsplit('|').next().map(ti4_model::id::PlanetId::new)
}

/// The core half of `InvasionWindow::commit_one` (one `commit|<n>|<planet>` landing) and of `commit_deploy` (the
/// Crimson Revenant, `deploy_commit|<planet>`, offered on `deploy`). The Omniopiares toll of a landing is paid
/// through the production package's `PayStep` between the answer and the landing. `None` (the bail-outs): `plain`
/// false (the question holds a commit candidate with an origin or a temporary space commit), nested activity
/// (a reaction to `UNITS_COMMITTED`), the diplomacy promises switched on, a Keleres table where the toll bars a
/// landing (the string question leaves it out), where the toll is owed at a table the production window does not
/// carry (`production_view::unported_window`), a ship-form unit flipping on landing, and a position the codecs
/// refuse.
#[must_use]
#[allow(clippy::too_many_arguments, reason = "the string function's own")]
pub fn land_one(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    invader: &PlayerId,
    system: &ti4_model::id::SystemId,
    plain: Option<&'static str>,
    announce: bool,
    pool: &[PoolIn],
    deploy: &[ti4_model::id::PlanetId],
    script: &Recording,
) -> Option<CoreWindow<LandOut>> {
    use ti4_engine_core::query::economy::Spend as CoreSpend;
    use ti4_engine_core::rules::invasion::commit::LandOne;
    use ti4_engine_core::rules::invasion::Troop;
    use ti4_engine_core::rules::production::pay::{PayStep, Stage as PayStage};
    const LABEL: &str = "invasion::commit_one";
    let keleres = gs.players.iter().any(|p| p.faction.as_str().starts_with("keleres"));
    let toll = if keleres { answered_planet(script).filter(|p| crate::factions::keleres_units::commit_toll_owed(gs, invader, system, p)) } else { None };
    let why = if let Some(why) = plain {
        Some(why)
    } else if gs.diplomacy.enabled {
        Some("diplomacy promises on")
    } else if keleres && landing_barred_by_toll(gs, sources, invader, system) {
        Some("keleres toll bars a landing")
    } else if toll.is_some() && crate::production_view::unported_window(gs, invader) {
        Some("keleres toll at a table the production window does not carry")
    } else {
        None
    };
    if let Some(why) = why {
        crate::dual::note_bail(LABEL, why);
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let seat = r.seat(invader.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, sources).why("the invasion environment could not be built from the position")?;
    let mut troops = Vec::new();
    for entry in pool {
        let origin = match &entry.origin {
            Some((s, p)) => Some((<ti4_engine_core::generated::System as IdStr>::from_id(s.as_str()).why(crate::dual::reason::UNKNOWN_ID)?, <ti4_engine_core::generated::Planet as IdStr>::from_id(p.as_str()).why(crate::dual::reason::UNKNOWN_ID)?)),
            None => None,
        };
        troops.push(Troop { piece: crate::core_sync::codec::board::piece(&cx, &entry.unit).ok()?, origin, temporary: entry.temporary });
    }
    let deploy_planets: Vec<_> = deploy.iter().map(|p| <ti4_engine_core::generated::Planet as IdStr>::from_id(p.as_str())).collect::<Option<_>>().why(crate::dual::reason::UNKNOWN_ID)?;
    let mut driver = Driver::new(LandOne::new(seat, sys, announce).with_pool(troops).with_deploy(deploy_planets), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let rendered = if toll.is_none() {
        let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(LAND_ONE, script), |core, _| {
            let driven = drive(&mut driver, core, &env.kernel, &r, &before, script);
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        driven.rendered
    } else {
        let (phase, round) = (render::generic::core_phase(gs.phase), gs.round);
        let mut rendered = Vec::new();
        with_core(gs, &mut cx, Sources::default(), nested_fp(LAND_ONE_TOLL, script), |core, _| -> Result<(), DriverError> {
            let Status::Asking(choice) = driver.run(core, &env.kernel)? else { return Ok(()) };
            rendered.push(Typed::new(&r, choice.clone(), phase, round).rendered);
            let Some(id) = script.questions.first().and_then(|q| q.answer.clone()) else { return Ok(()) };
            let Some(pick) = render::parse_pick(&r, &choice.ask, &id).or_else(|| {
                choice.opts.iter().find(|o| render::option(&r, &choice.ask, o).id == id).map(|o| ti4_engine_core::choice::Pick::Opt(*o))
            }) else {
                return Ok(());
            };
            let cost = crate::factions::keleres_units::TOLL_COST;
            let (mut paid, mut next) = (0_i64, 1_usize);
            while paid < cost {
                let step = PayStep { who: seat, kind: CoreSpend::Influence, cost, used_credit: 0, owed: cost, paid, took: None, stage: PayStage::Paying };
                let mut pay = Driver::new(step, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
                let pk = crate::production_view::kernel();
                if let Status::Asking(q) = pay.run(core, pk)? {
                    let position = render::Position { state: std::rc::Rc::new(core.clone()), kernel: pk };
                    rendered.push(Typed::new_at(&r, position, q.clone(), phase, round).rendered);
                    let answer = script.questions.get(next).and_then(|q| q.answer.clone());
                    next += 1;
                    let Some(pick) = answer.and_then(|id| render::parse_pick(&r, &q.ask, &id)) else { return Ok(()) };
                    pay.resume(core, pk, pick)?;
                }
                match pay.window.took {
                    Some(worth) => paid += worth,
                    None => return Ok(()),
                }
            }
            driver.resume(core, &env.kernel, pick)?;
            staged = std::mem::take(&mut core.pending_mut().staged);
            Ok(())
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if let Some(planet) = &toll {
            crate::factions::keleres_units::mark_toll_paid(gs, planet);
        }
        rendered
    };
    if driver.window.unsupported {
        crate::dual::note_bail("invasion::commit_one", "ship-form unit flips on landing");
        return None;
    }
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    if driver.window.deployed {
        crate::factions::crimson::note_deployed(gs, invader);
    }
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let landed = driver.window.landed;
    let out = LandOut {
        origin: landed.and_then(|t| t.origin).map(|(s, p)| (s.as_str().to_owned(), p.as_str().to_owned())),
        temporary: landed.is_some_and(|t| t.temporary),
    };
    Some(CoreWindow { run: Run::new(out).with_events_vec(events), questions: rendered })
}

/// Whether the toll the invader cannot pay keeps a landing out of the string question (the core question
/// does not know the toll).
fn landing_barred_by_toll(gs: &GameState, sources: ti4_model::content_types::SourceSet, invader: &PlayerId, system: &ti4_model::id::SystemId) -> bool {
    let content = ti4_content::ContentStore::embedded();
    crate::invasion::landable_planets(gs, content, sources, system)
        .iter()
        .any(|planet| crate::factions::keleres_units::commit_blocked(gs, content, sources, invader, system, planet))
}

/// One entry of the string `commit_pool`: the unit, where it stands when not in the space area, and whether it is
/// a ship committed as a ground force for the moment.
#[derive(Debug, Clone)]
pub struct PoolIn {
    pub unit: ti4_model::units::Unit,
    pub origin: Option<(ti4_model::id::SystemId, ti4_model::id::PlanetId)>,
    pub temporary: bool,
}

/// What a landing records besides the board: the origin pushed to `extra_commits` and whether a temporary space
/// commit was recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandOut {
    pub origin: Option<(String, String)>,
    pub temporary: bool,
}

/// The question of the `ChoosingNextCombat` stage as the core `NextCombat` window puts it, for
/// `InvasionWindow::ask_next_combat`'s dual. `None`: an id the tables do not know.
#[must_use]
pub fn next_combat_ask(
    gs: &GameState,
    invader: &PlayerId,
    system: &ti4_model::id::SystemId,
    planet: &ti4_model::id::PlanetId,
    next: &PlayerId,
) -> Option<Choice> {
    use ti4_engine_core::rules::invasion::stages::NextCombat;
    let (_cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    let (seat, next) = (r.seat(invader.as_str()).why(crate::dual::reason::NO_SEAT)?, r.seat(next.as_str()).why(crate::dual::reason::NO_SEAT)?);
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let pl = <ti4_engine_core::generated::Planet as IdStr>::from_id(planet.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let choice = NextCombat::new(seat, sys, pl, next).question();
    let (phase, round) = (render::generic::core_phase(gs.phase), gs.round);
    Some(Typed::new(&r, choice, phase, round).rendered)
}

/// The question of the `ChoosingBombardment` stage as the core `BombardTarget` window puts it, with the display
/// offer card the string side shares (`invasion::with_bombard_offer`). `None`: the seats are not in id order (the
/// string options are sorted by player id), an id the tables do not know, or a position the codecs refuse.
#[must_use]
pub fn bombard_target_ask(
    gs: &GameState,
    invader: &PlayerId,
    system: &ti4_model::id::SystemId,
    planet: &ti4_model::id::PlanetId,
    hits: usize,
    present: &std::collections::BTreeSet<PlayerId>,
) -> Option<Choice> {
    use ti4_engine_core::rules::invasion::stages::BombardTarget;
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    if r.seats.windows(2).any(|w| w[0] >= w[1]) {
        return None;
    }
    let seat = r.seat(invader.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let pl = <ti4_engine_core::generated::Planet as IdStr>::from_id(planet.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let env = crate::core_invasion::ground_env(gs, &cx, ti4_model::content_types::DEFAULT).why("the invasion environment could not be built from the position")?;
    let mut copy = gs.clone();
    let mut driver = Driver::new(
        BombardTarget::new(seat, sys, pl, u16::try_from(hits).ok()?),
        Resolver::new(&[seat], Some(seat)),
        AbilityTable::default(),
        1,
    );
    let mut asked = None;
    with_core(&mut copy, &mut cx, Sources::default(), LAND_ONE, |core, _| {
        if let Ok(Status::Asking(choice)) = driver.run(core, &env.kernel) {
            asked = Some(choice);
        }
    })
    .ok()?;
    let (phase, round) = (render::generic::core_phase(gs.phase), gs.round);
    let rendered = Typed::new(&r, asked?, phase, round).rendered;
    Some(crate::invasion::with_bombard_offer(rendered, invader, system, planet, hits, present, gs.system_state(system).on_planet(planet)))
}

const GROUND_HITS: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Board, Slice::Players, Slice::Pending],
);

/// What the string hits step was called with: one batch on `victim`'s ground forces on `planet`.
#[derive(Debug, Clone, Copy)]
pub struct HitsArgs<'a> {
    pub victim: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub planet: &'a ti4_model::id::PlanetId,
    pub hits: usize,
    pub cause: &'static str,
    pub unit_ability: bool,
    /// The hits window is part of the call (`ground_hits_window`).
    pub open: bool,
    /// The hits are applied (`take_bombard_hits`, `remove_ground`).
    pub apply: bool,
    pub announce: bool,
}

/// A queued event as the string module keeps it in its `log`.
pub type LogRow = (String, BTreeMap<String, Value>);

/// The outcome of a hits step: the hits that remain after the window, the ground forces destroyed and the
/// events queued for the caller to flush.
#[derive(Debug, Clone, PartialEq)]
pub struct HitsOut {
    pub remaining: usize,
    pub destroyed: usize,
    pub log: Vec<LogRow>,
}

/// The core half of the hits step (`rules::invasion::hits`). `None` for a neutral victim (the reference card's
/// order is string-only), nested activity (a card played in the hits window), or a position the codecs refuse.
#[must_use]
pub fn ground_hits(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: HitsArgs<'_>,
    script: &Recording,
) -> Option<CoreWindow<HitsOut>> {
    use ti4_engine_core::rules::invasion::hits::{Batch, GroundHits};
    const LABEL: &str = "invasion::ground_hits";
    if crate::neutral_units::is_neutral(a.victim) {
        crate::dual::note_bail(LABEL, "neutral victim");
        return None;
    }
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let seat = r.seat(a.victim.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let planet = <ti4_engine_core::generated::Planet as IdStr>::from_id(a.planet.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let cause = r.code(a.cause);
    let hits = u32::try_from(a.hits).ok()?;
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, sources).why("the invasion environment could not be built from the position")?;
    let batch = Batch { victim: seat, system, planet, cause, unit_ability: a.unit_ability, announce: a.announce };
    let mut driver = Driver::new(GroundHits::new(batch, hits, a.open, a.apply), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(GROUND_HITS, script), |core, _| {
        let driven = drive(&mut driver, core, &env.kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    let log = staged
        .iter()
        .map(|rec| event_of(&r, rec).map(|e| (e.event_type, e.payload)))
        .collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let out = HitsOut { remaining: driver.window.remaining as usize, destroyed: driver.window.destroyed as usize, log };
    Some(CoreWindow { run: Run::new(out).with_events_vec(events), questions: driven.rendered })
}

const BOMBARD: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Board, Slice::Players, Slice::Transient, Slice::Pending],
);

/// One planet of the string bombardment plan (`BombardPlan`).
#[derive(Debug, Clone)]
pub struct BombardEntryIn {
    pub planet: ti4_model::id::PlanetId,
    pub before: Vec<ti4_model::units::Unit>,
    pub dice: Vec<ti4_model::state::InvasionDie>,
    pub groups: Vec<usize>,
    pub held: usize,
    pub victims: std::collections::BTreeSet<PlayerId>,
}

/// The string `BombardCursor` (a coexistence pause).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BombardCursorIn {
    pub planet_index: usize,
    pub planet: ti4_model::id::PlanetId,
    pub groups: Vec<usize>,
    pub next_group: usize,
    pub taken: usize,
    pub held: usize,
    pub victims: std::collections::BTreeSet<PlayerId>,
}

/// What the string bombardment step started from.
#[derive(Debug, Clone)]
pub struct BombardIn<'a> {
    pub invader: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub occurrence: u64,
    pub announce: bool,
    pub plan: &'a [BombardEntryIn],
    pub index: usize,
    /// `Some`: the window is in `ChoosingBombardment`; `answering`: the step is the answer to its question.
    pub cursor: Option<&'a BombardCursorIn>,
    pub answering: bool,
}

/// Where the bombardment step ended, as both sides report it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BombardOut {
    pub index: usize,
    /// `bombarding`, `choosing` (the cursor is set) or `done` (the string's `Stage::Custodians`).
    pub stage: &'static str,
    pub cursor: Option<BombardCursorIn>,
    pub kills: usize,
    pub scoring: Vec<(u64, bool)>,
}

/// The core half of the string bombardment step (`settle`'s `Bombarding` and `ChoosingBombardment` arms, or
/// `answer_bombardment_target`): `rules::invasion::bombard`. `None`: neutral or unsorted seats (the options of the
/// target question are sorted by player id), nested activity, a diplomacy game, or a position the codecs refuse.
#[must_use]
pub fn bombard(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: &BombardIn<'_>,
    script: &Recording,
) -> Option<CoreWindow<BombardOut>> {
    use ti4_engine_core::containers::{SeatSet, SmallList};
    use ti4_engine_core::rules::invasion::bombard::{Bombardment, Cursor, Entry};
    const LABEL: &str = "invasion::bombard_step";
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    if a.answering && r.seats.windows(2).any(|w| w[0] >= w[1]) {
        crate::dual::note_bail(LABEL, "seats not in id order");
        return None;
    }
    let seat = |p: &PlayerId| r.seat(p.as_str());
    let seats = |set: &std::collections::BTreeSet<PlayerId>| -> Option<SeatSet> {
        let mut out = SeatSet::EMPTY;
        for p in set {
            out.insert(seat(p)?);
        }
        Some(out)
    };
    let planet_of = |p: &ti4_model::id::PlanetId| <ti4_engine_core::generated::Planet as IdStr>::from_id(p.as_str());
    let invader = seat(a.invader)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let mut plan = Vec::new();
    for e in a.plan {
        let template = ti4_model::state::InvasionStep {
            planet: e.planet.clone(),
            kind: "bombardment".to_owned(),
            round: 0,
            before: e.before.clone(),
            after: Vec::new(),
            dice: e.dice.clone(),
            hits: [(a.invader.clone(), e.groups.iter().sum())].into_iter().collect(),
            harrow_hits: 0,
        };
        plan.push(Entry {
            planet: planet_of(&e.planet)?,
            template: crate::core_sync::codec::transient::invasion_step(&mut cx, &template).ok()?,
            groups: SmallList::from_slice(&e.groups),
            held: e.held,
            victims: seats(&e.victims)?,
        });
    }
    let cause = r.code("bombardment");
    let mut window = Bombardment::new(invader, system, a.occurrence, a.announce, cause, plan, a.index);
    if let Some(c) = a.cursor {
        window = window.at_cursor(
            Cursor {
                planet_index: c.planet_index,
                planet: planet_of(&c.planet)?,
                groups: SmallList::from_slice(&c.groups),
                next_group: c.next_group,
                taken: c.taken,
                held: c.held,
                victims: seats(&c.victims)?,
            },
            a.answering,
        );
    }
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, sources).why("the invasion environment could not be built from the position")?;
    let mut driver = Driver::new(window, Resolver::new(&[invader], Some(invader)), AbilityTable::default(), 1);
    let kernel = &env.kernel;
    let complete = |asked: &CoreChoice, _: &CoreState, c: &mut Choice| {
        // The target question carries the offer card the string side shares (`invasion::with_bombard_offer`).
        use ti4_engine_core::choice::invasion::{Ask as IAsk, Opt as IOpt};
        use ti4_engine_core::choice::{Ask, Opt};
        if let Ask::Invasion(IAsk::BombardTarget { system, planet, hits }) = asked.ask {
            let present: std::collections::BTreeSet<PlayerId> = asked
                .opts
                .iter()
                .filter_map(|o| if let Opt::Invasion(IOpt::BombardTarget { seat }) = o { Some(PlayerId::new(r.seats.get(usize::from(seat.0))?.clone())) } else { None })
                .collect();
            let (system, planet) = (ti4_model::id::SystemId::new(system.as_str()), ti4_model::id::PlanetId::new(planet.as_str()));
            let taken = c.clone();
            *c = crate::invasion::with_bombard_offer(taken, a.invader, &system, &planet, usize::from(hits), &present, before.system_state(&system).on_planet(&planet));
        }
    };
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(BOMBARD, script), |core, _| {
        let driven = drive_with(&mut driver, core, kernel, &r, &before, script, &complete);
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    // What an ability of the table staged inside the step (a Crimson Legionnaire's trade good): the string state holds it as a mark.
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let w = &driver.window;
    let id_of = |s: ti4_engine_core::ids::Seat| PlayerId::new(r.seats.get(usize::from(s.0)).cloned().unwrap_or_default());
    let set_of = |set: SeatSet| set.iter().map(id_of).collect::<std::collections::BTreeSet<_>>();
    let cursor = w.cursor.as_ref().map(|c| BombardCursorIn {
        planet_index: c.planet_index,
        planet: ti4_model::id::PlanetId::new(c.planet.as_str()),
        groups: c.groups.as_slice().to_vec(),
        next_group: c.next_group,
        taken: c.taken,
        held: c.held,
        victims: set_of(c.victims),
    });
    use ti4_engine_core::rules::invasion::bombard::Where;
    let stage = match w.string_stage() {
        Where::Bombarding => "bombarding",
        Where::Choosing => "choosing",
        Where::Done => "done",
    };
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let out = BombardOut { index: w.index, stage, cursor: if stage == "choosing" { cursor } else { None }, kills: w.kills, scoring: w.scoring.clone() };
    Some(CoreWindow { run: Run::new(out).with_events_vec(events), questions: driven.rendered })
}

// --- space cannon defense (B2) -------------------------------------------------------------------------

/// What the string `announce_and_defend` was called with.
#[derive(Debug, Clone, Copy)]
pub struct DefenseArgs<'a> {
    pub invader: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub planets: &'a [ti4_model::id::PlanetId],
    /// The resolver carries the timing machinery.
    pub announce: bool,
    /// The dice and streams are the ones the core can reproduce (no test faces, no pending restore).
    pub reproducible: bool,
}

/// What the defense leaves besides the state: the dice of the window (hit number, faces) and where the streams stand.
pub type DefenseOut = (Vec<(u32, Vec<u32>)>, crate::rng::RngPositions);

const DEFENSE: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
    &[Slice::Players, Slice::Board, Slice::Transient, Slice::Pending, Slice::Rng, Slice::Ledger],
);

/// Why the position holds something the core window does not carry (`None`: it is carried): entropic scars, L4
/// Disruptors, a planet attachment that gives SPACE CANNON, test faces or a pending stream restore, a neutral invader.
#[must_use]
pub fn defense_unported(gs: &GameState, content: &ti4_content::ContentStore, sources: ti4_model::content_types::SourceSet, a: &DefenseArgs<'_>) -> Option<&'static str> {
    if !a.reproducible {
        Some("irreproducible dice (test faces or a stream restore)")
    } else if crate::neutral_units::is_neutral(a.invader) {
        Some("neutral invader")
    } else if !crate::entropic_scars::abilities_usable(content, sources, a.system, Some(a.system)) {
        Some("entropic scar")
    } else if crate::technology::has_technology_text(gs, a.invader, "l4") {
        Some("L4 Disruptors")
    } else if a.planets.iter().any(|p| !crate::planets::attachment_cannons(gs, content, a.system, p).is_empty()) {
        Some("attachment cannon")
    } else {
        None
    }
}

/// The core half of `InvasionWindow::announce_and_defend`. `None` for the positions of [`defense_unported`], nested
/// activity (a reaction to the announcement or to the hits window) and a position the codecs refuse.
#[must_use]
pub fn space_cannon_defense(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: &DefenseArgs<'_>,
    rng_before: &crate::rng::RngPositions,
    script: &Recording,
) -> Option<CoreWindow<DefenseOut>> {
    use ti4_engine_core::rules::invasion::defense::{Facts, SpaceCannonDefense};
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let invader = r.seat(a.invader.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let mut planets = Vec::new();
    for p in a.planets {
        planets.push(<ti4_engine_core::generated::Planet as IdStr>::from_id(p.as_str()).why(crate::dual::reason::UNKNOWN_ID)?);
    }
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, sources).why("the invasion environment could not be built from the position")?;
    let mut unit_codes = Vec::new();
    for planet in a.planets {
        for p in before.system_state(a.system).on_planet(planet) {
            let unit = <ti4_engine_core::generated::Unit as IdStr>::from_id(p.type_id.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
            if !unit_codes.iter().any(|(u, _)| *u == unit) {
                unit_codes.push((unit, cx.texts.intern(Slice::Transient, p.type_id.as_str()).why("the text table of the typed state is full")?));
            }
        }
    }
    let facts = Facts {
        invader,
        system,
        planets,
        announce: a.announce,
        cause: r.code("space_cannon_defense"),
        kind: cx.texts.intern(Slice::Transient, "space_cannon_defense").why("the text table of the typed state is full")?,
        planets_key: r.code("planets"),
        unit_codes,
    };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let resolver = Resolver::new(&[invader], Some(invader));
    let mut driver = Driver::new(SpaceCannonDefense::new(facts), resolver, AbilityTable::default(), 1).with_seeds(seeds);
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy: None, rng: Some(rng_before) };
    let driven = with_core(gs, &mut cx, src, nested_fp(DEFENSE, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        rng_after = Some(core.rng().clone());
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let positions = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let value: DefenseOut = (driver.window.drawn.clone(), positions);
    Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions: driven.rendered })
}

// --- the ground round's dice (B2) ----------------------------------------------------------------------

/// What the string `roll_ground_pair` was called with.
#[derive(Debug, Clone, Copy)]
pub struct GroundRollArgs<'a> {
    pub attacker: &'a PlayerId,
    pub defender: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub planet: &'a ti4_model::id::PlanetId,
    pub sources: ti4_model::content_types::SourceSet,
    /// The dice and streams are the ones the core can reproduce (no test faces, no pending restore).
    pub reproducible: bool,
}

/// What the ground roll leaves besides the state: both sides' hits, the dice (hit number, faces) and the streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundRollOut {
    pub hits: (usize, usize),
    pub drawn: Vec<(u32, Vec<u32>)>,
    pub rng: crate::rng::RngPositions,
}

const GROUND_ROLL: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
    &[Slice::Transient, Slice::Pending, Slice::Rng, Slice::Ledger],
);

/// Why the position holds something the core window does not carry: test faces or a stream restore, a neutral
/// side, X-89 Bacterial Weapon (the doubled hits).
#[must_use]
pub fn ground_roll_unported(gs: &GameState, a: &GroundRollArgs<'_>) -> Option<&'static str> {
    if !a.reproducible {
        Some("irreproducible dice (test faces or a stream restore)")
    } else if crate::neutral_units::is_neutral(a.attacker) || crate::neutral_units::is_neutral(a.defender) {
        Some("neutral side")
    } else if [a.attacker, a.defender].into_iter().any(|id| gs.player(id).is_some_and(|p| p.technologies.contains(ti4_model::id::text(&"x89c4")))) {
        Some("X-89 Bacterial Weapon")
    } else {
        None
    }
}

/// The core half of `InvasionWindow::roll_ground_pair`. `None` for the positions of [`ground_roll_unported`], a pool
/// the window cannot name, nested activity and a position the codecs refuse.
#[must_use]
pub fn ground_roll(gs: &mut GameState, a: &GroundRollArgs<'_>, rng_before: &crate::rng::RngPositions, script: &Recording) -> Option<CoreWindow<GroundRollOut>> {
    use ti4_engine_core::rules::invasion::roll::{Facts, GroundRoll, NAMED};
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let attacker = r.seat(a.attacker.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let defender = r.seat(a.defender.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let planet = <ti4_engine_core::generated::Planet as IdStr>::from_id(a.planet.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, a.sources).why("the invasion environment could not be built from the position")?;
    let mut unit_codes = Vec::new();
    for p in before.system_state(a.system).on_planet(a.planet) {
        let unit = <ti4_engine_core::generated::Unit as IdStr>::from_id(p.type_id.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
        if !unit_codes.iter().any(|(u, _)| *u == unit) {
            unit_codes.push((unit, cx.texts.intern(Slice::Transient, p.type_id.as_str()).why("the text table of the typed state is full")?));
        }
    }
    let mut names = Vec::new();
    for n in 1..=NAMED {
        names.push(cx.texts.intern(Slice::Transient, &format!("combat value {n}")).why("the text table of the typed state is full")?);
    }
    let facts = Facts { attacker, defender, system, planet, kind: cx.texts.intern(Slice::Transient, "ground").why("the text table of the typed state is full")?, names, unit_codes };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let mut driver = Driver::new(GroundRoll::new(facts), Resolver::new(&[attacker, defender], Some(attacker)), AbilityTable::default(), 1).with_seeds(seeds);
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy: None, rng: Some(rng_before) };
    let driven = with_core(gs, &mut cx, src, nested_fp(GROUND_ROLL, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        rng_after = Some(core.rng().clone());
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    if driver.window.unsupported {
        crate::dual::note_bail("invasion::roll_ground", "a pool the window cannot name");
        return None;
    }
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let positions = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let (ah, dh) = driver.window.hits;
    let out = GroundRollOut { hits: (ah as usize, dh as usize), drawn: driver.window.drawn.clone(), rng: positions };
    Some(CoreWindow { run: Run::new(out).with_events_vec(events), questions: driven.rendered })
}

// --- the hits of a ground round (B2) -------------------------------------------------------------------

/// What the string `ground_round_hits` was called with.
#[derive(Debug, Clone, Copy)]
pub struct RoundHitsArgs<'a> {
    pub invader: &'a PlayerId,
    pub defender: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub planet: &'a ti4_model::id::PlanetId,
    /// The final rolled hits of the invader and the defender.
    pub rolled: (usize, usize),
    pub announce: bool,
    /// The units on the planet at the start of the round and the dice as the board shows them.
    pub before: &'a [ti4_model::units::Unit],
    pub dice: &'a [ti4_model::state::InvasionDie],
    /// The invader holds the Harrow ability (its bombardment dice follow the removals).
    pub harrow: bool,
    /// The dice and streams are the ones the core can reproduce (no test faces, no pending restore).
    pub reproducible: bool,
    /// Where the streams stood before the round's hits.
    pub rng_before: &'a crate::rng::RngPositions,
}

/// What the round's hits leave besides the state: the hits that landed, then the Harrow dice (hit number, faces)
/// and where the streams stand.
pub type RoundOut = ((bool, (usize, usize)), DefenseOut);

const ROUND_HITS: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Board, Slice::Players, Slice::Transient, Slice::Pending],
);

const ROUND_HITS_HARROW: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
    &[Slice::Board, Slice::Players, Slice::Transient, Slice::Pending, Slice::Rng, Slice::Ledger],
);

/// Why the position holds something the core window does not carry: a faction's added hits, L1Z1X's Harrow, Alarum
/// (a Ral Nel mech on the planet), a neutral side.
#[must_use]
pub fn round_hits_unported(gs: &GameState, content: &ti4_content::ContentStore, sources: ti4_model::content_types::SourceSet, a: &RoundHitsArgs<'_>) -> Option<&'static str> {
    if crate::neutral_units::is_neutral(a.invader) || crate::neutral_units::is_neutral(a.defender) {
        return Some("neutral side");
    }
    if a.harrow && !a.reproducible {
        return Some("irreproducible dice (test faces or a stream restore)");
    }
    if a.harrow && crate::factions::bastion_units::watches_ground_round(gs, &[a.invader, a.defender]) {
        return Some("Proxima Targeting VI");
    }
    if crate::factions::ralnel::alarum_on(gs, a.system, a.planet) {
        return Some("Alarum");
    }
    let extra = |who: &PlayerId, hits: usize, other: usize| crate::factions::hooks_ground::ground_rolls_extra_hits(gs, content, sources, who, a.system, a.planet, hits, other);
    if extra(a.invader, a.rolled.0, a.rolled.1) != 0 || extra(a.defender, a.rolled.1, a.rolled.0) != 0 {
        return Some("hits added to a roll");
    }
    None
}

/// The core half of `InvasionWindow::ground_round_hits`. `None` for the positions of [`round_hits_unported`], nested
/// activity (a card played in a hits window) and a position the codecs refuse.
#[must_use]
pub fn round_hits(gs: &mut GameState, sources: ti4_model::content_types::SourceSet, a: &RoundHitsArgs<'_>, script: &Recording) -> Option<CoreWindow<RoundOut>> {
    use ti4_engine_core::rules::invasion::round::{Facts, GroundRoundHits};
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let invader = r.seat(a.invader.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let defender = r.seat(a.defender.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let planet = <ti4_engine_core::generated::Planet as IdStr>::from_id(a.planet.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let template = ti4_model::state::InvasionStep {
        planet: a.planet.clone(),
        kind: "ground_round".to_owned(),
        round: 0,
        before: a.before.to_vec(),
        after: Vec::new(),
        dice: a.dice.to_vec(),
        hits: std::collections::BTreeMap::new(),
        harrow_hits: 0,
    };
    let template = crate::core_sync::codec::transient::invasion_step(&mut cx, &template).ok()?;
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, sources).why("the invasion environment could not be built from the position")?;
    let facts = Facts {
        invader,
        defender,
        system,
        planet,
        hits: (u32::try_from(a.rolled.0).ok()?, u32::try_from(a.rolled.1).ok()?),
        announce: a.announce,
        cause: r.code("ground_combat"),
        template,
        harrow: a.harrow.then(|| ti4_engine_core::rules::invasion::harrow::Facts { invader, defender, system, planet, announce: a.announce, cause: r.code("harrow") }),
    };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let mut driver = Driver::new(GroundRoundHits::new(facts), Resolver::new(&[invader, defender], Some(invader)), AbilityTable::default(), 1).with_seeds(seeds);
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy: None, rng: Some(a.rng_before) };
    let footprint = if a.harrow { ROUND_HITS_HARROW } else { ROUND_HITS };
    // Harrow draws, and so do the abilities of the reaction windows the removals open (Infantry II).
    let draws = a.harrow || !script.dice.is_empty();
    let driven = with_core(gs, &mut cx, src, nested_fp(footprint, script), |core, _| {
        let driven = drive(&mut driver, core, &env.kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        if draws {
            rng_after = Some(core.rng().clone());
        }
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let positions = if draws { crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()? } else { a.rng_before.clone() };
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let (a_hits, d_hits) = driver.window.assigned;
    let drawn = driver.window.harrow.as_ref().map(|h| h.drawn.clone()).unwrap_or_default();
    Some(CoreWindow { run: Run::new(((true, (a_hits as usize, d_hits as usize)), (drawn, positions))).with_events_vec(events), questions: driven.rendered })
}


// --- the start of a ground combat round (B2) -----------------------------------------------------------

/// What the string `begin_ground_round` was called with.
#[derive(Debug, Clone, Copy)]
pub struct BeginGroundArgs<'a> {
    pub invader: &'a PlayerId,
    pub defender: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub planet: &'a ti4_model::id::PlanetId,
    /// A copied round agent makes the round its own event.
    pub copied_round: bool,
    /// The sources of the game (the kernel's content scope: a round agent reads which units are ground forces).
    pub sources: ti4_model::content_types::SourceSet,
}

/// Why the start of the round holds something the core window does not carry: a Naaz side
/// (the Supercharge offer), Proxima Targeting VI, a Letnev side with infantry on the planet (the Dunlain Reaper),
/// a neutral side.
#[must_use]
pub fn begin_ground_unported(gs: &GameState, a: &BeginGroundArgs<'_>) -> Option<&'static str> {
    let sides = [a.invader, a.defender];
    if sides.into_iter().any(crate::neutral_units::is_neutral) {
        return Some("neutral side");
    }
    let faction = |id: &PlayerId| gs.player(id).map_or("", |p| p.faction.as_str());
    if sides.into_iter().any(|id| faction(id) == "naaz") {
        return Some("Naaz Supercharge");
    }
    if crate::factions::bastion_units::watches_ground_round(gs, &sides) {
        return Some("Proxima Targeting VI");
    }
    let infantry = |id: &PlayerId| gs.system_state(a.system).on_planet_of(a.planet, id).into_iter().any(|u| u.type_id.as_str().contains("infantry"));
    if sides.into_iter().any(|id| faction(id) == "letnev" && infantry(id)) {
        return Some("Dunlain Reaper");
    }
    None
}

const BEGIN_GROUND: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Meta, Slice::Transient, Slice::Pending],
);

/// The core half of `InvasionWindow::begin_ground_round`. `None` for the positions of [`begin_ground_unported`],
/// nested activity and a position the codecs refuse.
#[must_use]
pub fn begin_ground(gs: &mut GameState, a: &BeginGroundArgs<'_>, script: &Recording) -> Option<CoreWindow<bool>> {
    use ti4_engine_core::rules::invasion::begin::{BeginGroundRound, Facts};
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let invader = r.seat(a.invader.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let defender = r.seat(a.defender.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let planet = <ti4_engine_core::generated::Planet as IdStr>::from_id(a.planet.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let numbered = gs.rules_version >= ti4_model::state::RULES_FACTION_EFFECTS;
    let env = crate::core_invasion::ground_env(&before, &cx, a.sources).why("the invasion environment could not be built from the position")?;
    let mut driver = Driver::new(BeginGroundRound::new(Facts { system, planet, invader, defender, copied: a.copied_round, numbered }), Resolver::new(&[invader, defender], Some(invader)), AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(BEGIN_GROUND, script), |core, _| {
        let driven = drive(&mut driver, core, &env.kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    Some(CoreWindow { run: Run::new(true).with_events_vec(events), questions: driven.rendered })
}

// --- the tail of the ground roll step (B2) -------------------------------------------------------------

/// What the string `roll_ground_tail` was called with.
#[derive(Debug, Clone, Copy)]
pub struct RollTailArgs<'a> {
    pub invader: &'a PlayerId,
    pub defender: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub planet: &'a ti4_model::id::PlanetId,
    /// The hits of the two rolls (invader, defender).
    pub rolled: (usize, usize),
    pub bump: bool,
    pub announce: bool,
}

/// Why the tail holds something the core window does not carry: X-89 Bacterial Weapon (doubled staged hits), a replay mark,
/// a neutral side. A readied The Thundarian is no reason (package W1): it listens to the event that ENDS the "Roll Dice" step,
/// which the string emits after the tail (`roll_ground_step`), outside this window; its replays run the tail again from the
/// state the step left (the replay mark is cleared before the event, so the tail never sees one).
#[must_use]
pub fn roll_tail_unported(gs: &GameState, a: &RollTailArgs<'_>) -> Option<&'static str> {
    if crate::neutral_units::is_neutral(a.invader) || crate::neutral_units::is_neutral(a.defender) {
        Some("neutral side")
    } else if [a.invader, a.defender].into_iter().any(|id| gs.player(id).is_some_and(|p| p.technologies.contains(ti4_model::id::text(&"x89c4")))) {
        Some("X-89 Bacterial Weapon")
    } else if gs.faction_marks.contains_key(crate::combat::ROLL_REPLAY_MARK) {
        Some("roll replay mark")
    } else {
        None
    }
}

const ROLL_TAIL: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng],
    &[Slice::Meta, Slice::Transient, Slice::Pending, Slice::Rng],
);

/// The core half of `InvasionWindow::roll_ground_tail`. `None` for the positions of [`roll_tail_unported`], nested
/// activity (a card played in a reroll window) and a position the codecs refuse.
#[must_use]
pub fn roll_tail(
    gs: &mut GameState,
    a: &RollTailArgs<'_>,
    rng_before: &crate::rng::RngPositions,
    string_after: &std::cell::Cell<Option<crate::rng::RngPositions>>,
    script: &Recording,
) -> Option<CoreWindow<(usize, usize, Vec<ti4_model::state::InvasionDie>)>> {
    use ti4_engine_core::rules::invasion::tail::{Facts, GroundRollTail};
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    let invader = r.seat(a.invader.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let defender = r.seat(a.defender.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let planet = <ti4_engine_core::generated::Planet as IdStr>::from_id(a.planet.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, ti4_model::content_types::SourceSet::default()).why("the invasion environment could not be built from the position")?;
    let facts = Facts { invader, defender, system, planet, rolled: (u32::try_from(a.rolled.0).ok()?, u32::try_from(a.rolled.1).ok()?), bump: a.bump, announce: a.announce };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let mut driver = Driver::new(GroundRollTail::new(facts), Resolver::new(&[invader, defender], Some(invader)), AbilityTable::default(), 1).with_seeds(seeds);
    // The cards that reroll the staged dice (Fire Team) read the texts of the staged sets.
    let (reroll, texts) = reroll_facts(&mut cx, &r, &before)?;
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy: None, rng: Some(rng_before) };
    let driven = crate::reactions_core::with_reroll(reroll, || {
        with_core(gs, &mut cx, src, nested_fp(ROLL_TAIL, script), |core, _| {
            let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|c, st, rendered| roll_details(ti4_content::ContentStore::embedded(), &texts, &|_| 0, c, st, rendered));
            staged = std::mem::take(&mut core.pending_mut().staged);
            rng_after = Some(core.rng().clone());
            driven
        })
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    let positions = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
    if let Some(string_side) = string_after.take() {
        crate::dual::compare_value(crate::dual::Area::Invasion, "invasion::roll_ground_tail.rng", &string_side, &positions);
    }
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let dice = driver
        .window
        .dice
        .iter()
        .map(|(seat, group, face, target, hit)| {
            Some(ti4_model::state::InvasionDie {
                planet: a.planet.clone(),
                player: r.player(*seat),
                group: cx.texts.text(Slice::Transient, *group).ok()?.to_owned(),
                face: *face,
                target: *target,
                hit: *hit,
            })
        })
        .collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    let (ah, dh) = driver.window.hits;
    Some(CoreWindow { run: Run::new((ah as usize, dh as usize, dice)).with_events_vec(events), questions: driven.rendered })
}

/// The texts of the staged sets of the position, for the cards that reroll them: the state texts of the kinds and, per unit
/// group of a staged entry, the code a question shows it by.
fn reroll_facts(cx: &mut Ctx, r: &Render, before: &GameState) -> Option<(ti4_engine_core::kernel::ability::RerollFacts, Vec<(ti4_engine_core::state::Code, String)>)> {
    let mut facts = ti4_engine_core::kernel::ability::RerollFacts {
        ground: cx.texts.intern(Slice::Transient, "ground").why("the text table of the typed state is full")?,
        shown: Vec::new(),
    };
    let mut texts = Vec::new();
    for set in before.reroll_staging.values() {
        for roll in &set.rolls {
            let state = cx.texts.intern(Slice::Transient, &roll.unit).why("the text table of the typed state is full")?;
            if !facts.shown.iter().any(|(s, _)| *s == state) {
                facts.shown.push((state, r.code(&roll.unit)));
                texts.push((state, roll.unit.clone()));
            }
        }
    }
    Some((facts, texts))
}

// --- which planet the invasion fights on next (B2) -----------------------------------------------------

/// What the string `next_fight` was called with.
#[derive(Debug, Clone, Copy)]
pub struct AdvanceArgs<'a> {
    pub invader: &'a PlayerId,
    pub system: &'a ti4_model::id::SystemId,
    pub planets: &'a [ti4_model::id::PlanetId],
    pub index: usize,
    pub coexisted: &'a [ti4_model::id::PlanetId],
    pub sources: ti4_model::content_types::SourceSet,
}

const ADVANCE: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards],
    &[],
);

/// The core half of `InvasionWindow::next_fight`. `None` for seats not in id order (the string breaks ties by
/// player id), a neutral invader, nested activity and a position the codecs refuse.
#[must_use]
pub(crate) fn advance(gs: &mut GameState, a: &AdvanceArgs<'_>, script: &Recording) -> Option<CoreWindow<crate::invasion::FightPick>> {
    use ti4_engine_core::rules::invasion::advance::{Advance, Facts, Upcoming};
    const LABEL: &str = "invasion::advance_fighting";
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    if r.seats.windows(2).any(|w| w[0] >= w[1]) {
        crate::dual::note_bail(LABEL, "seats not in id order");
        return None;
    }
    let invader = r.seat(a.invader.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let planet_of = |p: &ti4_model::id::PlanetId| <ti4_engine_core::generated::Planet as IdStr>::from_id(p.as_str());
    let planets = a.planets.iter().map(planet_of).collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    let coexisted = a.coexisted.iter().map(planet_of).collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, a.sources).why("the invasion environment could not be built from the position")?;
    let facts = Facts { invader, system, planets, index: a.index, coexisted };
    let mut driver = Driver::new(Advance::new(facts), Resolver::new(&[invader], Some(invader)), AbilityTable::default(), 1);
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(ADVANCE, script), |core, _| drive(&mut driver, core, &env.kernel, &r, &before, script)).why(crate::dual::reason::NOT_LENDABLE)?.why(crate::dual::reason::DRIVER_REFUSED)?;
    let pick = match driver.window.pick.clone()? {
        Upcoming::Fight { index, defender } => crate::invasion::FightPick::Fight { index, defender: r.player(defender) },
        Upcoming::Coexisters { index, remaining } => crate::invasion::FightPick::Coexisters { index, remaining: remaining.into_iter().map(|s| r.player(s)).collect() },
        Upcoming::Exhausted => crate::invasion::FightPick::Exhausted,
    };
    Some(CoreWindow { run: Run::new(pick), questions: driven.rendered })
}

/// What the string `CombatWindow::settle` starts from: the window's fields and the gates the string rules
/// answer once for the whole settle (see `rules::combat::machine::Env`).
#[derive(Debug, Clone)]
pub struct SettleArgs {
    pub attacker: PlayerId,
    pub defender: PlayerId,
    pub system: ti4_model::id::SystemId,
    pub step: StepS,
    /// 0 round, 1 combat start, 2 round end (`HitPhase`).
    pub phase: u8,
    pub resuming: bool,
    pub pending_retreats: Vec<PlayerId>,
    pub offered: Vec<(u32, PlayerId)>,
    pub damaged: Vec<(PlayerId, Vec<String>)>,
    pub fled: Vec<(PlayerId, Vec<String>)>,
    pub at_start: Vec<(PlayerId, Vec<String>)>,
    pub end_announced: bool,
    pub announce: bool,
    pub staging: bool,
    /// The feat occurrence the combat has already (the adapter begins one when the first round needs it).
    pub occurrence: Option<u64>,
    pub begin_occurrence: bool,
    pub munitions_after: [bool; 2],
    pub opening_unported: bool,
    /// A Letnev round agent listens (`COMBAT_ROUND_STARTED` carries `round_seq`).
    pub round_seq_listens: bool,
    pub barrage_unported: bool,
    pub roll_unported: bool,
    pub retreat_unported: bool,
    /// The first of the four gates that is set, as the census text.
    pub gate_why: Option<&'static str>,
    pub neighbors: [Vec<(ti4_model::id::SystemId, bool)>; 2],
    pub may_retreat: [bool; 2],
    pub duranium: [bool; 2],
    pub devotion: bool,
}

/// What a settle leaves besides the state: the stage, the outcome (winner, rounds), the window fields, the dice
/// of the settle (hit number, faces) and where the streams stand.
pub type SettleOut = (
    StepS,
    Option<(Option<PlayerId>, u32)>,
    (u8, bool, Vec<PlayerId>, Vec<(u32, PlayerId)>),
    (Vec<(PlayerId, Vec<String>)>, Vec<(PlayerId, Vec<String>)>, bool),
    Vec<(u32, Vec<u32>)>,
    crate::rng::RngPositions,
);

const SETTLE: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
    &[Slice::Meta, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
);

/// The core half of `CombatWindow::settle`: the composed machine from the stage the settle starts in to the next
/// decision or the end of the fight. `None` for the positions the machine bails at (`Machine::bailed`), neutral
/// sides, reaction activity inside an emission, or a position the codecs refuse.
#[must_use]
pub fn settle_machine(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: SettleArgs,
    rng_before: &crate::rng::RngPositions,
    script: &Recording,
) -> Option<CoreWindow<SettleOut>> {
    settle_run(gs, sources, a, rng_before, script, None)
}

/// The core half of the answer to the decision a settle stopped at (`CombatWindow::resolve` without the settle
/// that follows): the script holds the pending question and the answer, then the commander's question of a
/// sustain. `string_pays`: the string asks the Letnev commander's payment. `None` (the bail-outs): those of
/// [`settle_machine`], a sustain below `RULES_FACTION_BUGFIXES`, a commander held another way than as an unlocked
/// leader, and the positions `Machine::bailed` names.
#[must_use]
pub fn answer_machine(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: SettleArgs,
    rng_before: &crate::rng::RngPositions,
    script: &Recording,
    string_pays: bool,
) -> Option<CoreWindow<SettleOut>> {
    if matches!(a.step, StepS::Queue(false, ..)) {
        if gs.rules_version < ti4_model::state::RULES_FACTION_BUGFIXES {
            crate::dual::note_bail_here("a sustain answer in a game below the faction bug fixes");
            return None;
        }
        let front = match &a.step {
            StepS::Queue(_, _, queue) => queue.first().map(|p| p.player.clone()),
            _ => None,
        }?;
        let unlocked = gs
            .player(&front)
            .is_some_and(|p| p.leaders.get(ti4_model::id::text("letnevcommander")) == Some(&ti4_model::state::LeaderStatus::Unlocked));
        if string_pays && !unlocked {
            crate::dual::note_bail_here("the Letnev commander's payment held another way than as an unlocked leader");
            return None;
        }
    }
    settle_run(gs, sources, a, rng_before, script, Some(()))
}

#[allow(clippy::too_many_lines, reason = "one environment, built field by field")]
fn settle_run(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    a: SettleArgs,
    rng_before: &crate::rng::RngPositions,
    script: &Recording,
    answering: Option<()>,
) -> Option<CoreWindow<SettleOut>> {
    use ti4_engine_core::rules::combat::flow::{Pend, Phase, Step};
    use ti4_engine_core::rules::combat::machine::{Env, Fields, Machine, Stop};
    if crate::neutral_units::is_neutral(&a.attacker) || crate::neutral_units::is_neutral(&a.defender) {
        crate::dual::note_bail_here(crate::dual::reason::NEUTRAL);
        return None;
    }
    // The first round begins the occurrence itself, once the opening has not ended the fight.
    let begin_occurrence = a.begin_occurrence && a.occurrence.is_none();
    let (mut cx, r) = tables(gs).why(crate::dual::reason::NO_TABLES)?;
    nested_check(script)?;
    waylay_check(script)?;
    let seat = |p: &PlayerId| r.seat(p.as_str());
    let seats = |l: &[PlayerId]| l.iter().map(seat).collect::<Option<Vec<_>>>();
    let (attacker, defender) = (seat(&a.attacker).why(crate::dual::reason::NO_SEAT)?, seat(&a.defender).why(crate::dual::reason::NO_SEAT)?);
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
    let unit_of = |k: &str| <ti4_engine_core::generated::Unit as IdStr>::from_id(k);
    let units = |l: &[String]| l.iter().map(|k| unit_of(k)).collect::<Option<Vec<_>>>();
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources).why(WINDOW_ENV)?;
    // The unit types present: every one a roll group or a barrage can name.
    let mut present: Vec<ti4_engine_core::generated::Unit> = Vec::new();
    let cell = before.system_state(&a.system);
    for p in cell.units.iter().chain(cell.planet_units.values().flatten()) {
        let u = unit_of(p.type_id.as_str()).why(crate::dual::reason::UNKNOWN_ID)?;
        if !present.contains(&u) {
            present.push(u);
        }
    }
    present.sort_unstable();
    if present.len() > 10 {
        crate::dual::note_bail_here("more than ten unit types in the fight (the roll group texts are pre-interned)");
        return None;
    }
    let mut unit_codes = Vec::new();
    for u in &present {
        unit_codes.push((*u, cx.texts.intern(Slice::Transient, u.as_str()).why("the text table of the typed state is full")?));
    }
    let mut groups = Vec::new();
    let mut texts: Vec<(ti4_engine_core::state::Code, String)> = Vec::new();
    for mask in 1_u32..(1 << present.len()) {
        let kinds: Vec<_> = present.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, u)| *u).collect();
        let mut ids: Vec<&str> = kinds.iter().map(|u| u.as_str()).collect();
        ids.sort_unstable();
        let joined = ids.join("+");
        // The roll names the group by its ids in alphabetical order; the machine looks the group up by its
        // sorted unit types, so the key must be sorted the same way.
        let state_code = cx.texts.intern(Slice::Transient, &joined).why("the text table of the typed state is full")?;
        texts.push((state_code, joined.clone()));
        groups.push((kinds.clone(), state_code, r.code(&joined)));
    }
    let nb = |side: usize| -> Option<Vec<(ti4_engine_core::generated::System, bool)>> {
        a.neighbors[side].iter().map(|(s, ok)| Some((<ti4_engine_core::generated::System as IdStr>::from_id(s.as_str()).why(crate::dual::reason::UNKNOWN_ID)?, *ok))).collect()
    };
    let phases = ["pre_roll", "barrage", "resolving_hits", "retreating", "complete"];
    let mut phase_codes = [ti4_engine_core::state::Code::default(); 5];
    for (i, p) in phases.iter().enumerate() {
        phase_codes[i] = cx.texts.intern(Slice::Transient, p).why("the text table of the typed state is full")?;
    }
    let menv = Env {
        attacker,
        defender,
        system,
        announce: a.announce,
        staging: a.staging,
        occurrence: a.occurrence.unwrap_or(0),
        begin_occurrence,
        phases: phase_codes,
        opening_unported: a.opening_unported,
        round_seq_listens: a.round_seq_listens,
        barrage_unported: a.barrage_unported,
        barrage_cause: r.code("unit_ability:anti_fighter_barrage"),
        barrage_kind: r.code("anti_fighter_barrage"),
        barrage_extra: cx.texts.intern(Slice::Transient, "extra barrage").why("the text table of the typed state is full")?,
        unit_codes,
        roll_unported: a.roll_unported,
        munitions_after: a.munitions_after,
        roll_cause: r.code("space_combat"),
        hacan: r.code("hacan_flagship"),
        groups,
        alpha: {
            let mut v = present.clone();
            v.sort_unstable_by_key(|u| u.as_str());
            v
        },
        retreat_cause: cx.texts.intern(Slice::Transient, "retreat_capacity").why("the text table of the typed state is full")?,
        retreat_pool: r.code("reinforcements"),
        retreat_unported: a.retreat_unported,
        neighbors: [nb(0)?, nb(1)?],
        may_retreat: a.may_retreat,
        duranium: a.duranium,
        devotion: a.devotion,
        casualty: r.code("assign_casualty"),
        commander: r.code("letnevcommander"),
        end_codes: ti4_engine_core::rules::combat::end_announce::Codes {
            losers: r.code("losers"),
            retreated: r.code("retreated"),
            reasons: [r.code("retreat"), r.code("draw"), r.code("destruction"), r.code("round_limit")],
        },
    };
    let step = match &a.step {
        StepS::Opening(n) => Step::Opening { round: *n },
        StepS::Announcing(n, who, done) => Step::Announcing { round: *n, asking: seat(who)?, announced: seats(done)? },
        StepS::Rolling(n) => Step::Rolling { round: *n },
        StepS::AfterBarrage(n) => Step::AfterBarrage { round: *n },
        StepS::Queue(assigning, n, queue) => Step::Queue {
            assigning: *assigning,
            round: *n,
            queue: queue
                .iter()
                .map(|p| Some(Pend { player: seat(&p.player)?, hits: u32::try_from(p.hits).ok()?, producer: seat(&p.producer)?, non_fighters_only: p.non_fighters_only, producer_assigns: p.producer_assigns }))
                .collect::<Option<_>>()?,
        },
        StepS::Retreating(n, leaving) => Step::Retreating { round: *n, leaving: seats(leaving)? },
        StepS::Done => Step::Done,
    };
    let pairs = |l: &[(PlayerId, Vec<String>)]| l.iter().map(|(p, k)| Some((seat(p)?, units(k)?))).collect::<Option<Vec<_>>>();
    let fields = Fields {
        step,
        phase: match a.phase {
            1 => Phase::CombatStart,
            2 => Phase::RoundEnd,
            _ => Phase::Round,
        },
        resuming: a.resuming,
        pending_retreats: seats(&a.pending_retreats)?,
        offered: a.offered.iter().map(|(n, p)| Some((*n, seat(p)?))).collect::<Option<_>>()?,
        damaged: pairs(&a.damaged)?,
        fled: pairs(&a.fled)?,
        at_start: pairs(&a.at_start)?,
        scoring: false,
        outcome: None,
        end_announced: a.end_announced,
    };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let machine = if answering.is_some() { Machine::awaiting(menv, fields) } else { Machine::new(menv, fields) };
    let mut driver = Driver::new(machine, resolver, AbilityTable::default(), 1).with_seeds(seeds);
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy: None, rng: Some(rng_before) };
    let held = |seat: ti4_engine_core::ids::Seat| before.player(&PlayerId::new(r.seats[usize::from(seat.0)].clone())).map_or(0, |p| i64::from(p.trade_goods.max(0)));
    let content = ti4_content::ContentStore::embedded();
    let driven = with_core(gs, &mut cx, src, nested_fp(SETTLE, script), |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|c, st, rendered| {
            roll_details(content, &texts, &held, c, st, rendered);
            if let ti4_engine_core::choice::Ask::Combat(ask @ ti4_engine_core::choice::combat::Ask::CancelHit { .. }) = &c.ask {
                rendered.details.insert("sustainers".to_owned(), crate::core_combat::sustainers_detail(&env.kernel, st, ask, c.actor));
            }
        });
        staged = std::mem::take(&mut core.pending_mut().staged);
        rng_after = Some(core.rng().clone());
        driven
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    let w = &driver.window;
    if let Some(Stop::Bailed(why)) = w.stop {
        use ti4_engine_core::rules::combat::machine::Bail;
        crate::dual::note_bail_here(match why {
            Bail::RollEndsFight => "the machine: a roll that ends the fight",
            Bail::Unported => "the machine: a position it does not carry (cancelled hits, Duranium, staged destructions, Devotion, a retreat the core does not move)",
        });
        return None;
    }
    if answering.is_some() {
        // The previews are not part of the serialized option (`serde(skip)`), so the whole-choice comparison
        // does not see them: compare them here, question by question.
        let previews = |c: &Choice| c.options.iter().map(|o| o.preview.clone()).collect::<Vec<_>>();
        for (asked, rendered) in script.questions.iter().zip(&driven.rendered) {
            crate::dual::compare_value(crate::dual::Area::Driver, "combat::answer.previews", &previews(&asked.choice), &previews(rendered));
        }
    }
    apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    // `remember_sustain_target` is the core window's own (`sustain_offer::remember_target`, a typed mark).
    let positions = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let player = |s: ti4_engine_core::ids::Seat| Some(PlayerId::new(r.seats.get(usize::from(s.0))?.clone()));
    let players = |l: &[ti4_engine_core::ids::Seat]| l.iter().map(|s| player(*s)).collect::<Option<Vec<_>>>();
    let f = &w.fields;
    let next = match &f.step {
        Step::Opening { round } => StepS::Opening(*round),
        Step::Announcing { round, asking, announced } => StepS::Announcing(*round, player(*asking)?, players(announced)?),
        Step::Rolling { round } => StepS::Rolling(*round),
        Step::AfterBarrage { round } => StepS::AfterBarrage(*round),
        Step::Queue { assigning, round, queue } => StepS::Queue(
            *assigning,
            *round,
            queue
                .iter()
                .map(|p| Some(PendS { player: player(p.player)?, hits: p.hits as usize, producer: player(p.producer)?, non_fighters_only: p.non_fighters_only, producer_assigns: p.producer_assigns }))
                .collect::<Option<_>>()?,
        ),
        Step::Retreating { round, leaving } => StepS::Retreating(*round, players(leaving)?),
        Step::Done => StepS::Done,
    };
    let outcome = match f.outcome {
        Some((who, rounds)) => Some((match who { Some(s) => Some(player(s)?), None => None }, rounds)),
        None => None,
    };
    let phase = match f.phase {
        Phase::Round => 0,
        Phase::CombatStart => 1,
        Phase::RoundEnd => 2,
    };
    let kinds = |l: &[ti4_engine_core::generated::Unit]| -> Vec<String> { l.iter().map(|u| u.as_str().to_owned()).collect() };
    let mut damaged = f.damaged.iter().map(|(s, l)| Some((player(*s)?, { let mut k = kinds(l); k.sort(); k }))).collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    damaged.sort();
    let fled = f.fled.iter().map(|(s, l)| Some((player(*s)?, kinds(l)))).collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    let mut offered = f.offered.iter().map(|(n, s)| Some((*n, player(*s)?))).collect::<Option<Vec<_>>>().why(crate::dual::reason::UNKNOWN_ID)?;
    offered.sort();
    offered.dedup();
    let value: SettleOut = (
        next,
        outcome,
        (phase, f.resuming, players(&f.pending_retreats)?, offered),
        (damaged, fled, f.end_announced),
        w.drawn.clone(),
        positions,
    );
    Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions: driven.rendered })
}

/// Whether `CombatWindow::settle` is compared as one composed machine (the default on `driver=dual`). Off,
/// the windows inside the string loop (round opening, barrage, roll, flow, retreat) are compared one by one.
#[must_use]
pub fn composed() -> bool {
    COMPOSED.with(Cell::get)
}

/// Run `body` with the composed machine switched `on` or off on this thread.
pub fn with_composed<T>(on: bool, body: impl FnOnce() -> T) -> T {
    let before = COMPOSED.with(|c| c.replace(on));
    let out = body();
    COMPOSED.with(|c| c.set(before));
    out
}
