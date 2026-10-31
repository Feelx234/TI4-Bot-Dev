//! The kernel (layer L3, docs/ENGINE_CORE_STATE.md 4.1; package A5): events, triggers, reaction windows,
//! the ability registry, `Next`, the `fx` primitives and the faction [`hooks`] table.
//!
//! * [`Resolver`]: the timing machine (`timing.rs`): WHEN/AFTER windows around an emitted event, player
//!   order, eligibility, frequency scopes, the absolute "cannot" effects, nested emission, checkpoints.
//!   It is data plus the ordering algorithm; conditions, effects and questions go to a [`Host`].
//! * [`hooks::HookTable`]: what faction modules plug into (the typed `factions::Hooks`).
//! * [`Next`]: what a rule area returns instead of calling another area (L4 peers do not import each
//!   other).
//! * [`fx`]: the shared state-changing primitives that also fire their trigger.
//! * [`stage`]: the window contract (`Window`: stage, step, ask_of, answer) and `Sx`, the services of a window
//!   (emit through the resolver, observers, nested questions). The driver (`crate::driver`) runs windows.
//! * [`trigger`]: the display facts of a reaction window, built from the event alone.
//! * [`reaction`]: the printed-window table that binds action-card reactions to events.
//!
//! The kernel never imports a rule area or a faction: it defines [`Kernel`] (the hook table and the
//! content scope) and [`Cx`] (state, kernel, and the question channel) and the areas take them as
//! arguments.

pub mod ability;
pub mod children;
pub mod commander_walk;
pub mod fx;
pub mod fx_agenda;
pub mod fx_explore;
pub mod fx_ground;
pub mod fx_move;
pub mod fx_perform;
pub mod fx_pay;
pub mod fx_plots;
pub mod fx_relic;
pub mod fx_strategy;
pub mod hooks;
pub mod hits;
pub mod next;
pub mod read;
pub mod reaction;
pub mod resolver;
pub mod seam;
pub mod stage;
pub mod trigger;

pub use hooks::HookTable;
pub use next::Next;
pub use ability::{AbilityTable, Eff, NestedFacts, RulesHost, Slot, Spec};
pub use stage::{Illegal, Sx, Tape, Window};
pub use resolver::{AbilityDef, Checkpoint, Host, Registry, Resolver, TimingError};

use crate::choice::{Choice, Pick};
use crate::generated::Source;
use crate::state::CoreState;

/// The expansion scope of a game (`SourceSet`): a bitset over [`Source`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Sources(pub u8);

impl Sources {
    /// No source.
    pub const NONE: Self = Self(0);

    /// The base game, Prophecy of Kings and the codices.
    #[must_use]
    pub const fn with(self, s: Source) -> Self {
        Self(self.0 | (1 << s as u8))
    }

    /// Whether a source is in the scope.
    #[must_use]
    pub const fn has(self, s: Source) -> bool {
        self.0 & (1 << s as u8) != 0
    }

    /// Every source.
    #[must_use]
    pub const fn all() -> Self {
        Self((1 << Source::COUNT as u8) - 1)
    }
}

/// What a rule reads besides the state: the hook table and the scope of the game. Cheap to share
/// (`&Kernel`); built once per game.
#[derive(Clone, Default)]
pub struct Kernel {
    pub hooks: HookTable,
    pub sources: Sources,
}

impl Kernel {
    /// A kernel with an empty hook table (no faction modules) and every source in scope.
    #[must_use]
    pub fn plain() -> Self {
        Self { hooks: HookTable::new(), sources: Sources::all() }
    }
}

/// The channel through which a rule asks a seat a question (the string engine's `Table`): the typed
/// [`Choice`] goes in, the [`Pick`] comes back. The driver implements it with the real deciders; tests
/// and the dual harness with scripted ones.
pub trait Asker {
    fn pick(&mut self, state: &CoreState, choice: &Choice) -> Pick;
}

/// An asker that always takes the first option (a bot with no opinion; the fork rule).
#[derive(Debug, Clone, Copy, Default)]
pub struct FirstOption;

impl Asker for FirstOption {
    fn pick(&mut self, _: &CoreState, choice: &Choice) -> Pick {
        choice.pick(0).unwrap_or(Pick::Opt(crate::choice::Opt::Decline))
    }
}

/// The mutable services a rule may use: the state, the kernel, and the question channel. The string
/// engine's `TimingContext` minus the content store (static tables) and the table/dice/rng (the dice
/// history and the streams are state slices).
pub struct Cx<'a> {
    pub state: &'a mut CoreState,
    pub kernel: &'a Kernel,
    pub asker: &'a mut dyn Asker,
}

impl<'a> Cx<'a> {
    pub fn new(state: &'a mut CoreState, kernel: &'a Kernel, asker: &'a mut dyn Asker) -> Self {
        Self { state, kernel, asker }
    }

    /// Put a question and take the answer.
    pub fn ask(&mut self, choice: &Choice) -> Pick {
        self.asker.pick(self.state, choice)
    }
}
