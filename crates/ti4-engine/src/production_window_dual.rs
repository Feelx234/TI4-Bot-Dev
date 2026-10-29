//! The core half of one step of the production window (package B3): the dual of
//! `production::resolve_timed`'s loop at the granularity of one question and its answer.
//!
//! The string side asks `pending_choice`, puts it to the table and applies the answer with `settle`
//! (`resolve_step`, without the end-of-use effects). Here the core window starts from the same fields
//! (`ProductionWindow::to_core`), the driver runs it to its question, the question is rendered in the
//! position and compared by the harness, and the recorded answer is applied; the question the window then
//! waits on is left to the next step's comparison.

use std::rc::Rc;

use ti4_engine_core::driver::{Driver, Status};
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::Resolver;
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;
use ti4_content::ContentStore;

use crate::core_sync::{Footprint, Sources, with_core};
use crate::dual::{CoreWindow, Run};
use crate::dual_window::{Recording, drive, event_of, nested_check, nested_fp, tables};
use crate::event::Event;
use crate::production::{ProductionWindow, WindowSnap};
use crate::render::{self, Position, Typed};

const FOOTPRINT: Footprint = Footprint {
    reads: crate::production_view::READS.union(SliceSet::of(&[Slice::Pending])),
    writes: SliceSet::of(&[Slice::Board, Slice::Players, Slice::Transient, Slice::Marks, Slice::Pending]),
};

/// Run one step of `before` (the string window as it stood at the question) on the core. `None`: the
/// position holds something the core window does not carry (the string result stands).
#[must_use]
pub fn step(
    before: &ProductionWindow,
    gs: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    script: &Recording,
    emit_timing: bool,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
) -> Option<CoreWindow<WindowSnap>> {
    if crate::production_view::unported_production(gs, player) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let seat = r.seat(player.as_str())?;
    let window = before.to_core(gs, content, sources, seat, emit_timing)?;
    let kernel = crate::production_view::kernel();
    let mut driver = Driver::new(window, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
    // The systems the producing player sees as adjacent to the producing system (Argent's agent reads them; the
    // core is lent no galaxy). No galaxy: none, which is what the string agent finds too.
    let neighbours: Vec<String> = galaxy
        .map(|g| crate::movement::PlayerAdjacency::new(gs, content, sources, g, player).neighbours(before.system.as_str()).into_iter().collect())
        .unwrap_or_default();
    let reach = vec![(player.to_string(), before.system.to_string(), neighbours)];
    let before_state = gs.clone();
    let mut rendered = Vec::new();
    let mut staged = Vec::new();
    let driven = crate::reactions_core::with_reach(reach, || {
        with_core(gs, &mut cx, Sources::default(), nested_fp(FOOTPRINT, script), |core, _| {
            let driven = crate::dual_window::drive_positioned(&mut driver, core, kernel, Some(kernel), &r, &before_state, script, &|_, _, _| {});
            staged = std::mem::take(&mut core.pending_mut().staged);
            if let Ok(d) = &driven {
                // The question the window then waits on is the next step's.
                rendered = d.rendered.iter().take(script.questions.len().max(1)).cloned().collect();
            }
            driven.map(|_| ())
        })
    })
    .ok()?;
    driven.ok()?;
    crate::dual_window::apply_staged(gs, &r, &staged)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    let w = &driver.window;
    let unit = |u: ti4_engine_core::generated::Unit| <ti4_engine_core::generated::Unit as crate::generated::IdStr>::as_str(u).to_owned();
    let stage = match w.stage {
        ti4_engine_core::rules::production::window::Stage::Choosing => "choosing".to_owned(),
        ti4_engine_core::rules::production::window::Stage::Paying { unit: u, owed, made, cost, paid } => format!("paying|{}|{owed}|{made}|{cost}|{paid}", unit(u)),
        ti4_engine_core::rules::production::window::Stage::Placing { unit: u, made } => format!("placing|{}|{made}", unit(u)),
        ti4_engine_core::rules::production::window::Stage::Done => "done".to_owned(),
    };
    let snap = WindowSnap {
        stage,
        remaining: i64::from(w.remaining),
        free_capacity: i64::from(w.free_capacity),
        small_exemptions: i64::from(w.small_exemptions),
        credit: i64::from(w.credit),
        discount_remaining: i64::from(w.discount_remaining),
        purchased: w.purchased,
        placement_timing_done: w.placement_timing_done,
        produced: w.produced.as_slice().iter().map(|(u, s)| (unit(*u), render::production::spot_text(*s))).collect(),
    };
    Some(CoreWindow { run: Run::new(snap).with_events_vec(events), questions: rendered })
}

const PAY_FOOTPRINT: Footprint = Footprint {
    reads: crate::production_view::READS,
    writes: SliceSet::of(&[Slice::Board, Slice::Players]),
};

/// The core half of one iteration of the free payment loop (`pay_with_observation_credit`): the worth the
/// iteration collected, `None` when it collected nothing.
#[must_use]
pub fn pay_step(
    gs: &mut GameState,
    player: &PlayerId,
    kind: crate::production::Spend,
    cost: i64,
    used_credit: i64,
    owed: i64,
    paid: i64,
    script: &Recording,
) -> Option<CoreWindow<Option<i64>>> {
    use ti4_engine_core::query::economy::Spend as CoreSpend;
    use ti4_engine_core::rules::production::pay::{PayStep, Stage};
    if crate::production_view::unported_production(gs, player) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let seat = r.seat(player.as_str())?;
    let kernel = crate::production_view::kernel();
    let (phase, round) = (render::generic::core_phase(gs.phase), gs.round);
    let window = PayStep {
        who: seat,
        kind: match kind {
            crate::production::Spend::Resources => CoreSpend::Resources,
            crate::production::Spend::Influence => CoreSpend::Influence,
        },
        cost,
        used_credit,
        owed,
        paid,
        took: None,
        stage: Stage::Paying,
    };
    let mut driver = Driver::new(window, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
    let mut rendered = Vec::new();
    with_core(gs, &mut cx, Sources::default(), PAY_FOOTPRINT, |core, _| -> Result<(), ti4_engine_core::driver::DriverError> {
        if let Status::Asking(choice) = driver.run(core, kernel)? {
            let position = Position { state: Rc::new(core.clone()), kernel };
            rendered.push(Typed::new_at(&r, position, choice.clone(), phase, round).rendered);
            let answer = script.questions.first().and_then(|q| q.answer.clone());
            if let Some(pick) = answer.and_then(|id| render::parse_pick(&r, &choice.ask, &id)) {
                driver.resume(core, kernel, pick)?;
            }
        }
        Ok(())
    })
    .ok()?
    .ok()?;
    Some(CoreWindow { run: Run::new(driver.window.took), questions: rendered })
}

const TRIGGERED_FOOTPRINT: Footprint = Footprint {
    reads: crate::production_view::READS,
    writes: SliceSet::of(&[Slice::Board, Slice::Players]),
};

/// The seat's buildable units as the windows carry them (`None`: more than nine, or an id the tables do not know).
fn buildable_of(gs: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId) -> Option<[Option<ti4_engine_core::generated::Unit>; 9]> {
    let ids = crate::production_view::buildable_for(gs, content, sources, player)?;
    if ids.len() > 9 {
        return None;
    }
    let mut out = [None; 9];
    for (slot, id) in out.iter_mut().zip(&ids) {
        *slot = Some(<ti4_engine_core::generated::Unit as crate::generated::IdStr>::from_id(id)?);
    }
    Some(out)
}

/// Drive a whole window of the triggered productions to its end on a copy of the position, answering with
/// the recorded string answers; the questions it asked, rendered, and `value` of the window at the end.
fn run_triggered<W: ti4_engine_core::kernel::stage::Window>(
    gs: &mut GameState,
    player: &PlayerId,
    declined: fn(&GameState, &PlayerId) -> bool,
    window: impl FnOnce(ti4_engine_core::ids::Seat) -> Option<W>,
    script: &Recording,
    value: impl FnOnce(&W) -> bool,
) -> Option<CoreWindow<bool>> {
    if declined(gs, player) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let seat = r.seat(player.as_str())?;
    let window = window(seat)?;
    let kernel = crate::production_view::kernel();
    let mut driver = Driver::new(window, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
    let before_state = gs.clone();
    let mut rendered = Vec::new();
    with_core(gs, &mut cx, Sources::default(), nested_fp(TRIGGERED_FOOTPRINT, script), |core, _| {
        let driven = crate::dual_window::drive_positioned(&mut driver, core, kernel, Some(kernel), &r, &before_state, script, &|_, _, _| {});
        if let Ok(d) = &driven {
            rendered = d.rendered.clone();
        }
        driven.map(|_| ())
    })
    .ok()?
    .ok()?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    Some(CoreWindow { run: Run::new(value(&driver.window)).with_events_vec(events), questions: rendered })
}

/// The core half of `production::sling_relay`: whether the ship was produced.
#[must_use]
pub fn sling_relay(gs: &mut GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
    let buildable = buildable_of(gs, content, sources, player)?;
    run_triggered(
        gs,
        player,
        crate::production_view::unported_triggered,
        |seat| Some(ti4_engine_core::rules::production::triggered::SlingRelay::new(seat, buildable)),
        script,
        |w| w.produced,
    )
}

/// The core half of the loop of `production::integrated_economy` (the fleet-supply enforcement after it stays
/// with the caller): whether a unit was built.
#[must_use]
pub fn integrated_economy(
    gs: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    system: &ti4_model::id::SystemId,
    planet: &ti4_model::id::PlanetId,
    budget: i64,
    script: &Recording,
) -> Option<CoreWindow<bool>> {
    use crate::generated::IdStr;
    let buildable = buildable_of(gs, content, sources, player)?;
    let fleet_limit = crate::fleet::limit_for_enforcement(gs, content, player);
    let (system, planet) = (ti4_engine_core::generated::System::from_id(system.as_str())?, ti4_engine_core::generated::Planet::from_id(planet.as_str())?);
    let (budget, fleet_limit) = (i32::try_from(budget).ok()?, i32::try_from(fleet_limit).ok()?);
    run_triggered(
        gs,
        player,
        crate::production_view::unported_triggered,
        |seat| Some(ti4_engine_core::rules::production::triggered::EconomyWindow::new(seat, system, planet, budget, fleet_limit, buildable)),
        script,
        |w| w.built,
    )
}

/// The core half of `leaders::offer_production_hero`: whether the Hacan hero was used. The offer reads only the
/// seat's leaders and the use's sequence number, so only a table whose economy the typed state cannot hold is
/// declined.
#[must_use]
pub fn hero_offer(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
    run_triggered(
        gs,
        player,
        |s, _| crate::production_view::unported_economy(s),
        |seat| Some(ti4_engine_core::rules::production::triggered::HeroOffer::new(seat)),
        script,
        |w| w.used,
    )
}

const OFFER_FOOTPRINT: Footprint = Footprint {
    reads: crate::production_view::READS,
    writes: SliceSet::of(&[Slice::Players, Slice::Marks]),
};

/// The core half of `keleres::offer_agent`: whether the offer opened a window the caller must close. `None`: the position holds
/// something the core window does not carry (the string result stands).
#[must_use]
pub fn agent_offer(gs: &mut GameState, payer: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
    use ti4_engine_core::rules::production::offers::AgentOffer;
    let (mut cx, r) = tables(gs)?;
    nested_check(script)?;
    let seat = r.seat(payer.as_str())?;
    let before = gs.clone();
    let kernel = crate::production_view::kernel();
    let mut driver = Driver::new(AgentOffer::new(seat), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(OFFER_FOOTPRINT, script), |core, _| drive(&mut driver, core, kernel, &r, &before, script)).ok()?.ok()?;
    Some(CoreWindow { run: Run::new(driver.window.opened), questions: driven.rendered })
}
