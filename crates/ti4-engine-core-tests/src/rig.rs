//! `Rig<W>`: runs one [`Window`] through the real [`Driver`] and keeps everything a test (and the properties) needs.
//!
//! * the question log and the answers given (`transcript()`), the stage trace (the stage the window is in when first stepped, and every `window.stage()` after a step, is registered in the covers
//!   registry as `stage Type::Stage`),
//! * `replay()`: the same window rebuilt from the recorded start and answered again (a window is deterministic over its start and answers),
//! * `assert_options_sound()`: P4, every offered option is accepted on a replayed copy, none is repeated, the actor is seated, a foreign pick
//!   is `Illegal` and leaves the state untouched.

use std::fmt::Write as _;
use std::rc::Rc;

use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind};
use ti4_engine_core::choice::{Ask, Choice, Opt, Pick};
use ti4_engine_core::driver::{Driver, DriverError, Status};
use ti4_engine_core::generated::Faction;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::stage::{AbilityTable, Window};
use ti4_engine_core::kernel::{Kernel, Resolver};
use ti4_engine_core::state::{Code, CoreState, RngDomain, SeedSource};

use crate::covers;
use crate::scenario::Scenario;

/// A deterministic dice source for tests: every stream is a function of `(seed, stream seed, domain)`; two rigs with the same seed roll the same faces.
#[derive(Debug, Clone, Copy)]
pub struct KitSeeds(pub u64);

impl SeedSource for KitSeeds {
    fn seed_for(&self, seed: u64, domain: RngDomain) -> [u8; 32] {
        let text = format!("{:?}", domain);
        let mut acc = self.0 ^ seed.rotate_left(17) ^ 0x9E37_79B9_7F4A_7C15;
        for b in text.bytes() {
            acc = (acc ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3);
        }
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            acc = acc.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407 + i as u64);
            *byte = (acc >> 33) as u8;
        }
        out
    }
}

/// One question and, once given, its answer.
#[derive(Debug, Clone)]
pub struct Entry {
    pub choice: Choice,
    pub answer: Option<Pick>,
}

/// How [`Rig::auto`] answers.
#[derive(Debug, Clone, Copy)]
pub enum Policy {
    First,
    Last,
    /// Option `(seed + question number * 7) % len`.
    Mix(u64),
}

pub struct Rig<W: Window> {
    pub state: CoreState,
    pub kernel: Kernel,
    pub driver: Driver<W>,
    start: CoreState,
    window0: W,
    order: Vec<Seat>,
    active: Option<Seat>,
    table: AbilityTable,
    abilities: bool,
    seed: Option<u64>,
    picks: Vec<Pick>,
    pub log: Vec<Entry>,
    pub stages: Vec<String>,
    status: Option<Status>,
}

fn pick_of(c: &Choice, index: usize) -> Pick {
    c.pick(index).unwrap_or_else(|| panic!("option {index} of {} offered", c.opts.len()))
}

impl<W: Window> Rig<W> {
    /// A rig on the position `scenario` builds; the window runs for the seats in seat order, `scenario`'s active player active.
    #[must_use]
    pub fn new(scenario: Scenario, window: W) -> Self {
        crate::covers::install_probe();
        let (state, kernel) = scenario.build();
        Self::from_parts(state, kernel, window)
    }

    #[must_use]
    pub fn from_parts(state: CoreState, kernel: Kernel, window: W) -> Self {
        let order = state.seats().seating_order().to_vec();
        let active = state.meta().active;
        let table = AbilityTable::default();
        let seed = None;
        let driver = Self::driver_for(&window, &order, active, &table, seed, false, &state);
        Self { start: state.clone(), state, kernel, driver, window0: window, order, active, table, abilities: false, seed, picks: Vec::new(), log: Vec::new(), stages: Vec::new(), status: None }
    }

    fn driver_for(window: &W, order: &[Seat], active: Option<Seat>, table: &AbilityTable, seed: Option<u64>, abilities: bool, state: &CoreState) -> Driver<W> {
        let mut resolver = Resolver::new(order, active);
        if abilities {
            // Register the abilities of the table for every seat, named by their slot (the adapter's texts are not needed here).
            let defs = table.registry(state, order, &mut |spec, _, _| AbilityId { kind: AbilityKind::Other, key: Code(spec.slot.0) });
            resolver.register(defs);
        }
        // The links to the windows an area runs as children (Politics draws through a `DrawCards` child since W6): installed in every run.
        let mut table = table.clone();
        ti4_engine_core::driver::children::register(&mut table);
        let d = Driver::new(window.clone(), resolver, table, 1);
        match seed {
            Some(s) => d.with_seeds(Rc::new(KitSeeds(s))),
            None => d,
        }
    }

    fn reset(&mut self) {
        self.driver = Self::driver_for(&self.window0, &self.order, self.active, &self.table, self.seed, self.abilities, &self.start);
    }

    /// The ability table of the run (set it before the first `run`).
    #[must_use]
    pub fn with_table(mut self, table: AbilityTable) -> Self {
        self.table = table;
        self.reset();
        self
    }

    /// The abilities of `table` registered in the resolver of every seat (reaction windows then open around the events the window emits).
    #[must_use]
    pub fn with_abilities(mut self, table: AbilityTable) -> Self {
        self.table = table;
        self.abilities = true;
        self.reset();
        self
    }

    /// Roll dice from [`KitSeeds`]`(seed)` (set it before the first `run`).
    #[must_use]
    pub fn seeded(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self.reset();
        self
    }

    /// The initiative order the resolver uses (set it before the first `run`).
    #[must_use]
    pub fn with_order(mut self, order: &[Seat], active: Option<Seat>) -> Self {
        self.order = order.to_vec();
        self.active = active;
        self.reset();
        self
    }

    /// The state the run started from.
    #[must_use]
    pub fn start_state(&self) -> &CoreState {
        &self.start
    }

    /// The answers given so far.
    #[must_use]
    pub fn picks(&self) -> &[Pick] {
        &self.picks
    }

    /// The stage the window is in when the driver first steps it (`Start` of most windows): a stage the run visits although the
    /// trace after each step never shows it (the first step leaves it).
    fn note_entry(&self) {
        let stage = format!("{:?}", self.driver.window.stage());
        covers::register("stage", &format!("{}::{stage}", std::any::type_name::<W>()), std::thread::current().name().unwrap_or("?"));
    }

    fn note(&mut self, status: Status) {
        let stage = format!("{:?}", self.driver.window.stage());
        covers::register("stage", &format!("{}::{stage}", std::any::type_name::<W>()), std::thread::current().name().unwrap_or("?"));
        covers::register("window", std::any::type_name::<W>(), std::thread::current().name().unwrap_or("?"));
        self.stages.push(stage);
        if let Status::Asking(c) = &status {
            self.log.push(Entry { choice: c.clone(), answer: None });
        }
        self.status = Some(status);
    }

    /// Step the window to its first question or its end.
    ///
    /// # Errors
    /// What the driver refuses with.
    pub fn try_run(&mut self) -> Result<&Status, DriverError> {
        if self.status.is_none() {
            self.note_entry();
        }
        let st = self.driver.run(&mut self.state, &self.kernel)?;
        self.note(st);
        Ok(self.status.as_ref().expect("status"))
    }

    /// Answer the question asked.
    ///
    /// # Errors
    /// What the driver refuses with; an `Illegal` pick changes nothing and is not recorded.
    pub fn try_answer(&mut self, pick: Pick) -> Result<&Status, DriverError> {
        let st = self.driver.resume(&mut self.state, &self.kernel, pick)?;
        if let Some(last) = self.log.last_mut() {
            last.answer = Some(pick);
        }
        self.picks.push(pick);
        self.note(st);
        Ok(self.status.as_ref().expect("status"))
    }

    /// [`Self::try_run`] that panics on an error.
    ///
    /// # Panics
    /// On a driver error.
    pub fn run(&mut self) -> &mut Self {
        if let Err(e) = self.try_run() {
            panic!("the window failed to run: {e:?}\n{}", self.transcript());
        }
        self
    }

    /// The status after the last step (`None` before the first `run`).
    #[must_use]
    pub fn status(&self) -> Option<&Status> {
        self.status.as_ref()
    }

    #[must_use]
    pub fn is_done(&self) -> bool {
        matches!(self.status, Some(Status::Done))
    }

    /// The question asked now.
    ///
    /// # Panics
    /// When the window is not asking.
    #[must_use]
    pub fn asked(&self) -> &Choice {
        match &self.status {
            Some(Status::Asking(c)) => c,
            other => panic!("not asking ({other:?})\n{}", self.transcript()),
        }
    }

    /// The options of the question asked now.
    #[must_use]
    pub fn opts(&self) -> Vec<Opt> {
        self.asked().opts.as_slice().to_vec()
    }

    /// Answer with option number `index`.
    ///
    /// # Panics
    /// When there is no such option or the driver refuses.
    pub fn pick(&mut self, index: usize) -> &mut Self {
        let p = pick_of(self.asked(), index);
        self.answer(p)
    }

    /// Answer with a specific offered option.
    ///
    /// # Panics
    /// When it is not offered.
    pub fn pick_opt(&mut self, o: Opt) -> &mut Self {
        assert!(self.asked().opts.index_of(&o).is_some(), "{o:?} is not offered\n{}", self.transcript());
        self.answer(Pick::Opt(o))
    }

    /// Answer with the first offered option for which `want` holds.
    ///
    /// # Panics
    /// When none matches.
    pub fn pick_where(&mut self, want: impl Fn(&Opt) -> bool) -> &mut Self {
        let Some(o) = self.asked().opts.iter().copied().find(|o| want(o)) else { panic!("no matching option\n{}", self.transcript()) };
        self.answer(Pick::Opt(o))
    }

    /// Answer the decline option.
    ///
    /// # Panics
    /// When the question cannot be declined.
    pub fn decline(&mut self) -> &mut Self {
        let i = self.asked().decline_index().unwrap_or_else(|| panic!("cannot decline\n{}", self.transcript()));
        self.pick(i)
    }

    /// Answer with a number (a count question).
    pub fn number(&mut self, n: i32) -> &mut Self {
        self.answer(Pick::Number(n))
    }

    fn answer(&mut self, p: Pick) -> &mut Self {
        if let Err(e) = self.try_answer(p) {
            panic!("the answer {p:?} was refused: {e:?}\n{}", self.transcript());
        }
        self
    }

    /// Answer `picks[i] % options` at each question until the picks run out or the window ends.
    pub fn script(&mut self, picks: &[u8]) -> &mut Self {
        if self.status.is_none() {
            self.run();
        }
        for b in picks {
            let Some(Status::Asking(c)) = &self.status else { break };
            let i = usize::from(*b) % c.opts.len().max(1);
            let p = c.pick(i).unwrap_or(Pick::Opt(Opt::Decline));
            self.answer(p);
        }
        self
    }

    /// Run (if needed) and answer by `policy` until the window ends.
    ///
    /// # Panics
    /// After 500 questions (a window that never ends).
    pub fn auto(&mut self, policy: Policy) -> &mut Self {
        if self.status.is_none() {
            self.run();
        }
        for n in 0..500u64 {
            let Some(Status::Asking(c)) = &self.status else { return self };
            let len = c.opts.len();
            let i = match policy {
                Policy::First => 0,
                Policy::Last => len.saturating_sub(1),
                Policy::Mix(seed) => ((seed + n * 7) % len.max(1) as u64) as usize,
            };
            let p = c.pick(i).unwrap_or(Pick::Number(0));
            self.answer(p);
        }
        panic!("500 questions and the window is still asking\n{}", self.transcript());
    }

    /// Assert that answering `pick` now is refused as `Illegal`, changes nothing and leaves the same question to answer.
    ///
    /// # Panics
    /// When it is accepted, or refused another way, or the state or the question changed.
    pub fn expect_illegal(&mut self, pick: Pick) {
        let before = self.state.clone();
        let question = self.asked().clone();
        let r = self.driver.resume(&mut self.state, &self.kernel, pick);
        assert_eq!(r, Err(DriverError::Illegal), "{pick:?} must be illegal here\n{}", self.transcript());
        assert!(self.state == before, "an illegal pick changed the state:\n{}", crate::delta::delta(&before, &self.state));
        assert_eq!(self.asked(), &question);
    }

    /// The transcript of the run: one block per question with its options and the answer given.
    #[must_use]
    pub fn transcript(&self) -> String {
        let mut out = String::new();
        for (i, e) in self.log.iter().enumerate() {
            let _ = writeln!(out, "Q{i} seat {} {:?}", e.choice.actor.0, e.choice.ask);
            for (j, o) in e.choice.opts.iter().enumerate() {
                let mark = if e.answer == Some(Pick::Opt(*o)) { ">" } else { " " };
                let _ = writeln!(out, "  {mark}{j}: {o:?}");
            }
            match e.answer {
                Some(Pick::Number(n)) => {
                    let _ = writeln!(out, "  answer: number {n}");
                }
                Some(Pick::Opt(_)) | None => {}
            }
        }
        let _ = writeln!(out, "{}", if self.is_done() { "done" } else { "asking" });
        out
    }

    /// The text a scenario snapshot holds: the transcript, then the delta from the start.
    #[must_use]
    pub fn snapshot(&self) -> String {
        format!("{}--- delta ---\n{}", self.transcript(), crate::delta::delta(&self.start, &self.state))
    }

    /// The same window, from the recorded start, answered with the recorded answers again: the state it reaches must be this one.
    ///
    /// # Panics
    /// When the replay fails to run or an answer is refused.
    #[must_use]
    pub fn replay(&self) -> Self {
        let mut r = Self::from_parts(self.start.clone(), self.kernel.clone(), self.window0.clone());
        r.table = self.table.clone();
        r.abilities = self.abilities;
        r.seed = self.seed;
        r.order.clone_from(&self.order);
        r.active = self.active;
        r.reset();
        if self.status.is_some() {
            r.run();
            for p in &self.picks {
                r.answer(*p);
            }
        }
        r
    }

    /// P4 for the question asked now. Panics with the transcript on a violation.
    ///
    /// # Panics
    /// On a violation.
    pub fn assert_options_sound(&self) {
        let c = self.asked();
        let seated = self.state.seats().seating_order();
        assert!(seated.contains(&c.actor) || self.state.seats().player_of(c.actor).is_some(), "the actor {:?} is not seated\n{}", c.actor, self.transcript());
        let counts = matches!(c.ask, Ask::Generic(ti4_engine_core::choice::generic::Ask::Count { .. }));
        assert!(!c.opts.is_empty() || counts, "a question without options\n{}", self.transcript());
        for (i, o) in c.opts.iter().enumerate() {
            assert!(!c.opts.as_slice()[..i].contains(o), "option {o:?} is offered twice\n{}", self.transcript());
        }
        for (i, o) in c.opts.iter().enumerate() {
            let mut r = self.replay();
            if let Err(e) = r.try_answer(Pick::Opt(*o)) {
                panic!("the offered option {i} ({o:?}) is refused: {e:?}\n{}", self.transcript());
            }
        }
        // A question put from inside an ability is answered from the tape: the driver does not judge its picks (the ability does).
        if self.driver.nested_facts().is_some() {
            return;
        }
        // A foreign option is illegal and changes nothing.
        let foreign = [Opt::Number(-9_999_001), Opt::Done, Opt::Decline].into_iter().find(|o| !c.opts.iter().any(|x| x == o));
        let mut probe = self.replay();
        if let Some(o) = foreign {
            probe.expect_illegal(Pick::Opt(o));
        }
        if !counts {
            probe.expect_illegal(Pick::Number(7));
        }
    }

    /// Whether the faction is seated (for tests that look a seat up by faction).
    #[must_use]
    pub fn seat_of(&self, faction: Faction) -> Option<Seat> {
        self.state.players().iter().find(|p| p.faction == faction).map(|p| p.seat)
    }
}
