//! The dual of the reaction-window machinery (package A5): after the string resolver finished an
//! outermost emission, rebuild the same resolver in the core and replay the recorded trace through it.
//!
//! The core `Resolver` owns the algorithm (player order, eligibility, "resolved in this window",
//! frequency scopes, the single-mandatory shortcut, passes, nesting, cancellation, prohibitions). The
//! rules' answers (participation, bars, conditions, prohibition predicates, the decider's pick, the
//! effects with the events they emit) come from the trace by a [`Host`] that checks each question
//! against the next recorded step: a question the core asks that the string resolver did not, or in a
//! different order, is a deviation. Afterwards the core's trace lines, journal, spent frequency keys and
//! the asks it put are compared with the string resolver's through `dual::run_dual_typed`.

use std::collections::BTreeSet;

use ti4_engine_core::choice::reaction::{Frequency as CFreq, Relation as CRel};
use ti4_engine_core::choice::{self, Choice, Opt, Pick};
use ti4_engine_core::kernel::resolver::{CannotDef, Line, UsedScope};
use ti4_engine_core::kernel::{AbilityDef, Host, Resolver as CoreResolver, TimingError as CoreError};
use ti4_engine_core::state::{CoreState, EventRec, Payload, Phase as CorePhase};
use ti4_engine_core::Seat;
use ti4_model::state::Phase;

use super::text::{ability_id, ability_text, event_kind, event_text, relation_text};
use super::trace::{self, Item};
use crate::dual::{self, Area, Backend};
use crate::event::Event;
use crate::render::Render;
use crate::timing::{Frequency, Resolver, TimingError};

/// Whether this emission should be recorded and replayed: `Area::Kernel` on `Dual` and no recording
/// already running (a nested emission joins the outer one).
#[inline]
#[must_use]
pub fn wants_dual() -> bool {
    !trace::on() && dual::backend(Area::Kernel) == Backend::Dual
}

/// One registered ability, as the string resolver holds it.
#[derive(Debug, Clone)]
pub struct AbilityMeta {
    pub id: String,
    pub owner: String,
    pub event: String,
    pub when: bool,
    pub frequency: Frequency,
    pub optional: bool,
    pub repeatable: bool,
    pub has_condition: bool,
}

/// The string resolver before the emission.
#[derive(Debug, Clone)]
pub struct View {
    pub initiative: Vec<String>,
    pub seating: Vec<String>,
    pub active: Option<String>,
    pub speaker: Option<String>,
    pub phase: Phase,
    pub round: u64,
    pub turn: u64,
    pub max_depth: usize,
    pub abilities: Vec<AbilityMeta>,
    pub cannot: Vec<String>,
    /// `(ability id, scope Debug text)`
    pub used: Vec<(String, String)>,
    pub log_len: usize,
    pub applied_len: usize,
}

/// The string resolver after the emission.
#[derive(Debug, Clone)]
pub struct After {
    pub log: Vec<String>,
    /// `(event type, id, cancelled)`
    pub applied: Vec<(String, u64, bool)>,
    pub used: Vec<String>,
}

pub type Before = View;

impl View {
    #[must_use]
    pub fn of(resolver: &Resolver) -> Self {
        resolver.kernel_view()
    }
}

/// What is compared.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Summary {
    pub log: Vec<String>,
    pub applied: Vec<(String, u64, bool)>,
    pub used: Vec<String>,
    pub cancelled: bool,
    pub deviations: Vec<String>,
}

fn core_phase(p: Phase) -> CorePhase {
    match p {
        Phase::Strategy => CorePhase::Strategy,
        Phase::Action => CorePhase::Action,
        Phase::Status => CorePhase::Status,
        Phase::Agenda => CorePhase::Agenda,
    }
}

fn core_freq(f: Frequency) -> CFreq {
    match f {
        Frequency::OncePerTrigger => CFreq::OncePerTrigger,
        Frequency::OncePerTurn => CFreq::OncePerTurn,
        Frequency::OncePerRound => CFreq::OncePerRound,
        Frequency::Unlimited => CFreq::Unlimited,
    }
}

/// Compare the string resolver's emission with the core's replay of its trace. Only clean emissions are
/// replayed (an error or a nested emission the rules swallowed leaves nothing to compare).
pub fn check(resolver: &Resolver, before: &View, items: Vec<Item>, result: &Result<Event, TimingError>) {
    let Ok(event) = result else { return };
    if items.iter().any(|i| matches!(i, Item::EmitEnd { ok: false, .. })) {
        return;
    }
    let after = resolver.kernel_after(before.log_len, before.applied_len);
    let string = Summary {
        log: after.log.clone(),
        applied: after.applied.clone(),
        used: after.used.clone(),
        cancelled: event.cancelled,
        deviations: Vec::new(),
    };
    dual::run_dual_typed(Area::Kernel, "timing.emit", || Some(replay(before, &items)), || string);
}

fn replay(before: &View, items: &[Item]) -> Summary {
    // The owner table: every seat name the emission can mention, in text order.
    let mut names: BTreeSet<String> = BTreeSet::new();
    names.extend(before.initiative.iter().cloned());
    names.extend(before.seating.iter().cloned());
    names.extend(before.active.iter().cloned());
    names.extend(before.speaker.iter().cloned());
    names.extend(before.abilities.iter().map(|a| a.owner.clone()));
    for i in items {
        match i {
            Item::Part { owner, .. } | Item::Bars { owner, .. } => {
                names.insert(owner.clone());
            }
            Item::Skipped { player } | Item::Ask { player, .. } => {
                names.insert(player.clone());
            }
            _ => {}
        }
    }
    let r = Render::new(names.into_iter().collect());
    let seat = |s: &str| r.seat(s).expect("every name is in the owner table");
    let seats = |v: &[String]| v.iter().map(|s| seat(s)).collect::<Vec<Seat>>();

    // S1 (`docs/STRING_ENGINE_BUGS.md`): the string offers an action-phase window in its fixed initiative list ROTATED to start at the active seat; the
    // core offers the active seat first and then the others in initiative order (the user's ruling). Handed the list rotated the same way, the core's
    // order is the string's: this replay compares the two, it does not copy the bug into the core.
    let mut order = seats(&before.initiative);
    if !core_phase(before.phase).uses_speaker_order()
        && let Some(at) = before.active.as_deref().map(seat).and_then(|active| order.iter().position(|s| *s == active))
    {
        order.rotate_left(at);
    }
    let mut core = CoreResolver::new(&order, before.active.as_deref().map(seat));
    core.set_phase(core_phase(before.phase));
    core.set_seating_order(&seats(&before.seating));
    core.set_speaker(before.speaker.as_deref().map(seat));
    core.set_counters(before.round, before.turn);
    core.set_maximum_depth(before.max_depth);
    core.register(before.abilities.iter().enumerate().map(|(i, a)| {
        let mut def = AbilityDef::new(
            ability_id(&r, &a.id),
            seat(&a.owner),
            event_kind(&r, &a.event),
            if a.when { CRel::When } else { CRel::After },
        )
        .with_frequency(core_freq(a.frequency))
        .with_optional(a.optional)
        .with_repeatable(a.repeatable)
        .with_arg(u32::try_from(i).unwrap_or(u32::MAX));
        if a.has_condition {
            def = def.with_condition();
        }
        def
    }));
    for (i, label) in before.cannot.iter().enumerate() {
        core.forbid(CannotDef { label: r.code(label), arg: u32::try_from(i).unwrap_or(u32::MAX) });
    }
    for (id, scope) in &before.used {
        // `Trigger(5)` / `Turn(3)` / `Round(2)`
        let Some((kind, rest)) = scope.split_once('(') else { continue };
        let n: u64 = rest.trim_end_matches(')').parse().unwrap_or(0);
        let kind = match kind {
            "Trigger" => UsedScope::Trigger,
            "Turn" => UsedScope::Turn,
            _ => UsedScope::Round,
        };
        core.spend(ability_id(&r, id), kind, n);
    }
    let log_start = core.log().len();
    let applied_start = core.applied_events().len();
    let _ = (log_start, applied_start);

    let mut host = Replay { r: &r, items, pos: 0, ctx: Vec::new(), deviations: Vec::new(), state: CoreState::all_loaded() };
    let outcome = match host.next_emit(&mut core) {
        Ok(Some(rec)) => Some(rec),
        Ok(None) => None,
        Err(e) => {
            host.deviate(format!("core error: {e:?}"));
            None
        }
    };
    if host.pos != items.len() {
        let rest = items.get(host.pos).map(|i| format!("{i:?}")).unwrap_or_default();
        host.deviate(format!("trace not consumed at {}/{}: {rest}", host.pos, items.len()));
    }

    let who = |s: Seat| r.seat_text(s).to_owned();
    let log = core
        .log()
        .iter()
        .map(|line| match *line {
            Line::Emit { kind, id } => format!("emit {}#{id}", event_text(&r, kind)),
            Line::Cancelled { kind, id } => format!("  {}#{id} cancelled", event_text(&r, kind)),
            Line::Forbidden { kind, id, label } => {
                format!("  {}#{id} forbidden by {}", event_text(&r, kind), r.code_text(label))
            }
            Line::Resolve { kind, id } => format!("  resolve {}#{id}", event_text(&r, kind)),
            Line::Picked { relation, who: w, ability } => {
                format!("  [{}] {} -> {}", relation_text(relation), who(w), ability_text(&r, ability))
            }
            Line::Declines { relation, who: w } => format!("  [{}] {} declines", relation_text(relation), who(w)),
        })
        .collect();
    let applied = core.applied_events().iter().map(|e| (event_text(&r, e.kind), e.id, e.cancelled)).collect();
    let used = core
        .used_keys()
        .into_iter()
        .map(|(id, scope, n)| {
            let scope = match scope {
                UsedScope::Trigger => "Trigger",
                UsedScope::Turn => "Turn",
                UsedScope::Round => "Round",
            };
            format!("{}|{scope}({n})", ability_text(&r, id))
        })
        .collect();
    Summary { log, applied, used, cancelled: outcome.is_some_and(|e| e.cancelled), deviations: host.deviations }
}

/// Answers the core's questions from the recorded trace.
struct Replay<'a> {
    r: &'a Render,
    items: &'a [Item],
    pos: usize,
    /// Whether each open emission is a contextual one (`emit_with_context`).
    ctx: Vec<bool>,
    deviations: Vec<String>,
    state: CoreState,
}

impl Replay<'_> {
    fn deviate(&mut self, what: String) {
        if self.deviations.len() < 8 {
            self.deviations.push(what);
        }
    }

    fn peek(&self) -> Option<&Item> {
        self.items.get(self.pos)
    }

    fn take(&mut self) -> Option<Item> {
        let item = self.items.get(self.pos).cloned();
        if item.is_some() {
            self.pos += 1;
        }
        item
    }

    fn in_ctx(&self) -> bool {
        self.ctx.last().copied().unwrap_or(false)
    }

    /// Replay the emission at the cursor, and every emission it opens.
    fn next_emit(&mut self, core: &mut CoreResolver) -> Result<Option<EventRec>, CoreError> {
        let Some(Item::Emit { event, ctx }) = self.take() else {
            self.deviate("trace does not start with an emission".to_owned());
            return Ok(None);
        };
        let rec = EventRec {
            id: event.id,
            kind: event_kind(self.r, &event.event_type),
            payload: Payload::new(),
            cancelled: event.cancelled,
        };
        self.ctx.push(ctx);
        let out = core.emit(self, rec, |ev, host, resolver| host.resolve(ev, resolver));
        self.ctx.pop();
        let out = out?;
        match self.take() {
            Some(Item::EmitEnd { cancelled, .. }) => {
                if cancelled != out.cancelled {
                    self.deviate(format!("event {} cancelled: string {cancelled}, core {}", out.id, out.cancelled));
                }
            }
            other => self.deviate(format!("expected the end of emission {}, found {other:?}", out.id)),
        }
        Ok(Some(out))
    }

    /// The nested emissions between a begin marker and its end marker.
    fn nested(&mut self, resolver: &mut CoreResolver, end: impl Fn(&Item) -> bool) -> Result<Option<Item>, CoreError> {
        loop {
            match self.peek() {
                Some(Item::Emit { .. }) => {
                    self.next_emit(resolver)?;
                }
                Some(i) if end(i) => return Ok(self.take()),
                other => {
                    let what = format!("unexpected step inside a nested span: {other:?}");
                    self.deviate(what);
                    return Ok(None);
                }
            }
        }
    }

    fn resolve(&mut self, _ev: &mut EventRec, resolver: &mut CoreResolver) {
        match self.take() {
            Some(Item::Resolve) => {}
            other => {
                self.deviate(format!("expected the resolve step, found {other:?}"));
                return;
            }
        }
        if self.nested(resolver, |i| matches!(i, Item::ResolveEnd)).is_err() {
            self.deviate("nested emission failed in the core".to_owned());
        }
    }
}

impl Host for Replay<'_> {
    fn state(&self) -> &CoreState {
        &self.state
    }

    fn participates(&mut self, ability: &AbilityDef) -> bool {
        match self.take() {
            Some(Item::Part { ability: id, result, .. }) => {
                let want = ability_text(self.r, ability.id);
                if id != want {
                    self.deviate(format!("participation of {want}, trace has {id}"));
                }
                result
            }
            other => {
                self.deviate(format!("participation of {}, trace has {other:?}", ability_text(self.r, ability.id)));
                true
            }
        }
    }

    fn bars_faction_abilities(&mut self, who: Seat) -> bool {
        if !self.in_ctx() {
            return false;
        }
        match self.take() {
            Some(Item::Bars { owner, result }) => {
                if owner != self.r.seat_text(who) {
                    self.deviate(format!("bar check of {}, trace has {owner}", self.r.seat_text(who)));
                }
                result
            }
            other => {
                self.deviate(format!("bar check of {}, trace has {other:?}", self.r.seat_text(who)));
                false
            }
        }
    }

    fn condition(&mut self, ability: &AbilityDef, _: &EventRec, _: &CoreResolver) -> bool {
        match self.take() {
            Some(Item::Cond { ability: id, result }) => {
                let want = ability_text(self.r, ability.id);
                if id != want {
                    self.deviate(format!("condition of {want}, trace has {id}"));
                }
                result
            }
            other => {
                self.deviate(format!("condition of {}, trace has {other:?}", ability_text(self.r, ability.id)));
                true
            }
        }
    }

    fn forbids(&mut self, _: &CannotDef, _: &EventRec) -> bool {
        match self.take() {
            Some(Item::Forbids { result }) => result,
            other => {
                self.deviate(format!("prohibition check, trace has {other:?}"));
                false
            }
        }
    }

    fn effect(&mut self, ability: &AbilityDef, event: &mut EventRec, resolver: &mut CoreResolver) -> Result<(), CoreError> {
        match self.take() {
            Some(Item::EffectBegin { ability: id }) => {
                let want = ability_text(self.r, ability.id);
                if id != want {
                    self.deviate(format!("effect of {want}, trace has {id}"));
                }
            }
            other => {
                self.deviate(format!("effect of {}, trace has {other:?}", ability_text(self.r, ability.id)));
                return Ok(());
            }
        }
        if let Some(Item::EffectEnd { cancelled }) = self.nested(resolver, |i| matches!(i, Item::EffectEnd { .. }))? {
            if cancelled {
                event.cancelled = true;
            }
        }
        Ok(())
    }

    fn decide(&mut self, choice: &Choice, _: &EventRec, _: &[u64]) -> Pick {
        let options: Vec<String> = choice
            .opts
            .iter()
            .filter_map(|o| match o {
                Opt::Reaction(ti4_engine_core::choice::reaction::Opt::Ability(id)) => Some(ability_text(self.r, *id)),
                _ => None,
            })
            .collect();
        let player = self.r.seat_text(choice.actor).to_owned();
        let declinable = choice.can_decline();
        match self.take() {
            Some(Item::Ask { player: p, options: o, declinable: d, chosen }) => {
                if p != player || o != options || d != declinable {
                    self.deviate(format!(
                        "ask: core ({player}, {options:?}, {declinable}), string ({p}, {o:?}, {d})"
                    ));
                }
                let index = match chosen {
                    None => choice.decline_index(),
                    Some(text) => options.iter().position(|x| *x == text),
                };
                index.and_then(|i| choice.pick(i)).unwrap_or(Pick::Opt(choice::Opt::Decline))
            }
            other => {
                self.deviate(format!("ask ({player}, {options:?}), trace has {other:?}"));
                Pick::Opt(choice::Opt::Decline)
            }
        }
    }

    fn window_skipped(&mut self, who: Seat, _: &EventRec, _: CRel) {
        if !self.in_ctx() {
            return;
        }
        match self.take() {
            Some(Item::Skipped { player }) => {
                if player != self.r.seat_text(who) {
                    self.deviate(format!("window skipped for {}, trace has {player}", self.r.seat_text(who)));
                }
            }
            other => self.deviate(format!("window skipped for {}, trace has {other:?}", self.r.seat_text(who))),
        }
    }

    fn window_closed(&mut self, resolver: &mut CoreResolver) -> Result<(), CoreError> {
        if !self.in_ctx() {
            return Ok(());
        }
        match self.take() {
            Some(Item::Closed) => {}
            other => {
                self.deviate(format!("window close, trace has {other:?}"));
                return Ok(());
            }
        }
        self.nested(resolver, |i| matches!(i, Item::ClosedEnd))?;
        Ok(())
    }
}
