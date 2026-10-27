//! The recorder of the string resolver's observable steps (package A5, the dual of the reaction-window
//! machinery).
//!
//! When `Area::Kernel` is on `Dual`, the outermost `Resolver::emit*` starts a recording; every step the
//! string resolver takes that is an answer from the rules (a participation, a political-secret bar, a
//! condition, a prohibition's predicate, a decider's pick, an effect with the events it emits, the
//! announcement at a window's close) is appended as an [`Item`] in call order. After the emission the
//! shim replays the core `Resolver` against that trace: the core asks the same questions in the same order
//! or the replay reports the first deviation, and its log, journal and spent frequency keys are compared
//! with the string resolver's.
//!
//! Costs nothing outside `Dual`: every hook is one thread-local read.

use std::cell::{Cell, RefCell};

use crate::event::Event;

/// One recorded step.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Emit { event: Event, ctx: bool },
    EmitEnd { cancelled: bool, ok: bool },
    Resolve,
    ResolveEnd,
    Part { ability: String, owner: String, result: bool },
    Bars { owner: String, result: bool },
    Cond { ability: String, result: bool },
    Forbids { result: bool },
    Skipped { player: String },
    Ask { player: String, options: Vec<String>, declinable: bool, chosen: Option<String> },
    EffectBegin { ability: String },
    EffectEnd { cancelled: bool },
    Closed,
    ClosedEnd,
}

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static ITEMS: RefCell<Vec<Item>> = const { RefCell::new(Vec::new()) };
}

/// Whether a recording is running on this thread.
#[inline]
#[must_use]
pub fn on() -> bool {
    ON.with(Cell::get)
}

/// Start a recording.
pub fn start() {
    ITEMS.with(|i| i.borrow_mut().clear());
    ON.with(|o| o.set(true));
}

/// Stop recording and take the items.
pub fn finish() -> Vec<Item> {
    ON.with(|o| o.set(false));
    ITEMS.with(|i| std::mem::take(&mut *i.borrow_mut()))
}

#[inline]
fn push(item: impl FnOnce() -> Item) {
    if on() {
        ITEMS.with(|i| i.borrow_mut().push(item()));
    }
}

pub fn emit_begin(event: &Event, ctx: bool) {
    push(|| Item::Emit { event: event.clone(), ctx });
}

pub fn emit_end(cancelled: bool, ok: bool) {
    push(|| Item::EmitEnd { cancelled, ok });
}

pub fn resolve_begin() {
    push(|| Item::Resolve);
}

pub fn resolve_end() {
    push(|| Item::ResolveEnd);
}

/// Record a participation answer and return it.
#[inline]
pub fn part(ability: &str, owner: &str, result: bool) -> bool {
    push(|| Item::Part { ability: ability.to_owned(), owner: owner.to_owned(), result });
    result
}

#[inline]
pub fn bars(owner: &str, result: bool) -> bool {
    push(|| Item::Bars { owner: owner.to_owned(), result });
    result
}

#[inline]
pub fn cond(ability: &str, result: bool) -> bool {
    push(|| Item::Cond { ability: ability.to_owned(), result });
    result
}

#[inline]
pub fn forbids(result: bool) -> bool {
    push(|| Item::Forbids { result });
    result
}

pub fn skipped(player: &str) {
    push(|| Item::Skipped { player: player.to_owned() });
}

pub fn ask(player: &str, options: Vec<String>, declinable: bool, chosen: Option<&str>) {
    push(|| Item::Ask { player: player.to_owned(), options, declinable, chosen: chosen.map(str::to_owned) });
}

pub fn effect_begin(ability: &str) {
    push(|| Item::EffectBegin { ability: ability.to_owned() });
}

pub fn effect_end(cancelled: bool) {
    push(|| Item::EffectEnd { cancelled });
}

pub fn closed_begin() {
    push(|| Item::Closed);
}

pub fn closed_end() {
    push(|| Item::ClosedEnd);
}
