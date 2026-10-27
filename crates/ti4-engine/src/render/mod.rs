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

use crate::choice::{Choice, ChoiceOption};
use crate::decision_context::DecisionContext;

pub mod generic;

pub mod agenda;
pub mod cards;
pub mod combat;
pub mod diplomacy;
pub mod explore;
pub mod invasion;
pub mod leaders;
pub mod legendary;
pub mod objectives;
pub mod production;
pub mod promissory;
pub mod reaction;
pub mod relic;
pub mod strategy;
pub mod tactical;
pub mod technology;
pub mod tokens;
pub mod trade;

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
    /// Texts the core holds as a [`Code`] (open string sets until X1 closes them).
    codes: RefCell<Vec<String>>,
    /// The position of the question being rendered, when the caller set one (`None` for the families
    /// that need none; see [`Position`]).
    position: RefCell<Option<Position>>,
}

impl Render {
    #[must_use]
    pub fn new(seats: Vec<String>) -> Self {
        Self { seats, codes: RefCell::new(Vec::new()), position: RefCell::new(None) }
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
        let mut codes = self.codes.borrow_mut();
        let i = codes.iter().position(|c| c == text).unwrap_or_else(|| {
            codes.push(text.to_owned());
            codes.len() - 1
        });
        Code(u16::try_from(i).expect("fewer than 65536 distinct codes"))
    }

    /// The code of a text already interned, without interning it.
    #[must_use]
    pub fn find_code(&self, text: &str) -> Option<Code> {
        self.codes.borrow().iter().position(|c| c == text).and_then(|i| u16::try_from(i).ok()).map(Code)
    }

    #[must_use]
    pub fn code_text(&self, code: Code) -> String {
        self.codes.borrow()[usize::from(code.0)].clone()
    }
}

/// The text of a generic option (`Decline`, `Done`, `Number`) inside one ask. Fields left `None`
/// take the generic default ("decline"/"Decline", "done"/"done", count/the number).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Wording {
    pub kind: Option<String>,
    pub label: Option<String>,
    pub payload: BTreeMap<String, Value>,
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
    tactical, technology, objectives, tokens, combat, invasion, trade, diplomacy, explore,
    relic, legendary, promissory, cards, strategy, agenda
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
    Tactical => unported!(tactical),
    Technology => unported!(technology),
    Objectives => unported!(objectives),
    Tokens => unported!(tokens),
    Reaction => reaction::Fam,
    Combat => unported!(combat),
    Invasion => unported!(invasion),
    Production => production::Fam,
    Trade => unported!(trade),
    Diplomacy => unported!(diplomacy),
    Explore => unported!(explore),
    Relic => unported!(relic),
    Legendary => unported!(legendary),
    Promissory => unported!(promissory),
    Cards => unported!(cards),
    Leaders => leaders::Fam,
    Strategy => unported!(strategy),
    Agenda => unported!(agenda),
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
    Choice {
        player: r.player(c.actor),
        prompt: prompt(r, &c.ask),
        options: c.opts.iter().map(|o| option(r, &c.ask, o)).collect(),
        context: Some(generic::encode_context(r, &c.ctx, c.actor, phase(phase_now), round, subtype_text(r, &c.ctx.subtype))),
        details,
        free_form,
    }
}

/// The answer an id gives to a question.
#[must_use]
pub fn parse_pick(r: &Render, ask: &Ask, id: &str) -> Option<core::Pick> {
    parse(r, ask, id).map(core::Pick::Opt)
}

/// A context for tests and tools: the string engine's `DecisionContext` type is re-exported here.
pub type Context = DecisionContext;
