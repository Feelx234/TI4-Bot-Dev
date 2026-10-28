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
}

/// What the string window did at its own level.
#[derive(Debug, Clone, Default)]
pub struct Recording {
    pub questions: Vec<Asked>,
    pub emits: Vec<Event>,
    /// Something happened inside an emission of the window (a reaction window's question, an event an
    /// ability caused): the abilities are string-only for now, so a core window cannot follow.
    pub nested: bool,
    /// The emission depth the window started at: a window may itself run inside an emission (an ability's
    /// effect opens a combat), and its own level is that depth, not zero.
    base: u32,
}

thread_local! {
    static REC: RefCell<Option<Recording>> = const { RefCell::new(None) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
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

/// End the recording and take it.
pub fn finish() -> Recording {
    REC.with(|r| r.borrow_mut().take()).unwrap_or_default()
}

/// A question was put (`Table::ask*`).
pub fn note_ask(choice: &Choice, outcome: &Result<ChoiceOption, IllegalChoice>) {
    let depth = DEPTH.with(Cell::get);
    REC.with(|r| {
        if let Some(rec) = r.borrow_mut().as_mut() {
            if rec.base == depth {
                rec.questions.push(Asked { choice: choice.clone(), answer: outcome.as_ref().ok().map(|o| o.id.clone()) });
            } else if depth > rec.base {
                rec.nested = true;
            }
        }
    });
}

/// An ability's effect is about to run: whatever it does is string-only until the abilities are ported.
pub fn note_effect() {
    REC.with(|r| {
        if let Some(rec) = r.borrow_mut().as_mut() {
            rec.nested = true;
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
pub fn event_of(r: &Render, rec: &EventRec) -> Option<Event> {
    if !rec.payload.units.is_empty() {
        return None;
    }
    let mut payload = BTreeMap::new();
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
            Val::Outcome(_) => return None,
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
    let (phase, round) = (render::generic::core_phase(gs.phase), gs.round);
    let mut out = Driven { questions: Vec::new(), rendered: Vec::new(), finished: false };
    let mut status = driver.run(core, kernel)?;
    loop {
        match status {
            Status::Done => {
                out.finished = true;
                return Ok(out);
            }
            Status::Asking(choice) => {
                let typed = Typed::new(r, choice.clone(), phase, round);
                out.questions.push(choice.clone());
                let mut rendered = typed.rendered;
                complete(&choice, core, &mut rendered);
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

/// The tables a core half needs, for one position. `None`: more than 16 owners.
pub(crate) fn tables(gs: &GameState) -> Option<(Ctx, Render)> {
    let cx = Ctx::of(gs).ok()?;
    let r = Render::new(cx.owners.ids().to_vec());
    Some((cx, r))
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
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let seat = r.seat(player.as_str())?;
    let source = r.code("letnevcommander");
    let before = gs.clone();
    let kernel = Kernel::plain();
    let resolver = Resolver::new(&[seat], Some(seat));
    let mut driver = Driver::new(SustainCommander::new(seat, source), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), SUSTAIN_COMMANDER, |core, _| {
        let driven = drive(&mut driver, core, &kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(events), questions: driven.rendered })
}

const SUSTAIN_OFFER: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Board, Slice::Players, Slice::Transient, Slice::Pending],
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
    if script.nested {
        return None;
    }
    if gs.rules_version < ti4_model::state::RULES_FACTION_EFFECTS {
        return None;
    }
    let unlocked = gs
        .player(a.player)
        .is_some_and(|p| p.leaders.get(ti4_model::id::text("letnevcommander")) == Some(&ti4_model::state::LeaderStatus::Unlocked));
    if a.string_pays && !unlocked {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let (player, producer) = (r.seat(a.player.as_str())?, r.seat(a.producer.as_str())?);
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let source = r.code("letnevcommander");
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources)?;
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
    let driven = with_core(gs, &mut cx, Sources::default(), SUSTAIN_OFFER, |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|c, st, rendered| {
            if let ti4_engine_core::choice::Ask::Combat(ask) = &c.ask {
                rendered.details.insert("sustainers".to_owned(), crate::core_combat::sustainers_detail(&env.kernel, st, ask, c.actor));
            }
        });
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    // `remember_sustain_target`: the string writes the handoff of the last unit that sustained.
    if offer.staging
        && let Some((site, piece)) = driver.window.last
    {
        let unit = ti4_model::units::Unit {
            type_id: ti4_model::id::UnitTypeId::new(piece.kind().as_str()),
            owner: PlayerId::new(r.seat_text(piece.owner())),
            sustained_damage: piece.sustained(),
            galvanized: piece.galvanized(),
        };
        let planet = match site {
            ti4_engine_core::choice::combat::SustainSite::Planet { planet, .. } => Some(planet.as_str()),
            ti4_engine_core::choice::combat::SustainSite::Space(_) => None,
        };
        gs.faction_marks.insert(
            "combat:sustain_target".to_owned(),
            serde_json::json!({"system": a.system, "planet": planet, "unit": unit}).to_string(),
        );
    }
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
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
    if script.nested || a.neutral {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let player = r.seat(a.player.as_str())?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources)?;
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
    let driven = with_core(gs, &mut cx, Sources::default(), ASSIGN_HITS, |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, rendered| {
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
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
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
    if script.nested || gs.players.iter().any(|p| p.faction.as_str().starts_with("keleres")) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let seat = r.seat(invader.as_str())?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str())?;
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, sources)?;
    let resolver = Resolver::new(&[seat], Some(seat));
    let mut driver = Driver::new(CommitGround::new(seat, sys), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), COMMIT_GROUND, |core, _| {
        let driven = drive(&mut driver, core, &env.kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .ok()?
    .ok()?;
    if driver.window.unsupported {
        return None;
    }
    apply_staged(gs, &r, &staged)?;
    let planets = driver.window.committed.iter().map(|p| ti4_model::id::PlanetId::new(p.as_str())).collect();
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
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
    if script.nested || crate::neutral_units::is_neutral(player) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let seat = r.seat(player.as_str())?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str())?;
    let pl = <ti4_engine_core::generated::Planet as IdStr>::from_id(planet.as_str())?;
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, sources)?;
    let resolver = Resolver::new(&[seat], Some(seat));
    let window = AbsorbGround::new(seat, sys, pl, u16::try_from(hits).ok()?, unit_ability, crate::supply::staging_enabled(&before));
    let mut driver = Driver::new(window, resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), ABSORB_GROUND, |core, _| {
        let driven = drive(&mut driver, core, &env.kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
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
    if script.nested || gs.players.iter().any(|p| p.faction.as_str() == "naalu") {
        return None;
    }
    let paid_yes = script.questions.first().and_then(|q| q.answer.as_deref()) == Some("yes")
        && crate::invasion::custodians_cost(gs, ti4_content::ContentStore::embedded(), invader) != 0;
    if paid_yes && crate::production_view::unported_window(gs) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let seat = r.seat(invader.as_str())?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str())?;
    let (phase, round) = (render::generic::core_phase(gs.phase), gs.round);
    let kernel = crate::production_view::kernel();
    let mut driver = Driver::new(Custodians::new(seat, sys, affordable), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
    let mut rendered = Vec::new();
    let mut staged = Vec::new();
    with_core(gs, &mut cx, Sources::default(), CUSTODIANS, |core, _| -> Result<(), DriverError> {
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
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
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
    let (cx, r) = tables(gs)?;
    let seat = r.seat(invader.as_str())?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str())?;
    let pl = <ti4_engine_core::generated::Planet as IdStr>::from_id(planet.as_str())?;
    let _ = (&cx, sources);
    let choice = FightRound::new(seat, sys, pl).question();
    let (phase, round) = (render::generic::core_phase(gs.phase), gs.round);
    Some(Typed::new(&r, choice, phase, round).rendered)
}

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
    if script.nested {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let target = r.seat(a.target.as_str())?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources)?;
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
    let driven = with_core(gs, &mut cx, Sources::default(), ASSIGN_HITS, |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
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
    if script.nested || crate::neutral_units::is_neutral(a.attacker) || crate::neutral_units::is_neutral(a.defender) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let attacker = r.seat(a.attacker.as_str())?;
    let defender = r.seat(a.defender.as_str())?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources)?;
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
    let driven = with_core(gs, &mut cx, Sources::default(), ASSIGN_HITS, |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|choice, _, rendered| {
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
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(events), questions: driven.rendered })
}

const LAND_ONE: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Board, Slice::Transient, Slice::Pending],
);

/// The core half of `InvasionWindow::commit_one` (one `commit|<n>|<planet>` landing). `None` (the bail-outs):
/// `plain` false (the question holds a commit candidate with an origin, a temporary space commit or the Crimson
/// deploy), nested activity (a reaction to `UNITS_COMMITTED`), a Keleres seat (the toll), the diplomacy promises
/// switched on, a ship-form unit flipping on landing, and a position the codecs refuse.
#[must_use]
pub fn land_one(
    gs: &mut GameState,
    sources: ti4_model::content_types::SourceSet,
    invader: &PlayerId,
    system: &ti4_model::id::SystemId,
    plain: bool,
    announce: bool,
    script: &Recording,
) -> Option<CoreWindow<()>> {
    use ti4_engine_core::rules::invasion::commit::LandOne;
    if !plain || script.nested || gs.diplomacy.enabled || gs.players.iter().any(|p| p.faction.as_str().starts_with("keleres")) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let seat = r.seat(invader.as_str())?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str())?;
    let before = gs.clone();
    let env = crate::core_invasion::ground_env(&before, &cx, sources)?;
    let mut driver = Driver::new(LandOne::new(seat, sys, announce), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), LAND_ONE, |core, _| {
        let driven = drive(&mut driver, core, &env.kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .ok()?
    .ok()?;
    if driver.window.unsupported {
        return None;
    }
    apply_staged(gs, &r, &staged)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    Some(CoreWindow { run: Run::new(()).with_events_vec(events), questions: driven.rendered })
}
