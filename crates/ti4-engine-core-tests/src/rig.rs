//! `Rig<W>`: runs one [`Window`] through the real [`Driver`] and keeps everything a test (and the properties) needs.
//!
//! * the question log and the answers given (`transcript()`), the stage trace (the stage the window is in when first stepped, and every `window.stage()` after a step, is registered in the covers
//!   registry as `stage Type::Stage`),
//! * `replay()`: the same window rebuilt from the recorded start and answered again (a window is deterministic over its start and answers),
//! * `assert_options_sound()`: P4, every offered option is accepted on a replayed copy, none is repeated, the actor is seated, a foreign pick
//!   is `Illegal` and leaves the state untouched.

use std::fmt::Write as _;

use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind};
use ti4_engine_core::choice::{Ask, Choice, Opt, Pick};
use ti4_engine_core::driver::{Driver, DriverError, Status};
use ti4_engine_core::generated::Faction;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::stage::{AbilityTable, Window};
use ti4_engine_core::kernel::{Kernel, Resolver};
use ti4_engine_core::state::{Code, CoreState, Rng};

use crate::covers;
use crate::scenario::Scenario;

/// The random streams of a test seed (lane C2: the streams are `ti4_chance::Streams`, keyed by the bottom crate; the old `KitSeeds`
/// seed source is gone): two rigs with the same seed roll the same faces.
#[must_use]
pub fn kit_rng(seed: u64) -> Rng {
    Rng::new(seed ^ 0x9E37_79B9_7F4A_7C15)
}

/// [`kit_rng`] standing where `at` stands (the streams a seeded rig draws from in that position).
#[must_use]
pub fn kit_at(seed: u64, at: &Rng) -> Rng {
    let mut kit = kit_rng(seed);
    kit.set_positions(&at.positions().collect::<Vec<_>>());
    kit
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
    /// The streams of [`Self::seeded`]: keyed by the test seed, standing where the position's streams stand (what the old `KitSeeds` seed
    /// source did: the keys are the rig's, the positions the state's). Put into the position for each driver call, and the positions it
    /// reached are written back (a run that rolls nothing leaves the state equal to its start; lane C2).
    kit: Option<Rng>,
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
        Self { start: state.clone(), state, kernel, driver, window0: window, order, active, table, abilities: false, seed, kit: None, picks: Vec::new(), log: Vec::new(), stages: Vec::new(), status: None }
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
        let _ = seed;
        Driver::new(window.clone(), resolver, table, 1)
    }

    fn reset(&mut self) {
        self.driver = Self::driver_for(&self.window0, &self.order, self.active, &self.table, self.seed, self.abilities, &self.start);
        self.kit = self.seed.map(kit_rng);
    }

    /// One driver call, then every chance question it stops at answered (lane C3: the core asks for each random event; the rig is its
    /// resolver). Seeded, the faces come from the rig's own streams standing at the position's stream positions, and the positions they
    /// reached are written back into `state.rng`; else from the position's streams (`ti4_engine_chance::settle`).
    fn drive(&mut self, f: impl FnOnce(&mut Driver<W>, &mut CoreState, &Kernel) -> Result<Status, DriverError>) -> Result<Status, DriverError> {
        let r = f(&mut self.driver, &mut self.state, &self.kernel);
        match self.kit.as_mut() {
            Some(kit) => {
                kit.set_positions(&self.state.rng.positions().collect::<Vec<_>>());
                let r = ti4_engine_chance::settle_from(&mut self.driver, &mut self.state, &self.kernel, r, kit);
                let reached: Vec<_> = kit.positions().collect();
                if !self.state.rng.positions().eq(reached.iter().copied()) {
                    self.state.rng.set_positions(&reached);
                }
                r
            }
            None => ti4_engine_chance::settle(&mut self.driver, &mut self.state, &self.kernel, r),
        }
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

    /// Roll dice from [`kit_rng`]`(seed)` (set it before the first `run`).
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
        let st = self.drive(|d, s, k| d.run(s, k))?;
        self.note(st);
        Ok(self.status.as_ref().expect("status"))
    }

    /// Answer the question asked.
    ///
    /// # Errors
    /// What the driver refuses with; an `Illegal` pick changes nothing and is not recorded.
    pub fn try_answer(&mut self, pick: Pick) -> Result<&Status, DriverError> {
        let st = self.drive(|d, s, k| d.resume(s, k, pick))?;
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

    /// Answer a payment question with the set of these atoms (offered options; a trade good once per one spent).
    ///
    /// # Panics
    /// When the question is no payment or an atom is not offered.
    pub fn pay(&mut self, atoms: &[Opt]) -> &mut Self {
        let opts: Vec<ti4_engine_core::choice::production::Opt> = self.opts().into_iter().filter_map(|o| if let Opt::Production(p) = o { Some(p) } else { None }).collect();
        let picks: Vec<ti4_engine_core::choice::production::Opt> = atoms.iter().filter_map(|o| if let Opt::Production(p) = o { Some(*p) } else { None }).collect();
        let set = ti4_engine_core::choice::production::PaySet::of(&opts, &picks).unwrap_or_else(|| panic!("{atoms:?} are not atoms of this question\n{}", self.transcript()));
        self.answer(Pick::Opt(Opt::Production(ti4_engine_core::choice::production::Opt::Set(set))))
    }

    /// Answer a payment question with the set of the offered atoms for which `want` holds (a trade good: as many as are held).
    pub fn pay_where(&mut self, want: impl Fn(&Opt) -> bool) -> &mut Self {
        let atoms: Vec<Opt> = self.opts().into_iter().filter(|o| want(o)).collect();
        self.pay(&atoms)
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

    /// Answer with `p` (a set, a number, an option).
    ///
    /// # Panics
    /// When the driver refuses.
    pub fn answer_pick(&mut self, p: Pick) -> &mut Self {
        self.answer(p)
    }

    fn answer(&mut self, p: Pick) -> &mut Self {
        let p = self.as_answer(p);
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
            // A multi-pick (a payment) is answered by one of its legal sets, the canonical one first.
            let answers = if c.is_multi_pick() { c.answers(64) } else { Vec::new() };
            let len = if c.is_multi_pick() { answers.len() } else { c.opts.len() };
            let i = match policy {
                Policy::First => 0,
                Policy::Last => len.saturating_sub(1),
                Policy::Mix(seed) => ((seed + n * 7) % len.max(1) as u64) as usize,
            };
            let p = if c.is_multi_pick() {
                if i == 0 { c.default_pick() } else { answers.get(i).copied() }.unwrap_or(Pick::Number(0))
            } else {
                c.pick(i).unwrap_or(Pick::Number(0))
            };
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
        let r = self.drive(|d, s, k| d.resume(s, k, pick));
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
                Some(Pick::Chance(faces)) => {
                    let _ = writeln!(out, "  answer: faces {:?}", faces.as_slice());
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

    /// A test's answer as the question takes it: to a payment (a multi-pick, `docs/MULTIASK1_RESUME.md`) an ATOM means "pay with this
    /// one first, then the canonical order" (`Choice::pick` of its index); every other pick is itself.
    fn as_answer(&self, p: Pick) -> Pick {
        let Some(Status::Asking(c)) = &self.status else { return p };
        if !c.is_multi_pick() {
            return p;
        }
        match p {
            Pick::Opt(o) if !matches!(o, Opt::Production(ti4_engine_core::choice::production::Opt::Set(_))) => {
                c.opts.index_of(&o).and_then(|i| c.pick(i)).unwrap_or(p)
            }
            _ => p,
        }
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
            // (A payment's option is an atom: the set it leads is the answer.)
            let pick = if c.is_multi_pick() { c.pick(i).unwrap_or(Pick::Opt(*o)) } else { Pick::Opt(*o) };
            if let Err(e) = r.try_answer(pick) {
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
