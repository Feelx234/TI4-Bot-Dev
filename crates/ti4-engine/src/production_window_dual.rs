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
use crate::dual_window::{Recording, event_of, tables};
use crate::event::Event;
use crate::production::{ProductionWindow, WindowSnap};
use crate::render::{self, Position, Typed};

const FOOTPRINT: Footprint = Footprint {
    reads: crate::production_view::READS,
    writes: SliceSet::of(&[Slice::Board, Slice::Players]),
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
) -> Option<CoreWindow<WindowSnap>> {
    if crate::production_view::unported_window(gs) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let seat = r.seat(player.as_str())?;
    let window = before.to_core(gs, content, sources, seat, emit_timing)?;
    let kernel = crate::production_view::kernel();
    let (phase, round) = (render::generic::core_phase(gs.phase), gs.round);
    let mut driver = Driver::new(window, Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
    let mut rendered = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), FOOTPRINT, |core, _| -> Result<(), ti4_engine_core::driver::DriverError> {
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
    .ok()?;
    driven.ok()?;
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
    if crate::production_view::unported_window(gs) {
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
