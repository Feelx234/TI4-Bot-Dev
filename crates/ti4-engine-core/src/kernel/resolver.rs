//! The timing resolver: the typed port of `ti4-engine/src/timing.rs`.
//!
//! Deterministic WHEN / AFTER windows around an emitted event (LRR 1.19/1.20): the registry of
//! abilities in registration order, the player order of the phase, eligibility (owner, participation,
//! "resolved in this window", frequency scope, the rule-specific condition), the question to the seat
//! when it has a choice, the absolute "cannot" effects, depth-first nested emission, and checkpoints
//! that undo what was added after them without copying the logs.
//!
//! The resolver is **data and ordering**. What an ability's condition and effect do, what a "cannot"
//! forbids, how a question is put and answered: all of that is the [`Host`]'s, so the same algorithm
//! runs under the real rules (the driver's host dispatches on [`AbilityId`]) and under the dual harness
//! (a shim answering from the string engine's trace).
//!
//! Nothing here allocates per window except the eligible list (a short `Vec` of copies).

use crate::choice::reaction::{AbilityId, AbilityKind, Ask, Frequency, Opt, Relation, Subtype};
use crate::choice::{self, Choice, Ctx, Opts, Origin, Pick};
use crate::containers::{Cow, SmallList};
use crate::ids::Seat;
use crate::state::{Code, CoreState, EventKind, EventRec, Phase};

/// A registered ability (`timing::Ability`) without its closures: identity and metadata. `condition` and
/// `effect` are the host's (`Host::condition`, `Host::effect`), selected by `id` and `arg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbilityDef {
    /// The registered slot, owner included (`ability:<owner>:<card>:<EVENT>:<rel>` in text).
    pub id: AbilityId,
    /// Who owns the ability.
    pub owner: Seat,
    /// Triggering event kind.
    pub event: EventKind,
    /// WHEN or AFTER.
    pub relation: Relation,
    /// Cross-window usage metadata.
    pub frequency: Frequency,
    /// Whether the decider may decline it.
    pub optional: bool,
    /// Whether several slots with this id may resolve in one window.
    pub repeatable: bool,
    /// Whether the host has a condition to evaluate (`false`: always eligible on the metadata).
    pub has_condition: bool,
    /// A word for the host (the data of a typed ability; the closure index in the dual shim).
    pub arg: u32,
}

impl AbilityDef {
    /// An unconditional ability with the oracle's defaults (once per trigger, mandatory, not repeatable).
    #[must_use]
    pub const fn new(id: AbilityId, owner: Seat, event: EventKind, relation: Relation) -> Self {
        Self {
            id,
            owner,
            event,
            relation,
            frequency: Frequency::OncePerTrigger,
            optional: false,
            repeatable: false,
            has_condition: false,
            arg: 0,
        }
    }

    #[must_use]
    pub const fn with_frequency(mut self, frequency: Frequency) -> Self {
        self.frequency = frequency;
        self
    }

    #[must_use]
    pub const fn with_optional(mut self, optional: bool) -> Self {
        self.optional = optional;
        self
    }

    #[must_use]
    pub const fn with_repeatable(mut self, repeatable: bool) -> Self {
        self.repeatable = repeatable;
        self
    }

    #[must_use]
    pub const fn with_condition(mut self) -> Self {
        self.has_condition = true;
        self
    }

    #[must_use]
    pub const fn with_arg(mut self, arg: u32) -> Self {
        self.arg = arg;
        self
    }
}

/// A registered absolute "cannot" effect (`timing::CannotEffect`): the label shown in the trace and a word
/// for the host's predicate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CannotDef {
    pub label: Code,
    pub arg: u32,
}

/// A resolver failure (`timing::TimingError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimingError {
    /// A decider chose an option the window did not offer.
    IllegalChoice,
    /// A nested event would exceed the resolver's depth budget.
    NestedEmissionDepthExceeded { maximum_depth: usize, chain: Vec<(EventKind, u64)> },
    /// A timing lifecycle counter cannot advance without losing replay identity.
    CounterExhausted(Counter),
    /// The host refused (a rule-specific failure; the host keeps the detail).
    Host(u32),
}

/// Which counter ran out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counter {
    Turn,
    Round,
}

/// One line of the resolution trace (`Resolver.log`, which the string engine keeps as text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// `emit KIND#id`
    Emit { kind: EventKind, id: u64 },
    /// `  KIND#id cancelled`
    Cancelled { kind: EventKind, id: u64 },
    /// `  KIND#id forbidden by <label>`
    Forbidden { kind: EventKind, id: u64, label: Code },
    /// `  resolve KIND#id`
    Resolve { kind: EventKind, id: u64 },
    /// `  [when] <seat> -> <ability>`
    Picked { relation: Relation, who: Seat, ability: AbilityId },
    /// `  [when] <seat> declines`
    Declines { relation: Relation, who: Seat },
}

/// The ordered registry of abilities and prohibitions. `by_event` is an index only: each bucket stores
/// insertion indexes, so retrieval preserves the registration order.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    abilities: Vec<AbilityDef>,
    cannot: Vec<CannotDef>,
    /// Sorted by `(event, relation)`.
    buckets: Vec<Bucket>,
}

#[derive(Debug, Clone)]
struct Bucket {
    event: EventKind,
    relation: Relation,
    indexes: Vec<u32>,
}

/// How much of a registry existed at a checkpoint (both parts only grow).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistryMark {
    abilities: usize,
    cannot: usize,
}

impl Registry {
    /// Register abilities in the supplied order. Duplicate ids are retained (the oracle's registry does
    /// not reject them).
    pub fn register(&mut self, abilities: impl IntoIterator<Item = AbilityDef>) {
        for ability in abilities {
            let index = u32::try_from(self.abilities.len()).expect("fewer than 2^32 abilities");
            let key = (ability.event, ability.relation);
            match self.buckets.binary_search_by(|b| (b.event, b.relation).cmp(&key)) {
                Ok(at) => self.buckets[at].indexes.push(index),
                Err(at) => self.buckets.insert(at, Bucket { event: key.0, relation: key.1, indexes: vec![index] }),
            }
            self.abilities.push(ability);
        }
    }

    /// Matching abilities in their original registration order.
    pub fn for_event(&self, event: EventKind, relation: Relation) -> impl Iterator<Item = &AbilityDef> {
        let key = (event, relation);
        self.buckets
            .binary_search_by(|b| (b.event, b.relation).cmp(&key))
            .ok()
            .into_iter()
            .flat_map(move |at| self.buckets[at].indexes.iter())
            .map(|&i| &self.abilities[i as usize])
    }

    /// Register an absolute "cannot" effect. Registrations are persistent.
    pub fn forbid(&mut self, cannot: CannotDef) {
        self.cannot.push(cannot);
    }

    /// The prohibitions, first registered first.
    #[must_use]
    pub fn cannots(&self) -> &[CannotDef] {
        &self.cannot
    }

    /// Number of registered abilities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.abilities.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.abilities.is_empty()
    }

    fn mark(&self) -> RegistryMark {
        RegistryMark { abilities: self.abilities.len(), cannot: self.cannot.len() }
    }

    /// Drop everything registered after `mark`.
    fn truncate_to(&mut self, mark: RegistryMark) {
        debug_assert!(self.abilities.len() >= mark.abilities && self.cannot.len() >= mark.cannot);
        self.cannot.truncate(mark.cannot);
        if self.abilities.len() > mark.abilities {
            self.abilities.truncate(mark.abilities);
            let keep = u32::try_from(mark.abilities).expect("fewer than 2^32 abilities");
            for b in &mut self.buckets {
                b.indexes.retain(|&i| i < keep);
            }
            self.buckets.retain(|b| !b.indexes.is_empty());
        }
    }
}

/// The scope in which a frequency key was spent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Scope {
    Trigger(u64),
    Turn(u64),
    Round(u64),
}

type UsedKey = (AbilityId, Scope);

/// Frequency keys already spent, with their insertion order so a checkpoint can undo the keys added
/// after it. Linear scan: a game spends a few hundred keys at most and most lookups hit the front.
#[derive(Debug, Clone, Default)]
struct UsedKeys {
    order: Vec<UsedKey>,
}

impl UsedKeys {
    fn contains(&self, key: &UsedKey) -> bool {
        self.order.iter().rev().any(|k| k == key)
    }

    fn insert(&mut self, key: UsedKey) {
        if !self.contains(&key) {
            self.order.push(key);
        }
    }

    fn undo_to(&mut self, len: usize) {
        self.order.truncate(len);
    }
}

/// What the resolver asks of the rules.
pub trait Host {
    /// The state the conditions read (the host owns the mutable state its effects change).
    fn state(&self) -> &CoreState;

    /// Whether this ability participates at all (a fork binding that stops a seat's reactions). Checked
    /// before the condition. Default: yes.
    fn participates(&mut self, _ability: &AbilityDef) -> bool {
        true
    }

    /// Political Secret (version 17): no faction ability, no action card, for this seat. Checked for
    /// `ability:` slots before the condition. Default: not barred.
    fn bars_faction_abilities(&mut self, _who: Seat) -> bool {
        false
    }

    /// The rule-specific eligibility of an ability that has a condition (both string kinds, in order).
    fn condition(&mut self, ability: &AbilityDef, event: &EventRec, resolver: &Resolver) -> bool;

    /// Whether a prohibition forbids the event.
    fn forbids(&mut self, cannot: &CannotDef, event: &EventRec) -> bool;

    /// The effect of a selected ability. It may cancel the event and emit nested events.
    ///
    /// # Errors
    /// Whatever the rule refuses with.
    fn effect(&mut self, ability: &AbilityDef, event: &mut EventRec, resolver: &mut Resolver) -> Result<(), TimingError>;

    /// Put the window's question and take the answer. The default asks nobody and takes the first option.
    fn decide(&mut self, choice: &Choice, _event: &EventRec, _chain: &[u64]) -> Pick {
        choice.pick(0).unwrap_or(Pick::Opt(choice::Opt::Decline))
    }

    /// A seat that has nothing to resolve was looked at once in this window (`Table::window_skipped`).
    fn window_skipped(&mut self, _who: Seat, _event: &EventRec, _relation: Relation) {}

    /// After a window's loop ended cleanly: announce the cards declared in it (rules version 3).
    ///
    /// # Errors
    /// Whatever the announcement refuses with.
    fn window_closed(&mut self, _resolver: &mut Resolver) -> Result<(), TimingError> {
        Ok(())
    }
}

/// How the journal of applied events looked at a checkpoint: appended-to only, except that an event's
/// own entry is rewritten when its emission ends, so a checkpoint taken while an emission is open keeps
/// a copy.
#[derive(Debug, Clone)]
enum JournalMark {
    Prefix(usize),
    Full(Vec<EventRec>),
}

/// A checkpoint of the resolver (the registry, the logs and the used keys are marked, not copied).
#[derive(Debug, Clone)]
pub struct Checkpoint {
    registry: RegistryMark,
    initiative_order: SmallList<Seat, 8>,
    seating_order: SmallList<Seat, 8>,
    active_player: Option<Seat>,
    speaker: Option<Seat>,
    phase: Phase,
    log_len: usize,
    applied: JournalMark,
    relation_being_resolved: Option<Relation>,
    emission_stack: Vec<(EventKind, u64)>,
    maximum_depth: usize,
    used: usize,
    round_number: u64,
    turn_number: u64,
}

/// A timing-window resolver.
#[derive(Debug, Clone)]
pub struct Resolver {
    registry: Cow<Registry>,
    initiative_order: SmallList<Seat, 8>,
    seating_order: SmallList<Seat, 8>,
    active_player: Option<Seat>,
    speaker: Option<Seat>,
    phase: Phase,
    log: Vec<Line>,
    applied_events: Vec<EventRec>,
    relation_being_resolved: Option<Relation>,
    emission_stack: Vec<(EventKind, u64)>,
    maximum_depth: usize,
    used: UsedKeys,
    round_number: u64,
    turn_number: u64,
}

impl Resolver {
    /// Default maximum count of simultaneously resolving events.
    pub const DEFAULT_MAXIMUM_DEPTH: usize = 100;

    /// An action-phase resolver with the supplied initiative order.
    #[must_use]
    pub fn new(initiative_order: &[Seat], active_player: Option<Seat>) -> Self {
        Self {
            registry: Cow::new(Registry::default()),
            initiative_order: SmallList::from_slice(initiative_order),
            seating_order: SmallList::new(),
            active_player,
            speaker: None,
            phase: Phase::Action,
            log: Vec::new(),
            applied_events: Vec::new(),
            relation_being_resolved: None,
            emission_stack: Vec::new(),
            maximum_depth: Self::DEFAULT_MAXIMUM_DEPTH,
            used: UsedKeys::default(),
            round_number: 1,
            turn_number: 1,
        }
    }

    // -- configuration ---------------------------------------------------------------------------

    pub fn set_phase(&mut self, phase: Phase) {
        self.phase = phase;
    }

    pub fn set_seating_order(&mut self, seating_order: &[Seat]) {
        self.seating_order = SmallList::from_slice(seating_order);
    }

    pub fn set_initiative_order(&mut self, initiative_order: &[Seat]) {
        self.initiative_order = SmallList::from_slice(initiative_order);
    }

    pub fn set_active_player(&mut self, active_player: Option<Seat>) {
        self.active_player = active_player;
    }

    pub fn set_speaker(&mut self, speaker: Option<Seat>) {
        self.speaker = speaker;
    }

    /// Set the maximum simultaneous event depth.
    ///
    /// # Panics
    /// Panics if `maximum_depth` is zero.
    pub fn set_maximum_depth(&mut self, maximum_depth: usize) {
        assert!(maximum_depth > 0, "a resolver needs room for its root event");
        self.maximum_depth = maximum_depth;
    }

    /// Copy the driver's frequency-scope counters before an emission.
    pub fn sync_lifecycle(&mut self, round_number: u32, turn_number: u32) {
        self.round_number = u64::from(round_number);
        self.turn_number = u64::from(turn_number);
    }

    /// Set the frequency counters outright (restoring a resolver from another representation).
    pub fn set_counters(&mut self, round_number: u64, turn_number: u64) {
        self.round_number = round_number;
        self.turn_number = turn_number;
    }

    /// Mark a frequency key as spent (restoring the used keys of another representation, in order).
    pub fn spend(&mut self, ability: AbilityId, scope: UsedScope, counter: u64) {
        let scope = match scope {
            UsedScope::Trigger => Scope::Trigger(counter),
            UsedScope::Turn => Scope::Turn(counter),
            UsedScope::Round => Scope::Round(counter),
        };
        self.used.insert((ability, scope));
    }

    /// The maximum simultaneous event depth.
    #[must_use]
    pub const fn maximum_depth(&self) -> usize {
        self.maximum_depth
    }

    /// Advance the turn counter and change the active player.
    ///
    /// # Errors
    /// [`TimingError::CounterExhausted`] instead of wrapping.
    pub fn begin_turn(&mut self, active_player: Seat) -> Result<(), TimingError> {
        self.turn_number = self.turn_number.checked_add(1).ok_or(TimingError::CounterExhausted(Counter::Turn))?;
        self.active_player = Some(active_player);
        Ok(())
    }

    /// Advance the round and turn counters (existing used keys keep their old counter and so no longer
    /// match).
    ///
    /// # Errors
    /// [`TimingError::CounterExhausted`] instead of wrapping.
    pub fn begin_round(&mut self) -> Result<(), TimingError> {
        let round = self.round_number.checked_add(1).ok_or(TimingError::CounterExhausted(Counter::Round))?;
        let turn = self.turn_number.checked_add(1).ok_or(TimingError::CounterExhausted(Counter::Turn))?;
        self.round_number = round;
        self.turn_number = turn;
        Ok(())
    }

    // -- reading ---------------------------------------------------------------------------------

    /// The relation currently being resolved, if a timing window is open.
    #[must_use]
    pub const fn relation_being_resolved(&self) -> Option<Relation> {
        self.relation_being_resolved
    }

    /// The resolution trace so far.
    #[must_use]
    pub fn log(&self) -> &[Line] {
        &self.log
    }

    /// Events after WHEN effects, cancellation, resolution and AFTER effects.
    #[must_use]
    pub fn applied_events(&self) -> &[EventRec] {
        &self.applied_events
    }

    /// The registry.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Matching abilities in registration order.
    pub fn for_event(&self, event: EventKind, relation: Relation) -> impl Iterator<Item = &AbilityDef> {
        self.registry.for_event(event, relation)
    }

    /// Ids of the events being resolved right now, outermost first, `event` itself excluded: what a
    /// reaction to a reaction was nested inside.
    #[must_use]
    pub fn emission_chain(&self, event: &EventRec) -> Vec<u64> {
        self.emission_stack.iter().map(|(_, id)| *id).filter(|id| *id != event.id).collect()
    }

    /// The open emissions, outermost first.
    #[must_use]
    pub fn emission_stack(&self) -> &[(EventKind, u64)] {
        &self.emission_stack
    }

    // -- registration ----------------------------------------------------------------------------

    /// Register abilities in the supplied order.
    pub fn register(&mut self, abilities: impl IntoIterator<Item = AbilityDef>) {
        self.registry.edit().register(abilities);
    }

    /// Register an absolute "cannot" effect. Registrations are intentionally persistent.
    pub fn forbid(&mut self, cannot: CannotDef) {
        self.registry.edit().forbid(cannot);
    }

    /// The first registered prohibition that matches the event.
    pub fn is_forbidden<H: Host + ?Sized>(&self, host: &mut H, event: &EventRec) -> Option<CannotDef> {
        self.registry.cannots().iter().copied().find(|c| host.forbids(c, event))
    }

    // -- checkpoints -----------------------------------------------------------------------------

    /// Mark the resolver so [`Resolver::restore`] can undo what is added after it.
    #[must_use]
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            registry: self.registry.mark(),
            initiative_order: self.initiative_order.clone(),
            seating_order: self.seating_order.clone(),
            active_player: self.active_player,
            speaker: self.speaker,
            phase: self.phase,
            log_len: self.log.len(),
            applied: if self.emission_stack.is_empty() {
                JournalMark::Prefix(self.applied_events.len())
            } else {
                JournalMark::Full(self.applied_events.clone())
            },
            relation_being_resolved: self.relation_being_resolved,
            emission_stack: self.emission_stack.clone(),
            maximum_depth: self.maximum_depth,
            used: self.used.order.len(),
            round_number: self.round_number,
            turn_number: self.turn_number,
        }
    }

    /// Undo everything after `checkpoint`.
    pub fn restore(&mut self, checkpoint: Checkpoint) {
        if self.registry.mark() != checkpoint.registry {
            self.registry.edit().truncate_to(checkpoint.registry);
        }
        self.initiative_order = checkpoint.initiative_order;
        self.seating_order = checkpoint.seating_order;
        self.active_player = checkpoint.active_player;
        self.speaker = checkpoint.speaker;
        self.phase = checkpoint.phase;
        debug_assert!(self.log.len() >= checkpoint.log_len);
        self.log.truncate(checkpoint.log_len);
        match checkpoint.applied {
            JournalMark::Prefix(len) => {
                debug_assert!(self.applied_events.len() >= len);
                self.applied_events.truncate(len);
            }
            JournalMark::Full(events) => self.applied_events = events,
        }
        self.relation_being_resolved = checkpoint.relation_being_resolved;
        self.emission_stack = checkpoint.emission_stack;
        self.maximum_depth = checkpoint.maximum_depth;
        self.used.undo_to(checkpoint.used);
        self.round_number = checkpoint.round_number;
        self.turn_number = checkpoint.turn_number;
    }

    // -- emission --------------------------------------------------------------------------------

    /// Emit an event through WHEN, ordinary resolution (`resolve`) and AFTER windows.
    ///
    /// # Errors
    /// An illegal decider answer, an exhausted depth budget, or whatever the host refuses with.
    pub fn emit<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        mut event: EventRec,
        resolve: impl FnOnce(&mut EventRec, &mut H, &mut Resolver),
    ) -> Result<EventRec, TimingError> {
        if self.emission_stack.len() == self.maximum_depth {
            return Err(TimingError::NestedEmissionDepthExceeded {
                maximum_depth: self.maximum_depth,
                chain: self.emission_stack.clone(),
            });
        }
        self.emission_stack.push((event.kind, event.id));
        // Reserve the event's position before opening windows so nested emissions follow their parent in
        // emission order even though the nested event completes first.
        let journal_index = self.applied_events.len();
        self.applied_events.push(event.clone());
        self.log.push(Line::Emit { kind: event.kind, id: event.id });
        let result = (|| {
            self.run_window(host, &mut event, Relation::When)?;
            if event.cancelled {
                self.log.push(Line::Cancelled { kind: event.kind, id: event.id });
                return Ok(());
            }
            if let Some(cannot) = self.is_forbidden(host, &event) {
                event.cancelled = true;
                self.log.push(Line::Forbidden { kind: event.kind, id: event.id, label: cannot.label });
                return Ok(());
            }
            self.log.push(Line::Resolve { kind: event.kind, id: event.id });
            resolve(&mut event, host, self);
            self.run_window(host, &mut event, Relation::After)?;
            Ok(())
        })();
        self.emission_stack.pop();
        match result {
            Ok(()) => {
                self.applied_events[journal_index] = event.clone();
                Ok(event)
            }
            Err(e) => {
                self.applied_events.remove(journal_index);
                Err(e)
            }
        }
    }

    fn run_window<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        event: &mut EventRec,
        relation: Relation,
    ) -> Result<(), TimingError> {
        let previous = self.relation_being_resolved;
        self.relation_being_resolved = Some(relation);
        let mut resolved_here: Vec<AbilityId> = Vec::new();
        let result = (|| {
            let mut passed = SeatBits::default();
            // Seats already looked at once in this window (`Table::window_skipped`).
            let mut announced = SeatBits::default();
            loop {
                let mut resolved_this_pass = false;
                for player in self.player_order().iter().copied() {
                    if passed.has(player) {
                        continue;
                    }
                    let eligible = self.eligible(host, event, player, relation, &resolved_here);
                    if eligible.is_empty() {
                        if announced.insert(player) {
                            host.window_skipped(player, event, relation);
                        }
                        continue;
                    }
                    announced.insert(player);
                    let Some(ability) = self.pick(host, eligible, event, player, relation)? else {
                        passed.insert(player);
                        continue;
                    };
                    resolved_here.push(ability.id);
                    self.mark_used(&ability, event);
                    self.log.push(Line::Picked { relation, who: player, ability: ability.id });
                    host.effect(&ability, event, self)?;
                    resolved_this_pass = true;
                    if event.cancelled {
                        return Ok(());
                    }
                }
                if !resolved_this_pass {
                    return Ok(());
                }
                passed = SeatBits::default();
            }
        })();
        // Rules version 3 (`RULES_SABOTAGE_TIMING`): the cards the players declared in this window are
        // announced only now, once every declaration is in.
        let result = result.and_then(|()| host.window_closed(self));
        self.relation_being_resolved = previous;
        result
    }

    fn eligible<H: Host + ?Sized>(
        &self,
        host: &mut H,
        event: &EventRec,
        player: Seat,
        relation: Relation,
        resolved_here: &[AbilityId],
    ) -> Vec<AbilityDef> {
        let mut out = Vec::new();
        for ability in self.registry.for_event(event.kind, relation) {
            if ability.owner == player
                && host.participates(ability)
                && (ability.repeatable || !resolved_here.contains(&ability.id))
                && !self.is_used(ability, event)
                && !(ability.id.kind == AbilityKind::Ability && host.bars_faction_abilities(player))
                && (!ability.has_condition || host.condition(ability, event, self))
            {
                out.push(*ability);
            }
        }
        out
    }

    fn pick<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        eligible: Vec<AbilityDef>,
        event: &EventRec,
        player: Seat,
        relation: Relation,
    ) -> Result<Option<AbilityDef>, TimingError> {
        if eligible.len() == 1 && !eligible[0].optional {
            return Ok(eligible.into_iter().next());
        }
        let declinable = eligible.iter().any(|a| a.optional);
        let mut opts: Opts = eligible.iter().map(|a| choice::Opt::Reaction(Opt::Ability(a.id))).collect();
        if declinable {
            opts.push(choice::Opt::Decline);
        }
        let ask = Ask::Window { event: event.kind, relation };
        let choice = Choice {
            actor: player,
            ask: choice::Ask::Reaction(ask),
            ctx: Ctx {
                origin: Origin::Reaction(event.kind),
                subtype: choice::Subtype::Reaction(Subtype::Window { event: event.kind, relation }),
                optional: declinable,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts,
        };
        let chain = self.emission_chain(event);
        let chosen = host.decide(&choice, event, &chain);
        let chosen = match chosen {
            Pick::Opt(o) if choice.opts.index_of(&o).is_some() => o,
            _ => return Err(TimingError::IllegalChoice),
        };
        if chosen.is_decline() {
            self.log.push(Line::Declines { relation, who: player });
            return Ok(None);
        }
        let choice::Opt::Reaction(Opt::Ability(id)) = chosen else {
            return Err(TimingError::IllegalChoice);
        };
        Ok(eligible.into_iter().find(|a| a.id == id))
    }

    /// The seats in the order a window offers them: from the speaker in strategy and agenda windows,
    /// from the active player (in initiative order) otherwise.
    #[must_use]
    pub fn player_order(&self) -> SmallList<Seat, 8> {
        let (sequence, first) = if self.phase.uses_speaker_order() {
            let sequence = if self.seating_order.is_empty() { &self.initiative_order } else { &self.seating_order };
            (sequence, self.speaker)
        } else {
            (&self.initiative_order, self.active_player)
        };
        let seq = sequence.as_slice();
        let Some(first) = first else {
            return sequence.clone();
        };
        let Some(index) = seq.iter().position(|p| *p == first) else {
            let mut order = SmallList::new();
            order.push(first);
            for p in seq.iter().filter(|p| **p != first) {
                order.push(*p);
            }
            return order;
        };
        seq[index..].iter().chain(&seq[..index]).copied().collect()
    }

    fn scope_key(&self, ability: &AbilityDef, event: &EventRec) -> Option<UsedKey> {
        let scope = match ability.frequency {
            Frequency::OncePerTrigger => Scope::Trigger(event.id),
            Frequency::OncePerTurn => Scope::Turn(self.turn_number),
            Frequency::OncePerRound => Scope::Round(self.round_number),
            Frequency::Unlimited => return None,
        };
        Some((ability.id, scope))
    }

    fn is_used(&self, ability: &AbilityDef, event: &EventRec) -> bool {
        self.scope_key(ability, event).is_some_and(|key| self.used.contains(&key))
    }

    fn mark_used(&mut self, ability: &AbilityDef, event: &EventRec) {
        if let Some(key) = self.scope_key(ability, event) {
            self.used.insert(key);
        }
    }

    /// Frequency keys spent so far, as `(ability, scope kind, counter)` in the order they were spent
    /// (the dual harness compares this).
    #[must_use]
    pub fn used_keys(&self) -> Vec<(AbilityId, UsedScope, u64)> {
        self.used
            .order
            .iter()
            .map(|(id, scope)| match scope {
                Scope::Trigger(n) => (*id, UsedScope::Trigger, *n),
                Scope::Turn(n) => (*id, UsedScope::Turn, *n),
                Scope::Round(n) => (*id, UsedScope::Round, *n),
            })
            .collect()
    }
}

/// The scope of a spent frequency key (read side of the private scope).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UsedScope {
    Trigger,
    Turn,
    Round,
}

/// A set of seats (the 16 owners fit a `u16`).
#[derive(Debug, Clone, Copy, Default)]
struct SeatBits(u16);

impl SeatBits {
    fn has(self, s: Seat) -> bool {
        self.0 & s.bit() != 0
    }

    fn insert(&mut self, s: Seat) -> bool {
        let new = !self.has(s);
        self.0 |= s.bit();
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::reaction::AbilityKind;

    fn id(n: u16) -> AbilityId {
        AbilityId { kind: AbilityKind::Other, key: Code(n) }
    }

    fn ev(id: u64, kind: EventKind) -> EventRec {
        EventRec { id, kind, payload: crate::state::Payload::new(), cancelled: false }
    }

    /// A host that records effects, answers conditions from a set, and picks by script.
    #[derive(Default)]
    struct Script {
        state: CoreState,
        effects: Vec<(Seat, AbilityId)>,
        false_conditions: Vec<AbilityId>,
        answers: Vec<usize>,
        asked: Vec<usize>,
        cancel_on: Option<AbilityId>,
        nested: Option<(AbilityId, EventKind)>,
    }

    impl Host for Script {
        fn state(&self) -> &CoreState {
            &self.state
        }
        fn condition(&mut self, a: &AbilityDef, _: &EventRec, _: &Resolver) -> bool {
            !self.false_conditions.contains(&a.id)
        }
        fn forbids(&mut self, c: &CannotDef, e: &EventRec) -> bool {
            (c.arg == 1 && e.kind == EventKind::ShipMoved)
        }
        fn effect(&mut self, a: &AbilityDef, e: &mut EventRec, r: &mut Resolver) -> Result<(), TimingError> {
            self.effects.push((a.owner, a.id));
            if self.cancel_on == Some(a.id) {
                e.cancelled = true;
            }
            if let Some((who, kind)) = self.nested.filter(|(w, _)| *w == a.id) {
                let _ = who;
                self.nested = None;
                r.emit(self, ev(e.id + 100, kind), |_, _, _| {})?;
            }
            Ok(())
        }
        fn decide(&mut self, choice: &Choice, _: &EventRec, _: &[u64]) -> Pick {
            let i = if self.answers.is_empty() { choice.opts.len() - 1 } else { self.answers.remove(0) };
            self.asked.push(choice.opts.len());
            choice.pick(i).unwrap()
        }
    }

    fn def(n: u16, owner: u8, kind: EventKind, rel: Relation) -> AbilityDef {
        AbilityDef::new(id(n), Seat(owner), kind, rel)
    }

    fn resolver() -> Resolver {
        Resolver::new(&[Seat(0), Seat(1), Seat(2)], Some(Seat(0)))
    }

    #[test]
    fn registration_is_partitioned_by_event_and_relation_without_reordering() {
        let mut r = resolver();
        r.register([
            def(1, 0, EventKind::ShipMoved, Relation::After),
            def(2, 1, EventKind::ShipMoved, Relation::When),
            def(3, 2, EventKind::ShipMoved, Relation::After),
            def(4, 0, EventKind::TurnBegan, Relation::After),
        ]);
        let after: Vec<_> = r.for_event(EventKind::ShipMoved, Relation::After).map(|a| a.id).collect();
        assert_eq!(after, vec![id(1), id(3)]);
        assert_eq!(r.for_event(EventKind::TurnBegan, Relation::When).count(), 0);
    }

    #[test]
    fn when_precedes_resolution_precedes_after_and_players_go_in_initiative_order() {
        let mut r = resolver();
        r.set_active_player(Some(Seat(1)));
        r.register([
            def(1, 0, EventKind::ShipMoved, Relation::After),
            def(2, 1, EventKind::ShipMoved, Relation::After),
            def(3, 2, EventKind::ShipMoved, Relation::When),
        ]);
        let mut h = Script::default();
        let mut order = Vec::new();
        r.emit(&mut h, ev(1, EventKind::ShipMoved), |_, h, _| order.push(h.effects.len())).unwrap();
        // WHEN effect first (1 effect done at resolution), then AFTER from seat 1, then seat 2's turn
        // comes before seat 0 (rotation 1,2,0).
        assert_eq!(order, vec![1]);
        assert_eq!(h.effects, vec![(Seat(2), id(3)), (Seat(1), id(2)), (Seat(0), id(1))]);
        assert_eq!(r.applied_events().len(), 1);
        assert!(matches!(r.log()[0], Line::Emit { id: 1, .. }));
    }

    #[test]
    fn strategy_and_agenda_windows_rotate_from_the_speaker() {
        let mut r = resolver();
        r.set_phase(Phase::Agenda);
        r.set_seating_order(&[Seat(0), Seat(1), Seat(2)]);
        r.set_speaker(Some(Seat(2)));
        assert_eq!(r.player_order().as_slice(), &[Seat(2), Seat(0), Seat(1)]);
        r.set_speaker(Some(Seat(7)));
        assert_eq!(r.player_order().as_slice(), &[Seat(7), Seat(0), Seat(1), Seat(2)]);
    }

    #[test]
    fn a_pass_is_reoffered_after_another_player_resolves() {
        let mut r = resolver();
        let opt = |n, o| def(n, o, EventKind::TurnBegan, Relation::After).with_optional(true).with_frequency(Frequency::Unlimited).with_repeatable(true);
        r.register([opt(1, 0), opt(2, 1)]);
        // seat 0 declines (index 1 = decline), seat 1 resolves, seat 0 is offered nothing more (passed).
        let mut h = Script { answers: vec![1, 0, 1], ..Script::default() };
        r.emit(&mut h, ev(1, EventKind::TurnBegan), |_, _, _| {}).unwrap();
        assert_eq!(h.effects[0], (Seat(1), id(2)));
        assert!(r.log().iter().any(|l| matches!(l, Line::Declines { who: Seat(0), .. })));
    }

    #[test]
    fn a_when_cancellation_skips_resolution_and_after() {
        let mut r = resolver();
        r.register([
            def(1, 0, EventKind::ShipMoved, Relation::When),
            def(2, 0, EventKind::ShipMoved, Relation::After),
        ]);
        let mut h = Script { cancel_on: Some(id(1)), ..Script::default() };
        let mut resolved = false;
        let out = r.emit(&mut h, ev(1, EventKind::ShipMoved), |_, _, _| resolved = true).unwrap();
        assert!(out.cancelled && !resolved);
        assert_eq!(h.effects.len(), 1);
        assert!(matches!(r.log().last(), Some(Line::Cancelled { id: 1, .. })));
    }

    #[test]
    fn a_cannot_cancels_the_event_before_resolution() {
        let mut r = resolver();
        r.forbid(CannotDef { label: Code(9), arg: 1 });
        r.register([def(2, 0, EventKind::ShipMoved, Relation::After)]);
        let mut h = Script::default();
        let mut resolved = false;
        let out = r.emit(&mut h, ev(1, EventKind::ShipMoved), |_, _, _| resolved = true).unwrap();
        assert!(out.cancelled && !resolved && h.effects.is_empty());
        assert!(matches!(r.log().last(), Some(Line::Forbidden { label: Code(9), .. })));
    }

    #[test]
    fn nested_events_resolve_depth_first_and_depth_is_bounded() {
        let mut r = resolver();
        r.register([
            def(1, 0, EventKind::ShipMoved, Relation::When),
            def(2, 0, EventKind::TurnBegan, Relation::When),
        ]);
        let mut h = Script { nested: Some((id(1), EventKind::TurnBegan)), ..Script::default() };
        r.emit(&mut h, ev(1, EventKind::ShipMoved), |_, _, _| {}).unwrap();
        assert_eq!(h.effects, vec![(Seat(0), id(1)), (Seat(0), id(2))]);
        let journal: Vec<u64> = r.applied_events().iter().map(|e| e.id).collect();
        assert_eq!(journal, vec![1, 101], "the parent keeps its place ahead of the nested event");

        let mut r = resolver();
        r.set_maximum_depth(1);
        r.register([def(1, 0, EventKind::ShipMoved, Relation::When)]);
        let mut h = Script { nested: Some((id(1), EventKind::TurnBegan)), ..Script::default() };
        let err = r.emit(&mut h, ev(1, EventKind::ShipMoved), |_, _, _| {}).unwrap_err();
        assert!(matches!(err, TimingError::NestedEmissionDepthExceeded { maximum_depth: 1, .. }));
        assert!(r.applied_events().is_empty() && r.emission_stack().is_empty());
    }

    #[test]
    fn frequency_scopes_lapse_with_their_counters() {
        let mut r = resolver();
        let once = |n, f| def(n, 0, EventKind::TurnBegan, Relation::After).with_frequency(f);
        r.register([once(1, Frequency::OncePerTrigger), once(2, Frequency::OncePerTurn), once(3, Frequency::OncePerRound), once(4, Frequency::Unlimited)]);
        let mut h = Script::default();
        r.emit(&mut h, ev(1, EventKind::TurnBegan), |_, _, _| {}).unwrap();
        assert_eq!(h.effects.len(), 4);
        h.effects.clear();
        r.emit(&mut h, ev(2, EventKind::TurnBegan), |_, _, _| {}).unwrap();
        // trigger-scoped lapses with a new event id; turn and round keys persist; unlimited fires again
        let mut got = h.effects.iter().map(|e| e.1).collect::<Vec<_>>();
        got.sort();
        assert_eq!(got, vec![id(1), id(4)]);
        h.effects.clear();
        r.begin_turn(Seat(1)).unwrap();
        r.emit(&mut h, ev(3, EventKind::TurnBegan), |_, _, _| {}).unwrap();
        assert!(h.effects.iter().any(|e| e.1 == id(2)) && !h.effects.iter().any(|e| e.1 == id(3)));
        h.effects.clear();
        r.begin_round().unwrap();
        r.emit(&mut h, ev(4, EventKind::TurnBegan), |_, _, _| {}).unwrap();
        assert!(h.effects.iter().any(|e| e.1 == id(3)));
    }

    #[test]
    fn a_false_condition_or_a_political_secret_removes_the_ability() {
        let mut r = resolver();
        r.register([
            def(1, 0, EventKind::TurnBegan, Relation::After).with_condition(),
            AbilityDef::new(AbilityId { kind: AbilityKind::Ability, key: Code(2) }, Seat(0), EventKind::TurnBegan, Relation::After),
        ]);
        let mut h = Script { false_conditions: vec![id(1)], ..Script::default() };
        r.emit(&mut h, ev(1, EventKind::TurnBegan), |_, _, _| {}).unwrap();
        assert_eq!(h.effects.len(), 1, "the unconditional ability only");
    }

    #[test]
    fn checkpoint_undoes_what_was_added_after_it() {
        let mut r = resolver();
        r.register([def(1, 0, EventKind::TurnBegan, Relation::After)]);
        let mut h = Script::default();
        let cp = r.checkpoint();
        r.register([def(2, 1, EventKind::TurnBegan, Relation::After)]);
        r.forbid(CannotDef { label: Code(1), arg: 5 });
        r.emit(&mut h, ev(1, EventKind::TurnBegan), |_, _, _| {}).unwrap();
        assert_eq!(r.registry().len(), 2);
        r.restore(cp);
        assert_eq!(r.registry().len(), 1);
        assert!(r.registry().cannots().is_empty());
        assert!(r.log().is_empty() && r.applied_events().is_empty() && r.used_keys().is_empty());
        assert_eq!(r.for_event(EventKind::TurnBegan, Relation::After).count(), 1);
    }

    #[test]
    fn one_mandatory_ability_is_not_asked_but_a_choice_is() {
        let mut r = resolver();
        r.register([def(1, 0, EventKind::TurnBegan, Relation::After), def(2, 0, EventKind::TurnBegan, Relation::After)]);
        let mut h = Script { answers: vec![1], ..Script::default() };
        r.emit(&mut h, ev(1, EventKind::TurnBegan), |_, _, _| {}).unwrap();
        assert_eq!(h.asked, vec![2]);
        assert_eq!(h.effects[0].1, id(2));
    }
}
