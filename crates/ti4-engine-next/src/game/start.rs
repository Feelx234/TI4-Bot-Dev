//! A game from a position the setup crate built (`ti4_engine_core_setup::start_game`): the core is already loaded, no `Codec::load` runs.
//! The string `GameState` of such a game is the mirror of its core (the same `materialize` every step ends with), over a shell that
//! holds only the seats; nothing of the setup is ever written to a string position (docs/SETUP_NATIVE_DESIGN.md).

use std::collections::BTreeMap;
use std::rc::Rc;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core_setup::{CoreStart, TableSpec};
use ti4_model::content_types::POK;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use super::{Game, GameError, Native, secondary_view};
use crate::core_sync::codec::rng::TextSeeds;
use crate::core_sync::{Ctx, materialize};
use crate::render::Render;
use crate::wire::choice::{SeededRandom, Table};
use crate::wire::rng::GameRng;

/// A table set up by the setup crate, with the tables the core needs to be played ([`Game::from_start`]) or shown as a string state
/// ([`GameStart::opening_state`]).
#[derive(Debug, Clone)]
pub struct GameStart {
    start: CoreStart,
    cx: Ctx,
    /// A `GameState` with the seats of the table and nothing else: the codecs store every slice over it.
    shell: GameState,
    seed: u64,
    /// The board as the host built it, before the laws and the lanes were applied (what a server stores and draws).
    raw: Galaxy,
}

impl GameStart {
    /// Set a table up (`ti4_engine_core_setup::start_game`).
    ///
    /// # Errors
    /// The message of the setup's refusal (an unknown faction, Thunder's Edge content, ...) or of a position the string side cannot show.
    pub fn new(spec: &TableSpec<'_>, galaxy: Galaxy) -> Result<Self, String> {
        let raw = galaxy.clone();
        let start = ti4_engine_core_setup::start_game(spec, galaxy).map_err(|e| e.to_string())?;
        Self::of(start, spec.seed, raw)
    }

    /// Wrap a position the setup crate built.
    ///
    /// # Errors
    /// More than 16 owners, an owner id that is no text.
    pub fn of(start: CoreStart, seed: u64, raw: Galaxy) -> Result<Self, String> {
        let cx = Ctx::of_core(&start.core).map_err(|e| e.to_string())?;
        let players: Vec<PlayerId> = start.core.players().iter().filter_map(|p| cx.owners.id(p.seat)).map(PlayerId::new).collect();
        let shell = GameState::new(&players, &[], BTreeMap::new(), None, 0);
        Ok(Self { start, cx, shell, seed, raw })
    }

    /// The table as `ti4_host::setup::create` returns it: the string state a server stores as the opening of a game, for replay and
    /// recovery. It is the mirror of the core before the frontier tokens, the baselines of a seat's holdings and the map are added
    /// (what [`Game::with_galaxy`] and `Game::with_table` did to it before the first load).
    ///
    /// # Errors
    /// A typed value without a text (never for a position the setup builds).
    pub fn opening_state(&self) -> Result<GameState, GameError> {
        materialize::materialize(&self.start.opening, &self.cx, &self.shell).map_err(|e| GameError::Core(format!("the opening cannot be shown: {e}")))
    }

    /// The board of the game, the laws and the printed lanes applied (what [`Game::galaxy`] returns).
    #[must_use]
    pub const fn galaxy(&self) -> &Galaxy {
        &self.start.galaxy
    }

    /// The board as the host built it (what `ti4_host::setup::create` returns with the state).
    #[must_use]
    pub const fn raw_galaxy(&self) -> &Galaxy {
        &self.raw
    }

    /// The seed of the game.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// The setup crate's result.
    #[must_use]
    pub const fn core_start(&self) -> &CoreStart {
        &self.start
    }
}

/// The pieces of a [`GameStart`] a driver of the core alone (the bridge's `NativeGame`) takes over: no `Game` is made.
pub struct StartParts {
    /// The loaded core.
    pub core: ti4_engine_core::state::CoreState,
    /// The owner and text tables of the core.
    pub cx: Ctx,
    /// The string mirror of the core (the position `Game::state` would hold).
    pub state: GameState,
    /// The board, the laws and the lanes applied.
    pub galaxy: Galaxy,
    pub seed: u64,
}

impl GameStart {
    /// Take the start apart for a driver that plays the core itself.
    ///
    /// # Errors
    /// [`GameError::Core`] when the core cannot be shown as a string state.
    pub fn into_parts(self) -> Result<StartParts, GameError> {
        let Self { start, cx, shell, seed, raw: _ } = self;
        let CoreStart { core, opening: _, galaxy } = start;
        let state = materialize::materialize(&core, &cx, &shell).map_err(|e| GameError::Core(format!("the position cannot be shown: {e}")))?;
        Ok(StartParts { core, cx, state, galaxy, seed })
    }
}

impl<'a> Game<'a> {
    /// The game of a table the setup crate built, played by seats that draw uniformly from one seeded legal stream (the dice share the
    /// seed): [`Game::with_seeded_random`] without the string position and without the load.
    ///
    /// # Errors
    /// [`GameError::Core`] when the core cannot be shown as a string state.
    pub fn from_start(start: GameStart, content: &'a ContentStore) -> Result<Self, GameError> {
        let seed = start.seed;
        Self::from_start_with_table(start, content, Table::with_default(Box::new(SeededRandom::new(seed))))
    }

    /// [`Game::from_start`] with explicit deciders (`Game::with_table`).
    ///
    /// # Errors
    /// As [`Game::from_start`].
    pub fn from_start_with_table(start: GameStart, content: &'a ContentStore, table: Table) -> Result<Self, GameError> {
        let GameStart { start, cx, shell, seed, raw: _ } = start;
        let CoreStart { core, opening: _, galaxy } = start;
        let state = materialize::materialize(&core, &cx, &shell).map_err(|e| GameError::Core(format!("the position cannot be shown: {e}")))?;
        // (The options that name a seat by faction -- Politics' speaker, Trade's refreshed seats -- need the faction texts.)
        let factions = cx
            .owners
            .ids()
            .iter()
            .map(|id| state.players.iter().find(|p| p.id.as_str() == id).map_or_else(String::new, |p| p.faction.as_str().to_owned()))
            .collect();
        let render = Render::new(cx.owners.ids().to_vec()).with_factions(factions).with_codes(core.texts.codes.clone());
        let native = Native { seeds: Rc::new(TextSeeds::of(&cx)), kernel: crate::live::kernel::kernel_for(POK), base: state.clone(), core, cx, render, after_glue: false };
        let secondary = secondary_view(&native.core, &native.cx);
        Ok(Self {
            state,
            table,
            events: Vec::new(),
            content,
            sources: POK,
            galaxy: Some(galaxy),
            rng: GameRng::new(seed),
            native: Some(native),
            rolls: Vec::new(),
            blocked: None,
            observation: None,
            aftermath_observer: None,
            strategic_observer: None,
            steps: 0,
            secondary,
            participation: None,
            rng_sync: None,
            round_income: (0, None),
        })
    }
}
