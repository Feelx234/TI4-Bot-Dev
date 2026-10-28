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
    for (key, list) in &rec.payload.lists {
        let seat = |s: &ti4_engine_core::ids::Seat| Some(Value::from(r.seats.get(usize::from(s.0))?.clone()));
        let v: Value = match list {
            ti4_engine_core::state::List::Seats(seats) => Value::Array(seats.iter().map(seat).collect::<Option<_>>()?),
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
fn tables(gs: &GameState) -> Option<(Ctx, Render)> {
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
/// flagship) or a Letnev round agent (own or copied).
#[must_use]
pub fn round_opening_unported(
    gs: &GameState,
    content: &ti4_content::ContentStore,
    a: &BeginRoundArgs<'_>,
) -> bool {
    let side = |id: &PlayerId| gs.player(id).is_none_or(|p| matches!(p.faction.as_str(), "naaz" | "letnev"));
    gs.diplomacy.enabled
        || side(a.attacker)
        || side(a.defender)
        || crate::factions::borrowed_round_agents::has_round_copy(gs, content, "letnevagent")
        || crate::factions::borrowed_round_agents::has_owner_space_round_agent(gs)
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
    if script.nested
        || crate::neutral_units::is_neutral(a.attacker)
        || crate::neutral_units::is_neutral(a.defender)
        || round_opening_unported(gs, content, &a)
    {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let attacker = r.seat(a.attacker.as_str())?;
    let defender = r.seat(a.defender.as_str())?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources)?;
    let facts = Facts { attacker, defender, system, round: a.round, phase: cx.texts.intern(Slice::Transient, "pre_roll").ok()?, announce: a.announce };
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let mut driver = Driver::new(BeginRound::new(facts), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), BEGIN_ROUND, |core, _| {
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

const BEGIN_ROUND: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Meta, Slice::Transient, Slice::Pending],
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
struct DiceSeeds(String);

impl ti4_engine_core::state::SeedSource for DiceSeeds {
    fn seed_for(&self, seed: u64, domain: ti4_engine_core::state::RngDomain) -> [u8; 32] {
        if domain == ti4_engine_core::state::RngDomain::Dice {
            crate::rng::GameRng::derive_seed(seed, &self.0)
        } else {
            [0; 32]
        }
    }
}

/// Whether the barrage step holds something the core window does not carry: a reroll question (Jol-Nar
/// commander, Heart of Ixth), a Waylay played this round, an Argent side (`afb_excess`).
#[must_use]
pub fn barrage_unported(gs: &GameState, a: &BarrageArgs<'_>) -> bool {
    let heart = ti4_model::id::RelicId::new("heartofixth");
    gs.players.iter().any(|p| crate::relics::holds(gs, &p.id, &heart))
        || [a.attacker, a.defender].into_iter().any(|id| {
            gs.player(id).is_none_or(|p| {
                p.faction.as_str() == "argent"
                    || p.waylay_barrage_round.is_some_and(|r| r == gs.combat_round_seq)
            }) || crate::promissory::has_commander_ability(gs, id, "jolnarcommander")
        })
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
    if script.nested
        || !a.reproducible
        || crate::neutral_units::is_neutral(a.attacker)
        || crate::neutral_units::is_neutral(a.defender)
        || barrage_unported(gs, &a)
    {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let attacker = r.seat(a.attacker.as_str())?;
    let defender = r.seat(a.defender.as_str())?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources)?;
    let mut unit_codes = Vec::new();
    for p in before.system_state(a.system).units.iter().chain(before.system_state(a.system).planet_units.values().flatten()) {
        let unit = <ti4_engine_core::generated::Unit as IdStr>::from_id(p.type_id.as_str())?;
        if !unit_codes.iter().any(|(u, _)| *u == unit) {
            unit_codes.push((unit, cx.texts.intern(Slice::Transient, p.type_id.as_str()).ok()?));
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
        phase: cx.texts.intern(Slice::Transient, "barrage").ok()?,
        cause: r.code("unit_ability:anti_fighter_barrage"),
        kind: r.code("anti_fighter_barrage"),
        extra: cx.texts.intern(Slice::Transient, "extra barrage").ok()?,
        unit_codes,
    };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let mut driver = Driver::new(BarrageStep::new(facts), resolver, AbilityTable::default(), 1).with_seeds(seeds);
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy: None, rng: Some(rng_before) };
    let driven = with_core(gs, &mut cx, src, BARRAGE_STEP, |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        rng_after = Some(core.rng().clone());
        driven
    })
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let positions = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    let feat = driver.window.feat.iter().map(|s| Some(PlayerId::new(r.seats.get(usize::from(s.0))?.clone()))).collect::<Option<Vec<_>>>()?;
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
    &[Slice::Transient, Slice::Pending, Slice::Rng, Slice::Ledger],
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
    if script.nested || !a.reproducible || crate::neutral_units::is_neutral(a.attacker) || crate::neutral_units::is_neutral(a.defender) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let attacker = r.seat(a.attacker.as_str())?;
    let defender = r.seat(a.defender.as_str())?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources)?;
    let mut names = Vec::new();
    {
        let mut view = crate::core_combat::CombatView::of(&before, content, sources)?;
        for (seat, id) in [(attacker, a.attacker), (defender, a.defender)] {
            for (value, tag, _, types) in view.fleet_groups(id, a.system)? {
                let mut kinds: Vec<&str> = types.iter().map(|(u, _)| u.as_str()).collect();
                kinds.sort_unstable();
                names.push((seat, value, tag, cx.texts.intern(Slice::Transient, &kinds.join("+")).ok()?));
            }
        }
    }
    let facts = Facts { attacker, defender, system, phase: cx.texts.intern(Slice::Transient, "resolving_hits").ok()?, names };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let mut driver = Driver::new(RollStep::new(facts), resolver, AbilityTable::default(), 1).with_seeds(seeds);
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy: None, rng: Some(rng_before) };
    let driven = with_core(gs, &mut cx, src, ROLL_STEP, |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        rng_after = Some(core.rng().clone());
        driven
    })
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let positions = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    let player = |s: ti4_engine_core::ids::Seat| Some(PlayerId::new(r.seats.get(usize::from(s.0))?.clone()));
    let w = &driver.window;
    let queue = w
        .queue
        .iter()
        .map(|q| Some((player(q.player)?, q.hits as usize, player(q.producer)?, q.non_fighters_only, false)))
        .collect::<Option<Vec<_>>>()?;
    let mut damaged = w
        .damaged
        .iter()
        .map(|(s, kinds)| {
            let mut ids: Vec<String> = kinds.iter().map(|u| u.as_str().to_owned()).collect();
            ids.sort();
            Some((player(*s)?, ids))
        })
        .collect::<Option<Vec<_>>>()?;
    damaged.sort();
    let value: RollOut = (queue, damaged, w.drawn.clone(), positions);
    Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions: driven.rendered })
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
    if script.nested || crate::neutral_units::is_neutral(a.attacker) || crate::neutral_units::is_neutral(a.defender) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let seat = |p: &PlayerId| r.seat(p.as_str());
    let seats = |l: &[PlayerId]| l.iter().map(seat).collect::<Option<Vec<_>>>();
    let (attacker, defender) = (seat(a.attacker)?, seat(a.defender)?);
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let step = match &a.step {
        StepS::Opening(n) => Step::Opening { round: *n },
        StepS::Announcing(n, who, done) => Step::Announcing { round: *n, asking: seat(who)?, announced: seats(done)? },
        StepS::Rolling(n) => Step::Rolling { round: *n },
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
    let env = crate::core_combat::window_env(&before, &cx, sources)?;
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let mut driver = Driver::new(Flow::new(facts, step), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), FLOW_STEP, |core, _| {
        let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    let player = |s: ti4_engine_core::ids::Seat| Some(PlayerId::new(r.seats.get(usize::from(s.0))?.clone()));
    let players = |l: &[ti4_engine_core::ids::Seat]| l.iter().map(|s| player(*s)).collect::<Option<Vec<_>>>();
    let out = driver.window.out.clone()?;
    let next = match &out.next {
        Step::Opening { round } => StepS::Opening(*round),
        Step::Announcing { round, asking, announced } => StepS::Announcing(*round, player(*asking)?, players(announced)?),
        Step::Rolling { round } => StepS::Rolling(*round),
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
    if script.nested || crate::neutral_units::is_neutral(a.attacker) || crate::neutral_units::is_neutral(a.defender) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let seat = |p: &PlayerId| r.seat(p.as_str());
    let (attacker, defender) = (seat(a.attacker)?, seat(a.defender)?);
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let units = |kinds: &[String]| -> Option<Vec<ti4_engine_core::generated::Unit>> {
        kinds.iter().map(|k| <ti4_engine_core::generated::Unit as IdStr>::from_id(k)).collect()
    };
    let fled = a.fled.iter().map(|(p, k)| Some((seat(p)?, units(k)?))).collect::<Option<Vec<_>>>()?;
    let at_start = a.at_start.iter().map(|(p, k)| Some((seat(p)?, units(k)?))).collect::<Option<Vec<_>>>()?;
    let winner = match a.winner {
        Some(w) => Some(seat(w)?),
        None => None,
    };
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources)?;
    let codes = Codes {
        losers: r.code("losers"),
        retreated: r.code("retreated"),
        reasons: [r.code("retreat"), r.code("draw"), r.code("destruction"), r.code("round_limit")],
    };
    let facts = Facts { attacker, defender, system, rounds: a.rounds, winner, declared_draw: a.declared_draw, fled, at_start, codes, announce: a.announce };
    let resolver = Resolver::new(&[attacker, defender], Some(attacker));
    let mut driver = Driver::new(EndAnnounce::new(facts), resolver, AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), BEGIN_ROUND, |core, _| {
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
    if script.nested
        || gs.player(a.producer).is_none_or(|p| p.faction.as_str() != "argent")
        || crate::neutral_units::is_neutral(a.target)
    {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let producer = r.seat(a.producer.as_str())?;
    let target = r.seat(a.target.as_str())?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources)?;
    let facts = Facts { producer, target, system, excess: u16::try_from(a.excess).ok()?, subtype: r.code("raid_formation_damage") };
    let resolver = Resolver::new(&[producer], Some(producer));
    let mut driver = Driver::new(RaidFormation::new(facts), resolver, AbilityTable::default(), 1);
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
