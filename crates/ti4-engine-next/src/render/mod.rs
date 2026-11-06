//! The adapter half of the decision model (docs/ENGINE_CORE_STATE.md 2.3; package F4): renders the
//! core's typed [`Choice`](ti4_engine_core::choice::Choice) into today's string `Choice`
//! (ids, kinds, labels, payload, prompt, `DecisionContext`) byte for byte, and parses option ids back.
//!
//! One module per rule area implements [`Family`] for the area's `Ask/Opt/Subtype`; the registry
//! (`registry!` below) is a `match` over the top-level enums. An area that is not ported is
//! [`Unported`], whose methods cannot be called (its types are uninhabited). To register a ported
//! area, replace its `Unported<..>` line in the `registry!` call with the area's `Fam` type.
//!
//! Text lives here and nowhere in the core. [`Render`] carries the per-game tables the text needs
//! (the owner table, the code interner).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::marker::PhantomData;

use serde_json::Value;
use ti4_engine_core::choice::{self as core, Ask, Opt, Subtype};
use ti4_engine_core::state::{Code, Phase as CorePhase};
use ti4_engine_core::Seat;
use ti4_model::id::PlayerId;
use ti4_model::state::Phase;

use crate::wire::choice::{Choice, ChoiceOption};
use crate::wire::decision_context::DecisionContext;

pub mod generic;

pub mod agenda;
pub mod cards;
pub mod combat;
pub mod component;
pub mod diplomacy;
pub mod nested;
pub mod events;
pub mod explore;
pub mod invasion;
pub mod leaders;
pub mod legendary;
pub mod objectives;
pub mod production;
pub mod promissory;
pub mod reaction;
pub mod silent;
pub mod silent3;
pub mod activation;
pub mod silent4;
pub mod relic;
pub mod strategy;
pub mod tactical;
pub mod technology;
pub mod tokens;
pub mod trade;
pub mod turn;

/// The position a question is rendered in: the state and the kernel the window read when it built the
/// question. A [`Family`] whose options show facts of the position (a build's cost, what a placement
/// leaves of the fleet limit) reads them here instead of carrying them in the option, which keeps
/// `Opt` identity-only and small. `Rc`, so setting it costs no copy of the state.
#[derive(Clone)]
pub struct Position {
    pub state: std::rc::Rc<ti4_engine_core::state::CoreState>,
    pub kernel: &'static ti4_engine_core::kernel::Kernel,
}

impl std::fmt::Debug for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Position")
    }
}

/// The per-game tables the renderers need.
#[derive(Debug, Default)]
pub struct Render {
    /// The text of every seat, in seat order (= text order of the owner ids).
    pub seats: Vec<String>,
    /// The faction text of every seat (empty for a seat with no player), where a family names seats by
    /// faction (the Trade primary's options); empty when the caller set none.
    pub factions: Vec<String>,
    /// Texts the core holds as a [`Code`] (open string sets until X1 closes them): ability ids, event names outside the
    /// closed set, reaction texts. **Window-local until the flip** (D2-4b decision): its codes mean something only with
    /// the `Render` that minted them, so a `ResolverState` built through it is read back through the same `Render`.
    /// At D2-8 the `Render` borrows the game's `CoreState::texts.codes` instead, and these become state-owned like every
    /// other `Code`. Same `Interner` as the state's (a lookup is a search, not a scan; the codes are the same dense
    /// insertion-order indices as before).
    codes: RefCell<ti4_engine_core::containers::Interner>,
    /// The position of the question being rendered, when the caller set one (`None` for the families
    /// that need none; see [`Position`]).
    position: RefCell<Option<Position>>,
    /// Option shapes of areas whose options the string engine still builds (the strategy waivers, trade deals, waivers), by index.
    shapes: RefCell<Vec<ChoiceOption>>,
    /// The free-form deals of a transaction window (`Shape::Free(i)`): the id text and the net the partner's accept option reports.
    free: RefCell<Vec<(String, f64)>>,
    /// The typed ids of the registered abilities by (rule, owner, lender, their factions): the text of an id is a function of those and of this
    /// `Render`'s tables alone and its code is stable (the code table only grows), so the live loop builds each id once per game instead of
    /// formatting and interning it again at every step (docs/PERF_WHY.md). A new `Render` (a fork, the rollback of a failed step) starts empty.
    ability_ids: RefCell<std::collections::HashMap<u64, ti4_engine_core::choice::reaction::AbilityId, std::hash::BuildHasherDefault<KeyMix>>>,
    /// The values of `AbilityTable::codes` this `Render` gave the table (the same texts always get the same codes).
    table_codes: std::cell::Cell<Option<ti4_engine_core::kernel::ability::Codes>>,
    /// The values of `AbilityTable::codes.explore` this `Render` gave the table (same reason as [`Self::table_codes`]).
    explore_codes: std::cell::Cell<Option<ti4_engine_core::kernel::ability::ExploreCodes>>,
    /// The live `Env` of the last call (`live::env::env_with_plays`) with the facts it was built from; reused while they hold. A new `Render` starts empty.
    pub(crate) env_template: RefCell<Option<crate::live::env::Template>>,
}

/// The key of [`Render::ability_id_memo`]: the rule's slot, the owner seat, the lender seat (255: none), the owner's and the lender's faction (255: none), packed.
#[must_use]
pub fn ability_key(slot: u16, owner: u8, lender: Option<u8>, owner_faction: Option<ti4_engine_core::generated::Faction>, lender_faction: Option<ti4_engine_core::generated::Faction>) -> u64 {
    let f = |f: Option<ti4_engine_core::generated::Faction>| f.map_or(u64::MAX >> 56, |f| f as u64);
    u64::from(slot) | u64::from(owner) << 16 | u64::from(lender.unwrap_or(u8::MAX)) << 24 | f(owner_faction) << 32 | f(lender_faction) << 40
}

/// A hasher for the packed keys of [`ability_key`] (a multiplication; the std default hashes every key with SipHash, which cost more than the lookup saves).
#[derive(Default)]
pub struct KeyMix(u64);

impl std::hash::Hasher for KeyMix {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn write_u64(&mut self, k: u64) {
        self.0 = (self.0 ^ k).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 ^= self.0 >> 29;
    }
}

impl Render {
    #[must_use]
    pub fn new(seats: Vec<String>) -> Self {
        Self { seats, factions: Vec::new(), codes: RefCell::new(ti4_engine_core::containers::Interner::default()), position: RefCell::new(None), shapes: RefCell::new(Vec::new()), free: RefCell::new(Vec::new()), ability_ids: RefCell::new(std::collections::HashMap::default()), table_codes: std::cell::Cell::new(None), explore_codes: std::cell::Cell::new(None), env_template: RefCell::new(None) }
    }

    /// The memoized id of a registered ability, or the result of `build` (kept for the next call).
    pub fn ability_id_memo(&self, key: u64, build: impl FnOnce() -> ti4_engine_core::choice::reaction::AbilityId) -> ti4_engine_core::choice::reaction::AbilityId {
        if let Some(id) = self.ability_ids.borrow().get(&key) {
            return *id;
        }
        let id = build();
        self.ability_ids.borrow_mut().insert(key, id);
        id
    }

    /// The `AbilityTable::codes` this `Render` computed before, if it did.
    #[must_use]
    pub fn cached_table_codes(&self) -> Option<ti4_engine_core::kernel::ability::Codes> {
        self.table_codes.get()
    }

    /// The `AbilityTable::codes.explore` this `Render` computed before, if it did.
    #[must_use]
    pub fn cached_explore_codes(&self) -> Option<ti4_engine_core::kernel::ability::ExploreCodes> {
        self.explore_codes.get()
    }

    /// Remember the `AbilityTable::codes.explore` computed from this `Render`.
    pub fn cache_explore_codes(&self, codes: ti4_engine_core::kernel::ability::ExploreCodes) {
        self.explore_codes.set(Some(codes));
    }

    /// Remember the `AbilityTable::codes` computed from this `Render`.
    pub fn cache_table_codes(&self, codes: ti4_engine_core::kernel::ability::Codes) {
        self.table_codes.set(Some(codes));
    }

    /// [`Self::new`] continuing the code table of a state's lineage (`CoreState::texts.codes`): the codes this `Render` mints
    /// extend it, so they mean the same text in the state that took them back ([`Self::codes_snapshot`]).
    #[must_use]
    pub fn with_codes(self, codes: ti4_engine_core::containers::Interner) -> Self {
        *self.codes.borrow_mut() = codes;
        self
    }

    /// Make this `Render` and the state's own code table one: the texts a window minted straight into the state (the exploration's log words)
    /// reach the render, the codes the render handed out reach the state, whichever has more (one of them is always a prefix of the other: the
    /// live loop syncs after every call that can mint).
    ///
    /// Only tables of one lineage are made one (the longer continues the shorter, text for text): a `Render` that does not continue the state's table
    /// (a fresh one) is left as it is, and `false` says so; `true`: the two tables are one now.
    pub fn sync_codes(&self, state: &mut ti4_engine_core::containers::Interner) -> bool {
        let mut mine = self.codes.borrow_mut();
        match mine.len().cmp(&state.len()) {
            std::cmp::Ordering::Greater if mine.extends(state) => {
                *state = mine.clone();
                true
            }
            std::cmp::Ordering::Less if state.extends(&mine) => {
                *mine = state.clone();
                true
            }
            std::cmp::Ordering::Equal => mine.extends(state),
            _ => false,
        }
    }

    /// The code table as it stands (a clone: one refcount and the short local suffix).
    #[must_use]
    pub fn codes_snapshot(&self) -> ti4_engine_core::containers::Interner {
        self.codes.borrow().clone()
    }

    /// Register a free-form deal of the window (its `x;...` id and the net the partner's accept option reports); the index
    /// is the core's `Shape::Free`. The same id registers once.
    pub fn register_free(&self, id: &str, their_net: f64) -> u8 {
        let mut free = self.free.borrow_mut();
        let at = free.iter().position(|(known, _)| known == id).unwrap_or_else(|| {
            free.push((id.to_owned(), their_net));
            free.len() - 1
        });
        u8::try_from(at).unwrap_or(u8::MAX)
    }

    /// Replace the table of free-form deals (the live loop sets the deal the open window holds on the table, index 0).
    pub fn set_free(&self, entries: Vec<(String, f64)>) {
        *self.free.borrow_mut() = entries;
    }

    /// The registered index of a free-form id.
    #[must_use]
    pub fn free_index(&self, id: &str) -> Option<u8> {
        self.free.borrow().iter().position(|(known, _)| known == id).and_then(|i| u8::try_from(i).ok())
    }

    /// The id text of a registered free-form deal.
    #[must_use]
    pub fn free_id(&self, index: u8) -> String {
        self.free.borrow().get(usize::from(index)).map(|(id, _)| id.clone()).unwrap_or_default()
    }

    /// The net the partner's accept option reports for a registered free-form deal.
    #[must_use]
    pub fn free_net(&self, index: u8) -> f64 {
        self.free.borrow().get(usize::from(index)).map_or(0.0, |(_, net)| *net)
    }

    /// Set the position the following `prompt`/`option`/`details` calls render in; `None` clears it.
    pub fn set_position(&self, position: Option<Position>) {
        *self.position.borrow_mut() = position;
    }

    /// The position set by the caller, if any. Families that show facts of the position require it.
    #[must_use]
    pub fn position(&self) -> Option<Position> {
        self.position.borrow().clone()
    }

    /// Run `f` with `position` set, restoring the previous one afterwards.
    pub fn at<T>(&self, position: Position, f: impl FnOnce(&Self) -> T) -> T {
        let before = self.position.replace(Some(position));
        let out = f(self);
        *self.position.borrow_mut() = before;
        out
    }

    /// [`Self::new`] knowing the faction text of every seat.
    #[must_use]
    pub fn with_factions(mut self, factions: Vec<String>) -> Self {
        self.factions = factions;
        self
    }

    /// The faction text of a seat; empty when unknown.
    #[must_use]
    pub fn faction_text(&self, seat: Seat) -> &str {
        self.factions.get(usize::from(seat.0)).map_or("", String::as_str)
    }

    #[must_use]
    pub fn seat(&self, text: &str) -> Option<Seat> {
        self.seats.iter().position(|s| s == text).and_then(|i| u8::try_from(i).ok()).map(Seat)
    }

    #[must_use]
    pub fn seat_text(&self, seat: Seat) -> &str {
        &self.seats[usize::from(seat.0)]
    }

    #[must_use]
    pub fn player(&self, seat: Seat) -> PlayerId {
        PlayerId::new(self.seat_text(seat))
    }

    /// The code of a text (the same text always gets the same code in one `Render`).
    pub fn code(&self, text: &str) -> Code {
        let i = self
            .codes
            .borrow_mut()
            .intern(text.as_bytes(), ti4_engine_core::state::CODE_CAPACITY)
            .expect("fewer than 65536 distinct codes");
        Code(u16::try_from(i).expect("fewer than 65536 distinct codes"))
    }

    /// The code of an option shape (the same shape always gets the same code in one `Render`).
    pub fn shape(&self, option: &ChoiceOption) -> Code {
        let mut shapes = self.shapes.borrow_mut();
        let i = shapes.iter().position(|c| c == option).unwrap_or_else(|| {
            shapes.push(option.clone());
            shapes.len() - 1
        });
        Code(u16::try_from(i).expect("fewer than 65536 distinct shapes"))
    }

    /// How many option shapes were interned.
    #[must_use]
    pub fn shape_count(&self) -> usize {
        self.shapes.borrow().len()
    }

    /// The option shape of a code.
    #[must_use]
    pub fn shape_option(&self, code: Code) -> ChoiceOption {
        self.shapes.borrow()[usize::from(code.0)].clone()
    }

    /// The code of a text already interned, without interning it.
    #[must_use]
    pub fn find_code(&self, text: &str) -> Option<Code> {
        self.codes.borrow().find(text.as_bytes()).and_then(|i| u16::try_from(i).ok()).map(Code)
    }

    #[must_use]
    pub fn code_text(&self, code: Code) -> String {
        String::from_utf8_lossy(self.codes.borrow().get(u32::from(code.0)).expect("a code this Render issued")).into_owned()
    }
}

/// The text of a generic option (`Decline`, `Done`, `Number`) inside one ask. Fields left `None`
/// take the generic default ("decline"/"Decline", "done"/"done", count/the number).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Wording {
    pub kind: Option<String>,
    pub label: Option<String>,
    pub payload: BTreeMap<String, Value>,
    pub preview: Option<crate::wire::preview::Preview>,
}

/// The renderer of one area (or of the generic asks).
pub trait Family {
    type Ask: Copy;
    type Opt: Copy;
    type Subtype: Copy;

    /// Byte-identical prompt text (it is hashed in fingerprint V2).
    fn prompt(r: &Render, a: &Self::Ask) -> String;
    /// `DecisionContext.subtype` of the question (a machine identifier, never derived from the prompt).
    fn subtype(r: &Render, s: &Self::Subtype) -> String;
    /// The option as the string engine lists it: id, kind, label, payload.
    fn option(r: &Render, a: &Self::Ask, o: &Self::Opt) -> ChoiceOption;
    /// Inverse of `option(..).id` for the area's own options; `None` for any other id (the generic
    /// ids are tried by the registry afterwards).
    fn parse(r: &Render, a: &Self::Ask, id: &str) -> Option<Self::Opt>;

    /// Text of the generic options in this ask (default: the generic defaults).
    fn wording(_r: &Render, _a: &Self::Ask, _o: &Opt) -> Wording {
        Wording::default()
    }
    /// Whether decimal ids are [`Opt::Number`] answers of this ask (areas whose ids are numbers of
    /// their own, like system ids, say no).
    fn numbers(_a: &Self::Ask) -> bool {
        false
    }
    /// `Choice.details` (display only).
    fn details(_r: &Render, _a: &Self::Ask) -> serde_json::Map<String, Value> {
        serde_json::Map::new()
    }
    /// `Choice.free_form`.
    fn free_form(_a: &Self::Ask) -> Option<String> {
        None
    }
}

/// The family of an area that is not ported: its types are uninhabited, so no method can be called.
pub struct Unported<A, O, S>(PhantomData<(A, O, S)>);

/// A type with no values: `absurd` can never be called.
pub trait Uninhabited: Copy {
    fn absurd<T>(self) -> T;
}

macro_rules! uninhabited {
    ($($m:ident),*) => {$(
        impl Uninhabited for core::$m::Ask { fn absurd<T>(self) -> T { match self {} } }
        impl Uninhabited for core::$m::Opt { fn absurd<T>(self) -> T { match self {} } }
        impl Uninhabited for core::$m::Subtype { fn absurd<T>(self) -> T { match self {} } }
    )*};
}
uninhabited!(
    tokens
);

impl<A: Uninhabited, O: Uninhabited, S: Uninhabited> Family for Unported<A, O, S> {
    type Ask = A;
    type Opt = O;
    type Subtype = S;
    fn prompt(_: &Render, a: &A) -> String {
        a.absurd()
    }
    fn subtype(_: &Render, s: &S) -> String {
        s.absurd()
    }
    fn option(_: &Render, a: &A, _: &O) -> ChoiceOption {
        a.absurd()
    }
    fn parse(_: &Render, a: &A, _: &str) -> Option<O> {
        a.absurd()
    }
}

macro_rules! unported {
    ($m:ident) => {
        Unported<core::$m::Ask, core::$m::Opt, core::$m::Subtype>
    };
}

/// The registry: one line per area. Replace `unported!(x)` by the area's `x::Fam` when it is ported.
macro_rules! registry {
    ($($V:ident => $fam:ty),* $(,)?) => {
        /// Prompt text of a question.
        #[must_use]
        pub fn prompt(r: &Render, ask: &Ask) -> String {
            match ask {
                Ask::Generic(a) => generic::Fam::prompt(r, a),
                $(Ask::$V(a) => <$fam as Family>::prompt(r, a)),*
            }
        }

        /// `DecisionContext.subtype` text.
        #[must_use]
        pub fn subtype_text(r: &Render, s: &Subtype) -> String {
            match s {
                Subtype::Generic(s) => generic::Fam::subtype(r, s),
                $(Subtype::$V(s) => <$fam as Family>::subtype(r, s)),*
            }
        }

        /// The option as the string engine lists it.
        #[must_use]
        pub fn option(r: &Render, ask: &Ask, o: &Opt) -> ChoiceOption {
            match o {
                Opt::Decline | Opt::Done | Opt::Number(_) => {
                    let w = match ask {
                        Ask::Generic(a) => generic::Fam::wording(r, a, o),
                        $(Ask::$V(a) => <$fam as Family>::wording(r, a, o)),*
                    };
                    generic::option(o, &w)
                }
                $(Opt::$V(x) => match ask {
                    Ask::$V(a) => <$fam as Family>::option(r, a, x),
                    other => panic!("option of area {:?} in a question of area {:?}", o.area(), other.area()),
                }),*
            }
        }

        /// The option an id names in this ask (the area's own ids first, then decline/done/numbers).
        #[must_use]
        pub fn parse(r: &Render, ask: &Ask, id: &str) -> Option<Opt> {
            let (own, numbers) = match ask {
                Ask::Generic(a) => (generic::Fam::parse(r, a, id).map(|never| match never {}), generic::Fam::numbers(a)),
                $(Ask::$V(a) => (<$fam as Family>::parse(r, a, id).map(Opt::$V), <$fam as Family>::numbers(a))),*
            };
            own.or_else(|| generic::parse(id, numbers))
        }

        /// `Choice.details` and `Choice.free_form` of a question.
        fn extras(r: &Render, ask: &Ask) -> (serde_json::Map<String, Value>, Option<String>) {
            match ask {
                Ask::Generic(a) => (generic::Fam::details(r, a), generic::Fam::free_form(a)),
                $(Ask::$V(a) => (<$fam as Family>::details(r, a), <$fam as Family>::free_form(a))),*
            }
        }
    };
}

registry! {
    Tactical => tactical::Fam,
    Technology => technology::Fam,
    Objectives => objectives::Fam,
    Tokens => unported!(tokens),
    Reaction => reaction::Fam,
    Combat => combat::Fam,
    Invasion => invasion::Fam,
    Production => production::Fam,
    Trade => trade::Fam,
    Diplomacy => diplomacy::Fam,
    Explore => explore::Fam,
    Relic => relic::Fam,
    Legendary => legendary::Fam,
    Promissory => promissory::Fam,
    Cards => cards::Fam,
    Leaders => leaders::Fam,
    Strategy => strategy::Fam,
    Agenda => agenda::Fam,
    Turn => turn::Fam,
}

/// Phase of the core to the wire phase.
#[must_use]
pub const fn phase(p: CorePhase) -> Phase {
    match p {
        CorePhase::Strategy => Phase::Strategy,
        CorePhase::Action => Phase::Action,
        CorePhase::Status => Phase::Status,
        CorePhase::Agenda => Phase::Agenda,
    }
}

/// A typed decision point and its rendering, index-aligned: `rendered.options[i]` is
/// `choice.opts[i]`. This is what the rule bot receives (D3): it reads the typed `ask`, `ctx` and
/// options, answers with an index, and the index is looked up in `rendered` for the id.
#[derive(Debug, Clone)]
pub struct Typed {
    pub choice: core::Choice,
    pub rendered: Choice,
}

impl Typed {
    /// Render a core choice at the given phase and round (state facts the core `Choice` omits).
    #[must_use]
    pub fn new(r: &Render, choice: core::Choice, phase_now: CorePhase, round: u32) -> Self {
        let rendered = render_choice(r, &choice, phase_now, round);
        Self { choice, rendered }
    }

    /// [`Self::new`] with the position the families read their facts from.
    #[must_use]
    pub fn new_at(r: &Render, position: Position, choice: core::Choice, phase_now: CorePhase, round: u32) -> Self {
        r.at(position, |r| Self::new(r, choice, phase_now, round))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.choice.opts.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.choice.opts.is_empty()
    }

    #[must_use]
    pub fn opt(&self, i: usize) -> Option<&Opt> {
        self.choice.opts.as_slice().get(i)
    }

    #[must_use]
    pub fn option(&self, i: usize) -> Option<&ChoiceOption> {
        self.rendered.options.get(i)
    }

    /// The rendered option answering with this typed one.
    #[must_use]
    pub fn option_of(&self, o: &Opt) -> Option<&ChoiceOption> {
        self.choice.opts.index_of(o).and_then(|i| self.option(i))
    }
}

/// Render a core choice into today's string `Choice`.
#[must_use]
pub fn render_choice(r: &Render, c: &core::Choice, phase_now: CorePhase, round: u32) -> Choice {
    let (details, free_form) = extras(r, &c.ask);
    let options: Vec<_> = c.opts.iter().map(|o| option(r, &c.ask, o)).collect();
    let multi_pick = match &c.ask {
        Ask::Production(a) => production::multi_pick(a, &options),
        _ => None,
    };
    Choice {
        player: r.player(c.actor),
        prompt: prompt(r, &c.ask),
        options,
        context: Some(generic::encode_context(r, &c.ctx, c.actor, phase(phase_now), round, subtype_text(r, &c.ctx.subtype))),
        details,
        free_form,
        multi_pick,
    }
}

/// The answer an id gives to a question.
#[must_use]
pub fn parse_pick(r: &Render, ask: &Ask, id: &str) -> Option<core::Pick> {
    parse(r, ask, id).map(core::Pick::Opt)
}

/// A context for tests and tools: the string engine's `DecisionContext` type is re-exported here.
pub type Context = DecisionContext;

#[cfg(test)]
mod sync_codes_tests {
    use super::Render;
    use ti4_engine_core::containers::Interner;

    /// Lane F2c/F2d: the render and the state's table become one when one is a prefix of the other, whichever is longer; two tables that disagree
    /// (also at the FIRST text) are left alone.
    #[test]
    fn the_render_and_the_state_share_one_code_table_only_when_one_is_a_prefix_of_the_other() {
        let render = Render::new(Vec::new());
        let a = render.code("alpha");
        // The state is behind: it takes the render's table.
        let mut state = Interner::default();
        render.sync_codes(&mut state);
        assert_eq!(state.get(u32::from(a.0)), Some("alpha".as_bytes()), "the state took the render's texts");
        // The state minted a text of its own: the render takes it.
        let minted = state.intern(b"beta", usize::MAX).expect("room");
        render.sync_codes(&mut state);
        assert_eq!(render.codes_snapshot().get(minted), Some("beta".as_bytes()), "the render knows the text the state minted, at the same code");
        // Two tables that disagree at the first text are left alone.
        let mine = render.codes_snapshot();
        let mut texts: Vec<Vec<u8>> = (0..mine.len() as u32).map(|i| mine.get(i).expect("text").to_vec()).collect();
        texts[0] = b"other".to_vec();
        texts.pop();
        let mut apart = Interner::sealed(texts.iter().map(Vec::as_slice));
        let before = apart.len();
        render.sync_codes(&mut apart);
        assert_eq!((apart.len(), apart.get(0)), (before, Some("other".as_bytes())), "a table built apart is untouched");
        // ... and at a later text.
        let mut late = Interner::default();
        late.intern(render.codes_snapshot().get(0).expect("a first text"), usize::MAX);
        late.intern(b"different", usize::MAX);
        let kept = late.clone();
        render.sync_codes(&mut late);
        assert_eq!(late.len(), kept.len());
    }
}
