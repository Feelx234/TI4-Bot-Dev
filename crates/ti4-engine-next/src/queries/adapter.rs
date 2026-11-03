//! The typed position that the movement and reach queries read (copy of the second half of the legacy `core_adapter.rs`): the loaded slices
//! movement needs, one seat's movement rules for every destination, and the per-position caches `Observed` keeps. Reads only the core.

use ti4_content::galaxy::Galaxy;
use ti4_engine_core::containers::Cow;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::movement::CoreRules;
use ti4_engine_core::query::reach::{SeatFleet as CoreSeatFleet, SeatReach as CoreSeatReach};
use ti4_engine_core::state::{CoreState, Slice, SliceSet};
use ti4_engine_core::{Seat, SystemIx, UnitIx};
use ti4_model::content_types::SourceSet;
use ti4_model::state::GameState;

use crate::core_sync::codec::Sources as CodecSources;
use crate::core_sync::{CoreView, Ctx};
use crate::generated::IdStr;
pub use crate::ties::core_adapter::{kernel_for, system_id, system_ix};

/// The typed unit of a unit type id.
#[must_use]
pub fn unit_ix(id: &str) -> Option<UnitIx> {
    <UnitIx as IdStr>::from_id(id)
}

type MovableRow = Vec<Option<std::rc::Rc<[super::tactical::Movable]>>>;

/// A position as the typed rules read it: the loaded slices movement needs, the owner table that maps player ids to seats, and the kernel.
/// Built once per `Observed`, on the first typed question.
pub struct Position {
    view: CoreView,
    cx: Ctx,
    kernel: &'static Kernel,
}

/// The key texts of the marks the movement hooks read (`ti4-engine-core-factions`): Crimson's sever
/// tokens and active breaches, the Empyrean's marks, the Firmament agent, the Ghosts' agent link, the
/// Nekro Z tokens, the Nomad hero, the Cabal's Crucible, the Saar agent, Winnu's Imperator and Law's
/// Order. Every other mark is left out of the reach view.
const REACH_MARKS: [&str; 11] = [
    "cabal:crucible:",
    "crimson:breach:active:",
    "crimson:sever:",
    "empyrean|",
    "firmament:agent",
    "ghost:agent_link",
    "keleres|laws_blank",
    "nekro:token:",
    "nomad:hero:",
    "private:",
    "saar:agent:boost:",
];

/// The board as movement reads it: the units of every space area and planet, command tokens, which
/// systems have an entry, who controls each planet (Mecatol Rex and Tempesta are asked) and where a
/// placed planet sits. Not read: exhaustion, attachments, coexistence, purges (the full codec copies
/// a chunk of the planet table for each of them).
fn load_reach_board(cx: &Ctx, gs: &GameState, core: &mut CoreState) -> Option<()> {
    use ti4_engine_core::generated::Planet;
    let seat = |id: &ti4_model::id::PlayerId| cx.owners.seat(id.as_str());
    let piece = |u: &ti4_model::units::Unit| -> Option<ti4_engine_core::state::Piece> {
        Some(
            ti4_engine_core::state::Piece::new(unit_ix(u.type_id.as_str())?, seat(&u.owner)?)
                .with_sustained(u.sustained_damage)
                .with_galvanized(u.galvanized),
        )
    };
    let board = &mut core.board;
    for (sid, st) in &gs.board {
        let cell = board.sys_mut(system_ix(sid.as_str())?);
        for u in &st.units {
            cell.units.push(piece(u)?);
        }
        for p in &st.command_tokens {
            cell.cmd.insert(seat(p)?);
        }
        cell.flags.set(ti4_engine_core::state::SysFlags::TOUCHED, true);
        for (pid, owner) in &st.planet_control {
            board.planet_mut(<Planet as IdStr>::from_id(pid.as_str())?).control = Some(seat(owner)?);
        }
        for (pid, units) in &st.planet_units {
            let cell = board.planet_mut(<Planet as IdStr>::from_id(pid.as_str())?);
            for u in units {
                cell.units.push(piece(u)?);
            }
        }
    }
    for (pid, sid) in &gs.placed_planets {
        board.planet_mut(<Planet as IdStr>::from_id(pid.as_str())?).placed_in = Some(system_ix(sid.as_str())?);
    }
    core.mark_loaded(Slice::Board);
    Some(())
}

/// The slices the movement rules read, loaded the cheap way: the board, the players and the map by
/// their codecs; of the explore, agenda, transient and marks slices only the few facts movement reads
/// (the full codecs load every exploration record, every law outcome and every mark).
fn load_reach(cx: &mut Ctx, gs: &GameState, galaxy: &Galaxy) -> Option<CoreState> {
    use ti4_engine_core::generated::{Agenda, Wormhole};
    use ti4_engine_core::keys::Outcome;
    let src = CodecSources { galaxy: Some(galaxy), rng: None };
    let mut core = CoreState::with_loaded(SliceSet::EMPTY);
    for slice in [Slice::Meta, Slice::Players, Slice::Map] {
        crate::core_sync::Codec::load_slice(cx, gs, src, slice, &mut core).ok()?;
    }
    load_reach_board(cx, gs, &mut core)?;
    let e = &mut core.explore;
    for (set, texts) in [(&mut e.ingress, &gs.ingress_tokens), (&mut e.breach, &gs.breach_tokens)] {
        for t in texts {
            set.insert(system_ix(t.as_str())?);
        }
    }
    for (kind, system) in &gs.wormhole_tokens {
        e.wormhole_tokens.insert(<Wormhole as IdStr>::from_id(kind)?, system_ix(system.as_str())?);
    }
    core.mark_loaded(Slice::Explore);
    if gs.laws.contains_key("shared_research") {
        core.agenda.laws.insert(Agenda::SharedResearch, Outcome::Empty);
    }
    core.mark_loaded(Slice::Agenda);
    for (system, value) in &gs.gravleash_move_values {
        core.transient.movement.gravleash_move_values.insert(system_ix(system.as_str())?, *value);
    }
    core.mark_loaded(Slice::Transient);
    let mut marks = ti4_engine_core::state::MarkMap::new();
    for (key, value) in &gs.faction_marks {
        if REACH_MARKS.iter().any(|p| key.starts_with(p)) {
            let (k, v) = cx.keys.parse_mark(&mut core.texts.blobs, key, value)?;
            marks.insert(k, v);
        }
    }
    core.marks.tail = Cow::new(marks);
    core.mark_loaded(Slice::Marks);
    Some(core)
}

impl Position {
    /// `None` when a value of the position cannot be held by the typed state (the caller asks the
    /// string engine).
    #[must_use]
    pub fn new(
        state: &GameState,
        sources: SourceSet,
        galaxy: &Galaxy,
    ) -> Option<std::rc::Rc<Self>> {
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::from_state(load_reach(&mut cx, state, galaxy)?);
        Some(std::rc::Rc::new(Self { view, cx, kernel: kernel_for(sources) }))
    }

    /// A position the caller may adjust (not shared): `None` when the core cannot hold the state.
    #[must_use]
    pub fn new_owned(state: &GameState, sources: SourceSet, galaxy: &Galaxy) -> Option<Self> {
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::from_state(load_reach(&mut cx, state, galaxy)?);
        Some(Self { view, cx, kernel: kernel_for(sources) })
    }

    /// Pretend `system` is the active one (the routes of a ship are asked towards a system that is not activated yet).
    pub fn set_active_system(&mut self, system: SystemIx) {
        let mut core = std::mem::replace(&mut self.view, CoreView::from_state(CoreState::with_loaded(SliceSet::EMPTY))).into_state();
        core.meta.active_system = Some(system);
        self.view = CoreView::from_state(core);
    }

    /// The seat of a player id.
    #[must_use]
    pub fn seat(&self, player: &ti4_model::id::PlayerId) -> Option<Seat> {
        self.cx.owners.seat(player.as_str())
    }

    /// The typed state.
    #[must_use]
    pub fn core(&self) -> &CoreState {
        &self.view
    }

    #[must_use]
    pub const fn kernel(&self) -> &'static Kernel {
        self.kernel
    }
}

/// One seat's movement rules for every destination of one position, built by the typed core from the
/// hook table (no string `MovementRules`). The rules of a destination are built once and shared.
pub struct SeatReach {
    pos: std::rc::Rc<Position>,
    reach: CoreSeatReach,
    rules: std::cell::RefCell<Vec<Option<std::rc::Rc<CoreRules>>>>,
}

impl SeatReach {
    /// `effects`: also the cards in play (what a real move and `movable_into` use); without it, the
    /// rules `Observed::can_reach` builds (laws only, no cards in hand).
    #[must_use]
    pub fn new(pos: &std::rc::Rc<Position>, player: &ti4_model::id::PlayerId, effects: bool) -> Option<Self> {
        let seat = pos.seat(player)?;
        let reach = CoreSeatReach::new(pos.core(), pos.kernel, seat, effects)?;
        Some(Self { pos: pos.clone(), reach, rules: std::cell::RefCell::new(Vec::new()) })
    }

    /// The rules for moving into `destination`.
    #[must_use]
    pub fn rules_for(&self, destination: SystemIx) -> std::rc::Rc<CoreRules> {
        let mut rules = self.rules.borrow_mut();
        if rules.is_empty() {
            rules.resize(SystemIx::ALL.len(), None);
        }
        rules[destination.index()]
            .get_or_insert_with(|| std::rc::Rc::new(self.reach.rules_for(self.pos.core(), self.pos.kernel, destination)))
            .clone()
    }

    #[must_use]
    pub const fn seat(&self) -> Seat {
        self.reach.seat()
    }
}

/// What one position's `Observed` keeps for the typed queries, indexed by seat and system.
#[derive(Default)]
pub struct SeatCache {
    position: Option<Option<std::rc::Rc<Position>>>,
    reach: [[Option<Option<std::rc::Rc<SeatReach>>>; 2]; 16],
    fleets: [Option<std::rc::Rc<CoreSeatFleet>>; 16],
    movable: [Option<MovableRow>; 8],
    rift_exits: std::collections::HashMap<u32, usize>,
}

impl SeatCache {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The typed position, built on first use.
    pub fn position(
        &mut self,
        state: &GameState,
        sources: SourceSet,
        galaxy: &Galaxy,
    ) -> Option<std::rc::Rc<Position>> {
        self.position.get_or_insert_with(|| Position::new(state, sources, galaxy)).clone()
    }

    /// The rules of one seat.
    pub fn reach(
        &mut self,
        pos: &std::rc::Rc<Position>,
        player: &ti4_model::id::PlayerId,
        effects: bool,
    ) -> Option<std::rc::Rc<SeatReach>> {
        let seat = pos.seat(player)?;
        self.reach[usize::from(seat.0)][usize::from(effects)]
            .get_or_insert_with(|| SeatReach::new(pos, player, effects).map(std::rc::Rc::new))
            .clone()
    }

    /// The ships of one seat.
    pub fn fleet(&mut self, pos: &Position, seat: Seat) -> std::rc::Rc<CoreSeatFleet> {
        self.fleets[usize::from(seat.0)]
            .get_or_insert_with(|| std::rc::Rc::new(CoreSeatFleet::new(pos.core(), pos.kernel, seat)))
            .clone()
    }

    /// The memoised ships that reach a system, when asked before.
    #[must_use]
    pub fn known_movable(&self, row: usize, system: SystemIx) -> Option<std::rc::Rc<[super::tactical::Movable]>> {
        self.movable.get(row)?.as_ref()?[system.index()].clone()
    }

    pub fn remember_movable(&mut self, row: usize, system: SystemIx, found: std::rc::Rc<[super::tactical::Movable]>) {
        if let Some(cell) = self.movable.get_mut(row) {
            cell.get_or_insert_with(|| vec![None; SystemIx::ALL.len()])[system.index()] = Some(found);
        }
    }

    /// The memoised rift exits of one ship type and route, when asked before.
    #[must_use]
    pub fn known_rift_exits(&self, key: u32) -> Option<usize> {
        self.rift_exits.get(&key).copied()
    }

    pub fn remember_rift_exits(&mut self, key: u32, exits: usize) {
        self.rift_exits.insert(key, exits);
    }
}

/// The key of a rift-exit query: seat, destination, origin, ship type and boosts.
#[must_use]
pub fn rift_key(row: usize, destination: SystemIx, origin: SystemIx, kind: UnitIx, boosts: (bool, bool)) -> u32 {
    ((row as u32) << 28)
        | ((destination.index() as u32) << 19)
        | ((origin.index() as u32) << 10)
        | ((kind.index() as u32) << 2)
        | u32::from(boosts.0) << 1
        | u32::from(boosts.1)
}

