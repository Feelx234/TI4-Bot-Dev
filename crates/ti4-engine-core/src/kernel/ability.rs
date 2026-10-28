//! The ability table (package `core-reactions`; docs/ENGINE_REACTIONS.md): the rules behind the abilities a
//! reaction window offers.
//!
//! A timing window is the resolver's bucket `(EventKind, Relation)`. What the resolver stores for an ability is
//! its metadata ([`AbilityDef`]); what the ability *does* is a [`Spec`] in the [`AbilityTable`], found by the
//! [`Slot`] carried in `AbilityDef::arg`. A spec is both halves: [`Spec::def`] is the registry row of one owner
//! and `condition`/`effect` are the rule, so there is one place per ability (S8's `AbilityDef` + `EffectId`).
//!
//! [`AbilityTable::registry`] builds the registry of a game in the string order of `reactions::arm` (per seat,
//! the specs in table order), so the offer order of every window equals the string's. The table need not be
//! complete for a window to be followed: an ability whose condition does not hold changes nothing, so it may be
//! missing; one that is eligible must be in the table ([`AbilityTable::knows`]).

use super::resolver::{AbilityDef, CannotDef, Host, Resolver, TimingError};
use super::{Asker, Kernel};
use crate::choice::reaction::{AbilityId, AbilityKind, Frequency, Relation};
use crate::choice::{Choice, Pick};
use crate::generated::Faction;
use crate::ids::Seat;
use crate::state::{CoreState, EventKind, EventRec, RngDomain, SeedSource};

/// The identity of an ability *rule*, independent of its owner (`AbilityDef::arg`). Each crate that ports
/// abilities owns a range: the kernel `0..1000`, the faction packages `1000..` in steps of 1000.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slot(pub u16);

impl Slot {
    /// `AbilityDef::arg` of an ability the table does not know (the adapter's mapping found no slot).
    pub const UNPORTED: u32 = u32::MAX;

    #[must_use]
    pub const fn arg(self) -> u32 {
        self.0 as u32
    }
}

/// Who gets the registry row: the string registers some abilities for every seat (the condition limits them to
/// the holder, because a technology can be gained mid-game and the resolver has no late registration) and the
/// faction ones for the seats of that faction.
#[derive(Clone, Copy)]
pub enum Applies {
    EverySeat,
    Faction(Faction),
    Held(fn(&CoreState, Seat) -> bool),
}

impl Applies {
    #[must_use]
    pub fn holds(self, s: &CoreState, who: Seat) -> bool {
        match self {
            Self::EverySeat => true,
            Self::Faction(f) => s.players().by_seat(who).is_some_and(|p| p.faction == f),
            Self::Held(f) => f(s, who),
        }
    }
}

/// Why a condition cannot be answered by the core: a documented bail of the rule it reads (the adapter words it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unknown {
    /// Yssaril's Transparasteel Plating (the faction module's hook).
    YssarilTransparasteel,
    /// Bribery's goods (the Keleres agent's commodities).
    BriberyGoods,
    /// A printed card cost the core does not judge.
    CardCost,
    /// Harness Energy / Infiltrate: the tile catalogue and the swap rules.
    HarnessOrInfiltrate,
    /// A Nomad that listens to the exhaustion of an agent (Temporal Command Suite): the exhaustion is announced.
    NomadAgentExhaustion,
    /// A fact the adapter should have computed for the window is missing (`Facts`).
    MissingFact,
}

/// What a condition reads: the state, the kernel, the ability and the event it is asked about.
pub struct CondCx<'a> {
    pub state: &'a CoreState,
    pub kernel: &'a Kernel,
    pub def: &'a AbilityDef,
    pub event: &'a EventRec,
    pub resolver: &'a Resolver,
    /// Political Secret bars this seat's faction abilities and action cards (`AbilityTable::bars`).
    pub barred: bool,
    /// The texts the payloads carry as codes (`AbilityTable::codes`).
    pub codes: Codes,
    /// What the adapter computed for the window (`AbilityTable::facts`).
    pub facts: &'a AdapterFacts,
    unknown: &'a std::cell::Cell<Option<Unknown>>,
}

impl CondCx<'_> {
    /// The condition cannot be answered by the core (a documented bail of the rule it reads): the answer given
    /// stands for nothing, and the driver reports `why` (the dual falls back to the string run).
    pub fn unknown(&self, why: Unknown) -> bool {
        if self.unknown.get().is_none() {
            self.unknown.set(Some(why));
        }
        false
    }
}

/// The few texts an ability compares an event payload against (a payload holds `Code`s, interned by the
/// adapter; the core has no text). Set once by the adapter when it builds the resolver.
#[derive(Debug, Clone, Copy, Default)]
pub struct Codes {
    /// The cause `ground_combat` of a destroyed ground force.
    pub ground_combat: crate::state::Code,
}

/// Facts the adapter computes for the window and the core cannot (no galaxy is lent to the windows that follow
/// abilities): the systems adjacent to a system as one seat sees them (`PlayerAdjacency::neighbours`).
#[derive(Debug, Clone, Default)]
pub struct AdapterFacts {
    reach: Vec<(Seat, crate::generated::System, Vec<crate::generated::System>)>,
}

impl AdapterFacts {
    /// Record the neighbours of `system` for `who`.
    pub fn set_reach(&mut self, who: Seat, system: crate::generated::System, neighbours: Vec<crate::generated::System>) {
        self.reach.retain(|(w, s, _)| !(*w == who && *s == system));
        self.reach.push((who, system, neighbours));
    }

    /// The neighbours of `system` for `who`, if the adapter computed them.
    #[must_use]
    pub fn reach(&self, who: Seat, system: crate::generated::System) -> Option<&[crate::generated::System]> {
        self.reach.iter().find(|(w, s, _)| *w == who && *s == system).map(|(_, _, n)| n.as_slice())
    }
}

/// A rule-specific eligibility (`Host::condition`).
pub type Condition = fn(&CondCx<'_>) -> bool;
/// The effect of a selected ability.
pub type Effect = fn(&mut Eff<'_, '_>) -> Result<(), TimingError>;
/// The predicate of a registered "cannot" (`Host::forbids`).
pub type Forbids = fn(&CoreState, &CannotDef, &EventRec) -> bool;

/// One ability rule: its registry row and its behaviour.
#[derive(Clone, Copy)]
pub struct Spec {
    pub slot: Slot,
    pub kind: AbilityKind,
    pub applies: Applies,
    pub event: EventKind,
    pub relation: Relation,
    pub frequency: Frequency,
    pub optional: bool,
    pub repeatable: bool,
    pub condition: Option<Condition>,
    pub effect: Effect,
    /// The effect is the core's. `false`: only the eligibility is (the offer is rendered and a decline is
    /// followed); a run in which the string took the ability is not followed.
    pub effect_ported: bool,
}

/// The effect of a spec whose eligibility only is ported.
#[allow(clippy::unnecessary_wraps)]
pub fn no_effect(_: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    Ok(())
}

impl Spec {
    /// A mandatory, once-per-trigger, unconditional spec for every seat.
    #[must_use]
    pub const fn new(slot: Slot, kind: AbilityKind, event: EventKind, relation: Relation, effect: Effect) -> Self {
        Self {
            slot,
            kind,
            applies: Applies::EverySeat,
            event,
            relation,
            frequency: Frequency::OncePerTrigger,
            optional: false,
            repeatable: false,
            condition: None,
            effect,
            effect_ported: true,
        }
    }

    /// An ability whose eligibility is ported and whose effect is not (yet): it can be offered and declined.
    #[must_use]
    pub const fn eligibility_only(slot: Slot, kind: AbilityKind, event: EventKind, relation: Relation, condition: Condition) -> Self {
        let mut s = Self::new(slot, kind, event, relation, no_effect);
        s.condition = Some(condition);
        s.effect_ported = false;
        s
    }

    #[must_use]
    pub const fn applies(mut self, a: Applies) -> Self {
        self.applies = a;
        self
    }

    #[must_use]
    pub const fn when(mut self, c: Condition) -> Self {
        self.condition = Some(c);
        self
    }

    #[must_use]
    pub const fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    #[must_use]
    pub const fn frequency(mut self, f: Frequency) -> Self {
        self.frequency = f;
        self
    }

    #[must_use]
    pub const fn repeatable(mut self) -> Self {
        self.repeatable = true;
        self
    }

    /// The registry row of `owner`; `id` is the registered text interned by the adapter.
    #[must_use]
    pub fn def(&self, id: AbilityId, owner: Seat) -> AbilityDef {
        let mut d = AbilityDef::new(id, owner, self.event, self.relation)
            .with_frequency(self.frequency)
            .with_optional(self.optional)
            .with_repeatable(self.repeatable)
            .with_arg(self.slot.arg());
        if self.condition.is_some() {
            d = d.with_condition();
        }
        d
    }
}

/// The rules behind the abilities the resolver's registry lists, by slot.
#[derive(Clone, Default)]
pub struct AbilityTable {
    specs: Vec<Spec>,
    forbids: Vec<(u32, Forbids)>,
    /// Political Secret (`laws::political_secret_bars`): no faction ability for this seat. Set by the rules
    /// package that owns it (the kernel cannot import the agenda area).
    pub bars: Option<fn(&CoreState, Seat) -> bool>,
    pub codes: Codes,
    pub facts: AdapterFacts,
}

impl AbilityTable {
    pub fn add(&mut self, spec: Spec) {
        debug_assert!(self.of_slot(spec.slot).is_none(), "slot {:?} registered twice", spec.slot);
        self.specs.push(spec);
    }

    /// The predicate of the cannot whose `arg` is given.
    pub fn add_forbid(&mut self, arg: u32, f: Forbids) {
        self.forbids.push((arg, f));
    }

    #[must_use]
    pub fn specs(&self) -> &[Spec] {
        &self.specs
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.specs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    #[must_use]
    pub fn of_slot(&self, slot: Slot) -> Option<&Spec> {
        self.specs.iter().find(|s| s.slot == slot)
    }

    /// The spec behind a registry row (`arg` is the slot).
    #[must_use]
    pub fn of(&self, def: &AbilityDef) -> Option<&Spec> {
        let slot = u16::try_from(def.arg).ok()?;
        self.of_slot(Slot(slot))
    }

    /// Whether the table has a rule for `arg`.
    #[must_use]
    pub fn knows(&self, arg: u32) -> bool {
        u16::try_from(arg).ok().is_some_and(|s| self.of_slot(Slot(s)).is_some())
    }

    /// The registry of a game: per seat of `order`, the specs that apply to it, in table order. `name` interns
    /// the registered text of a spec for an owner (the adapter's; the core holds no text).
    pub fn registry(
        &self,
        s: &CoreState,
        order: &[Seat],
        name: &mut dyn FnMut(&Spec, Seat) -> AbilityId,
    ) -> Vec<AbilityDef> {
        let mut out = Vec::new();
        for &who in order {
            for spec in &self.specs {
                if spec.applies.holds(s, who) {
                    out.push(spec.def(name(spec, who), who));
                }
            }
        }
        out
    }
}

/// What a nested question was asked about: the event of the window, the relation and the emission chain around
/// it. The adapter builds the question's trigger from it (the display facts of the string `DecisionContext`).
#[derive(Debug, Clone, PartialEq)]
pub struct NestedFacts {
    pub event: EventRec,
    pub relation: Relation,
    pub chain: Vec<u64>,
}

/// The answers of the current operation, in the order they were given; the nested questions of a step
/// read them in order. `needed` is the first question that found the tape dry.
#[derive(Debug, Clone, Default)]
pub struct Tape {
    answers: Vec<Pick>,
    used: usize,
    needed: Option<Choice>,
    facts: Option<NestedFacts>,
    /// A condition the core could not answer (`CondCx::unknown`); survives the rewind of a re-run.
    pub unknown: std::cell::Cell<Option<Unknown>>,
}

impl Tape {
    pub fn clear(&mut self) {
        self.answers.clear();
        self.rewind();
    }

    /// The reason a condition could not be answered since the last call, if any.
    pub fn take_unknown(&self) -> Option<Unknown> {
        self.unknown.take()
    }

    /// Start reading from the first answer again (a re-run of the step).
    pub fn rewind(&mut self) {
        self.used = 0;
        self.needed = None;
        self.facts = None;
    }

    pub fn push(&mut self, pick: Pick) {
        self.answers.push(pick);
    }

    /// The question that found the tape dry in the run just made.
    #[must_use]
    pub fn needed(&self) -> Option<&Choice> {
        self.needed.as_ref()
    }

    /// What that question was asked about (`None` for a question that has no event).
    #[must_use]
    pub fn needed_facts(&self) -> Option<&NestedFacts> {
        self.facts.as_ref()
    }

    /// [`Asker::pick`] with the facts of the window the question belongs to.
    pub fn pick_with(&mut self, choice: &Choice, facts: Option<NestedFacts>) -> Pick {
        if let Some(p) = self.answers.get(self.used) {
            self.used += 1;
            return *p;
        }
        if self.needed.is_none() {
            self.needed = Some(choice.clone());
            self.facts = facts;
        }
        // The run is rolled back; an option that ends the question lets it finish: the decline when there is one
        // (a repeatable, unlimited ability that is picked again and again would never end the window).
        choice.decline_index().and_then(|i| choice.pick(i)).or_else(|| choice.pick(0)).unwrap_or(Pick::Opt(crate::choice::Opt::Decline))
    }
}

impl Asker for Tape {
    fn pick(&mut self, _: &CoreState, choice: &Choice) -> Pick {
        self.pick_with(choice, None)
    }
}

/// The resolver's `Host` under the real rules: dispatches on the slot, asks through the tape.
pub struct RulesHost<'a> {
    pub state: &'a mut CoreState,
    pub kernel: &'a Kernel,
    pub table: &'a AbilityTable,
    pub tape: &'a mut Tape,
    /// The next trace id of an event an ability emits.
    pub next_id: &'a mut u64,
    /// Seeds the random streams (see `Sx::die`).
    pub seeds: &'a dyn SeedSource,
}

impl Host for RulesHost<'_> {
    fn state(&self) -> &CoreState {
        self.state
    }

    fn bars_faction_abilities(&mut self, who: Seat) -> bool {
        self.table.bars.is_some_and(|f| f(self.state, who))
    }

    fn condition(&mut self, ability: &AbilityDef, event: &EventRec, resolver: &Resolver) -> bool {
        match self.table.of(ability).and_then(|a| a.condition) {
            Some(c) => {
                let barred = self.table.bars.is_some_and(|f| f(self.state, ability.owner));
                c(&CondCx { state: self.state, kernel: self.kernel, def: ability, event, resolver, barred, codes: self.table.codes, facts: &self.table.facts, unknown: &self.tape.unknown })
            }
            None => true,
        }
    }

    fn forbids(&mut self, cannot: &CannotDef, event: &EventRec) -> bool {
        self.table.forbids.iter().find(|(arg, _)| *arg == cannot.arg).is_some_and(|(_, f)| f(self.state, cannot, event))
    }

    fn effect(&mut self, ability: &AbilityDef, event: &mut EventRec, resolver: &mut Resolver) -> Result<(), TimingError> {
        let Some(spec) = self.table.of(ability) else { return Ok(()) };
        let effect = spec.effect;
        let mut eff = Eff { host: self, resolver, def: *ability, event };
        effect(&mut eff)
    }

    fn decide(&mut self, choice: &Choice, event: &EventRec, chain: &[u64]) -> Pick {
        let relation = match choice.ask {
            crate::choice::Ask::Reaction(crate::choice::reaction::Ask::Window { relation, .. }) => Some(relation),
            _ => None,
        };
        let facts = relation.map(|relation| NestedFacts { event: event.clone(), relation, chain: chain.to_vec() });
        self.tape.pick_with(choice, facts)
    }
}

/// What an ability's effect may do: read and change the state, ask, draw dice, emit a nested event, cancel
/// the event of its window. Everything it does is undone by the driver's rollback when a nested question
/// found the tape dry, so an effect that asks twice still runs once per committed run.
pub struct Eff<'h, 'a> {
    host: &'h mut RulesHost<'a>,
    resolver: &'h mut Resolver,
    pub def: AbilityDef,
    pub event: &'h mut EventRec,
}

impl Eff<'_, '_> {
    pub fn state(&mut self) -> &mut CoreState {
        self.host.state
    }

    #[must_use]
    pub fn kernel(&self) -> &Kernel {
        self.host.kernel
    }

    /// What the adapter computed for the window.
    #[must_use]
    pub fn adapter_facts(&self) -> &AdapterFacts {
        &self.host.table.facts
    }

    /// The state and the kernel, read together (`state()` borrows the effect mutably).
    #[must_use]
    pub fn read(&self) -> (&CoreState, &Kernel) {
        (&*self.host.state, self.host.kernel)
    }

    #[must_use]
    pub fn owner(&self) -> Seat {
        self.def.owner
    }

    /// The ids of the emissions around the event of this window (outermost first, the event excluded).
    #[must_use]
    pub fn chain(&self) -> Vec<u64> {
        self.resolver.emission_chain(self.event)
    }

    /// The facts a question asked by this ability carries (its trigger).
    #[must_use]
    pub fn facts(&self) -> NestedFacts {
        NestedFacts { event: self.event.clone(), relation: self.def.relation, chain: self.chain() }
    }

    /// Put a question and take the answer.
    pub fn ask(&mut self, choice: &Choice) -> Pick {
        let facts = self.facts();
        self.host.tape.pick_with(choice, Some(facts))
    }

    /// Draw a die from the stream of `domain` (`Sx::die`).
    pub fn die(&mut self, domain: RngDomain, sides: u32) -> u32 {
        self.host.state.rng_mut().die(domain, self.host.seeds, sides)
    }

    /// Roll `count` dice with `sides` sides from the dice stream and record the roll in the ledger (`Dice::roll_by`
    /// of an ability: the faces and the record are the string's). Returns the faces.
    pub fn roll(&mut self, by: Seat, count: usize, hits_on: Option<u8>, purpose: crate::state::RollPurpose, sides: u32) -> Vec<u8> {
        let faces: Vec<u8> = (0..count).map(|_| u8::try_from(self.die(RngDomain::Dice, sides)).unwrap_or(u8::MAX)).collect();
        self.host.state.ledger_mut().dice.push(crate::state::RollRec {
            purpose,
            owner: Some(by),
            unit: None,
            planet: None,
            scene: None,
            hits_on,
            faces: faces.iter().copied().collect(),
            rerolled: 0,
        });
        faces
    }

    /// Emit a nested event through the same resolver (the string `emit_with_context` of an effect). A zero id is
    /// replaced by the next trace id.
    ///
    /// # Errors
    /// What the resolver refuses with.
    pub fn emit(&mut self, mut rec: EventRec) -> Result<EventRec, TimingError> {
        if rec.id == 0 {
            rec.id = *self.host.next_id;
            *self.host.next_id += 1;
        }
        self.resolver.emit(&mut *self.host, rec, |_, _, _| {})
    }

    /// "Cancel that event."
    pub fn cancel(&mut self) {
        self.event.cancelled = true;
    }
}
