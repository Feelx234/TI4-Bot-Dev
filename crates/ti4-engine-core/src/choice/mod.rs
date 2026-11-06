//! The decision model (docs/ENGINE_CORE_STATE.md section 2; package F4): what a decision point is,
//! as typed values with no text.
//!
//! * [`Choice`]: the actor, the [`Ask`] (what is asked, with parameters), the [`Ctx`] (why, display
//!   facts that are part of the fingerprint) and the [`Opts`] (the options in today's order).
//! * [`Opt`]: one answer. [`Pick`]: the answer given. Options are derived from `(state, ask)`, never
//!   stored in the state.
//! * One sub-enum per rule area (`choice/<area>.rs`, owned by the area's package). The top-level
//!   enums are generated from the one list in `areas!` below, so adding an area is one line here and
//!   adding an option kind touches only the area's file.
//!
//! # For bots
//!
//! The rule bot consumes these values directly: no option id, label or prompt is parsed. `Opt` is
//! `Copy + Eq + Ord + Hash`, `Choice::opts` is index-aligned with the rendered `ChoiceOption` list
//! (the adapter's `render::Typed` pairs them), and an answer is `Pick::Opt(opts[i])` or just `i`.
//! See docs/ENGINE_CORE_DECISIONS.md.
//!
//! Everything here is `Copy` (or a small inline list of `Copy`) so a decision point clones cheaply.

use crate::containers::SmallList;
use crate::generated::{Agenda, ActionCard, Planet, StrategyCard, System, Unit};
use crate::ids::Seat;
use crate::state::EventKind;

pub mod generic;
pub mod silent;
pub mod silent3;
pub mod activation;
pub mod silent4;

macro_rules! areas {
    ($($(#[$m:meta])* $variant:ident($module:ident)),* $(,)?) => {
        $( pub mod $module; )*

        /// The rule areas (one per `choice/<area>.rs`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Area { Generic, $($variant),* }

        impl Area {
            /// Every area, in declaration order.
            pub const ALL: &'static [Area] = &[Area::Generic, $(Area::$variant),*];
        }

        /// What is asked.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Ask { Generic(generic::Ask), $($variant($module::Ask)),* }

        impl Ask {
            #[must_use]
            pub const fn area(&self) -> Area {
                match self { Self::Generic(_) => Area::Generic, $(Self::$variant(_) => Area::$variant),* }
            }
        }

        /// The machine identifier of the question.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Subtype { Generic(generic::Subtype), $($variant($module::Subtype)),* }

        impl Subtype {
            #[must_use]
            pub const fn area(&self) -> Area {
                match self { Self::Generic(_) => Area::Generic, $(Self::$variant(_) => Area::$variant),* }
            }
        }

        /// One answer. `Decline`, `Done` and `Number` are the generic families; every other variant
        /// belongs to one area.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Opt { Decline, Done, Number(i32), $($variant($module::Opt)),* }

        impl Opt {
            /// The area that owns this option; [`Area::Generic`] for decline, done and numbers.
            #[must_use]
            pub const fn area(&self) -> Area {
                match self {
                    Self::Decline | Self::Done | Self::Number(_) => Area::Generic,
                    $(Self::$variant(_) => Area::$variant),*
                }
            }
        }
    };
}

areas! {
    Tactical(tactical),
    Technology(technology),
    Objectives(objectives),
    Tokens(tokens),
    Reaction(reaction),
    Combat(combat),
    Invasion(invasion),
    Production(production),
    Trade(trade),
    Diplomacy(diplomacy),
    Explore(explore),
    Relic(relic),
    Legendary(legendary),
    Promissory(promissory),
    Cards(cards),
    Leaders(leaders),
    Strategy(strategy),
    Agenda(agenda),
    Turn(turn),
}

impl Opt {
    #[must_use]
    pub const fn is_decline(&self) -> bool {
        matches!(self, Self::Decline)
    }

    /// True for the options that end a multi-step decision without doing anything more
    /// (`Decline` and `Done`). Area-specific "finish" options (finish movement, ...) are not
    /// covered: the area's `Opt` says so itself.
    #[must_use]
    pub const fn is_generic_exit(&self) -> bool {
        matches!(self, Self::Decline | Self::Done)
    }
}

/// An answer given to a [`Choice`]. Free-form answers (the transaction deal, today's `free_form`
/// prefix) get their variant when package B4 defines the deal type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pick {
    Opt(Opt),
    Number(i32),
    /// The faces of a chance question ([`generic::Ask::Chance`]): the resolver's (play) or the search's.
    Chance(crate::chance::Faces),
}

impl From<Opt> for Pick {
    fn from(o: Opt) -> Self {
        Self::Opt(o)
    }
}

/// What raised the decision (`DecisionSource`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    Rule(generic::Rule),
    StrategyCard { card: StrategyCard, secondary: bool },
    ActionCard(ActionCard),
    FactionAbility(generic::Content),
    Content(generic::Content),
    Agenda(Agenda),
    /// A reaction window opened by this event kind.
    Reaction(EventKind),
}

/// The subject of the question (`DecisionTarget`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    System(System),
    Planet { system: System, planet: Planet },
    Unit { system: System, unit: Unit },
    Seat(Seat),
}

/// `ConstraintKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OwedKind {
    Resources,
    Influence,
    TradeGoods,
    CommandTokens,
    ProductionCapacity,
    TransportCapacity,
    FleetSupply,
    UnitsToRemove,
    Votes,
}

/// A quantity still owed inside a decision already under way (`OutstandingConstraint`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Owed {
    pub kind: OwedKind,
    pub amount: i64,
    pub paid: i64,
}

impl Owed {
    #[must_use]
    pub const fn remaining(&self) -> i64 {
        let left = self.amount - self.paid;
        if left < 0 { 0 } else { left }
    }
}

/// Why the question exists and what it concerns. Phase and round are in the state's `Meta`, the actor
/// is [`Choice::actor`]; the display-only parts of today's `DecisionContext` (trigger, hit, card
/// previews) are built by the adapter from the ask.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ctx {
    pub origin: Origin,
    pub subtype: Subtype,
    pub optional: bool,
    pub target: Option<Target>,
    pub outstanding: SmallList<Owed, 4>,
    pub space_battle: bool,
    pub invasion_seq: Option<u64>,
}

/// The options of a decision, in the order the string engine lists them (observable: option order
/// feeds the fingerprint and the bots).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Opts(pub SmallList<Opt, 16>);

impl Opts {
    #[must_use]
    pub fn as_slice(&self) -> &[Opt] {
        self.0.as_slice()
    }

    pub fn push(&mut self, o: Opt) {
        self.0.push(o);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Opt> {
        self.0.as_slice().iter()
    }

    /// Position of an option in the list (what a bot answers with).
    #[must_use]
    pub fn index_of(&self, o: &Opt) -> Option<usize> {
        self.0.as_slice().iter().position(|x| x == o)
    }
}

impl FromIterator<Opt> for Opts {
    fn from_iter<I: IntoIterator<Item = Opt>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// A decision point: who is asked what, why, and the options.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Choice {
    pub actor: Seat,
    pub ask: Ask,
    pub ctx: Ctx,
    pub opts: Opts,
}

impl Choice {
    /// The answer for the option at `index` (what the adapter turns into the option id).
    #[must_use]
    pub fn pick(&self, index: usize) -> Option<Pick> {
        self.opts.as_slice().get(index).map(|o| Pick::Opt(*o))
    }

    /// Whether the actor may decline (an option that is [`Opt::Decline`] exists).
    #[must_use]
    pub fn can_decline(&self) -> bool {
        self.opts.iter().any(Opt::is_decline)
    }

    /// Index of the generic decline option.
    #[must_use]
    pub fn decline_index(&self) -> Option<usize> {
        self.opts.iter().position(Opt::is_decline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_options_belong_to_generic() {
        assert_eq!(Opt::Decline.area(), Area::Generic);
        assert_eq!(Opt::Number(3).area(), Area::Generic);
        assert_eq!(Ask::Generic(generic::Ask::Confirm).area(), Area::Generic);
    }

    #[test]
    fn choice_indexing() {
        let opts: Opts = [Opt::Number(1), Opt::Decline].into_iter().collect();
        let c = Choice {
            actor: Seat(0),
            ask: Ask::Generic(generic::Ask::Count { lo: 0, hi: 1 }),
            ctx: Ctx {
                origin: Origin::Rule(generic::Rule::R22),
                subtype: Subtype::Generic(generic::Subtype::Unclassified),
                optional: true,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts,
        };
        assert_eq!(c.decline_index(), Some(1));
        assert_eq!(c.pick(0), Some(Pick::Opt(Opt::Number(1))));
        assert_eq!(c.opts.index_of(&Opt::Decline), Some(1));
        assert!(c.pick(2).is_none());
    }

    #[test]
    fn opt_stays_small() {
        assert!(std::mem::size_of::<Opt>() <= 16, "{}", std::mem::size_of::<Opt>());
        // A chance answer (`Pick::Chance`) fits beside the option's niche: a `Pick` is no bigger than an `Opt` (lane C3, the wasm budget).
        assert!(std::mem::size_of::<Pick>() <= 16, "{}", std::mem::size_of::<Pick>());
    }
}
