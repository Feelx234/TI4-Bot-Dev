//! The encoding of one decision (docs/MCTS_BOT_PLAN.md 3.1): fixed token sets for the position, a ragged option block.
//!
//! Everything is read from [`Seen`] for the actor and is **actor-relative**: seat block 0 is the actor, then the others in the
//! seating order clockwise. Hidden information cannot reach a feature (`Seen` has no accessor for it): other seats' hands are
//! counts, the decks are not read at all.
//!
//! The layout is described once, by the code that writes it: every value goes through a [`Sink`], and [`Names`] records the
//! column names instead of the values ([`crate::schema`]). The widths are constants ([`FS`], [`FP`], [`FO`], [`FG`], [`FA`]);
//! a test checks that the code writes exactly that many columns.

use ti4_engine_core::choice::{objectives, strategy, tactical, turn, Choice, Opt};
use ti4_engine_core::generated::{Faction, Planet, PlanetTrait, System, Technology, TechType, UnitBaseType};
use ti4_engine_core::query::objectives::Imagined;
use ti4_engine_core::seen::Seen;
use ti4_engine_core::state::{ObjectiveRef, Phase, Piece};
use ti4_engine_core::Seat;

use crate::filter;
use crate::kind::{self, TreeKind};

/// Bump on any change of the layout or the meaning of a column.
pub const FEATURE_VERSION: u32 = 1;
/// System slots (the map's placed systems in hex order; Learn to Play 4p has 37).
pub const MAX_SYS: usize = 40;
/// Seats (the first MCTS setup is 4 players).
pub const SEATS: usize = 4;
/// Objective slots: the revealed objectives in reveal order, then the actor's secrets in hand.
pub const MAX_OBJ: usize = 16;
/// Pointers per option: system slot, objective slot, entity id.
pub const PTRS: usize = 3;
/// Bytes per row of the packed adjacency.
pub const ADJ_BYTES: usize = MAX_SYS.div_ceil(8);
/// Hash buckets of the ask-kind and option-kind ids.
pub const KIND_BUCKETS: u64 = 512;

/// The four factions of the first setup, in the one-hot order (anything else is "other").
pub const FACTIONS: [Faction; 4] = [Faction::TheFederationOfSol, Faction::TheBaronyOfLetnev, Faction::TheXxchaKingdom, Faction::TheUniversitiesOfJolNar];

const BASES: [UnitBaseType; 11] = UnitBaseType::ALL;
const GROUND: [UnitBaseType; 4] = [UnitBaseType::Infantry, UnitBaseType::Mech, UnitBaseType::Pds, UnitBaseType::Spacedock];
const TECH_TYPES: [TechType; 5] = [TechType::Biotic, TechType::Cybernetic, TechType::Propulsion, TechType::Warfare, TechType::Unitupgrade];

/// Columns per system token.
pub const FS: usize = 1 + SEATS * 17 + 22 + 12 + SEATS + 1 + 3 + 1;
/// Columns per seat token.
pub const FP: usize = 5 + 12 + 16 + 5 + Technology::COUNT + 3 + 11 + 2;
/// Columns per objective token.
pub const FO: usize = 6 + SEATS + 2 * SEATS;
/// Global columns.
pub const FG: usize = 1 + 4 + 1 + 1 + (TreeKind::ALL.len() + 1) + 1 + 1;
/// Columns per option.
pub const FA: usize = 2 + 4 + 1 + 17 + 12 + 2 + 2;

/// Where the values go: the numbers ([`Vals`]) or the column names ([`Names`]).
pub trait Sink {
    fn put(&mut self, v: f32, name: impl Fn() -> String);
}

/// Appends the values.
pub struct Vals<'a>(pub &'a mut Vec<f32>);

impl Sink for Vals<'_> {
    #[inline]
    fn put(&mut self, v: f32, _: impl Fn() -> String) {
        self.0.push(v);
    }
}

/// Records the column names.
#[derive(Default)]
pub struct Names(pub Vec<String>);

impl Sink for Names {
    fn put(&mut self, _: f32, name: impl Fn() -> String) {
        self.0.push(name());
    }
}

fn b(x: bool) -> f32 {
    if x { 1.0 } else { 0.0 }
}

/// One encoded decision.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Encoded {
    /// `[MAX_SYS, FS]`.
    pub sys: Vec<f32>,
    /// `[MAX_SYS]`: the system index, -1 for an empty slot.
    pub sys_id: Vec<i16>,
    /// `[MAX_SYS, ADJ_BYTES]`: row `i` bit `j` (little-endian bits) set when slot `j` is adjacent to slot `i`.
    pub adj: Vec<u8>,
    /// `[SEATS, FP]`, actor first.
    pub seat: Vec<f32>,
    /// `[SEATS]`: the faction index (`Faction as usize`), -1 for no seat.
    pub seat_fac: Vec<i16>,
    /// `[MAX_OBJ, FO]`.
    pub obj: Vec<f32>,
    /// `[MAX_OBJ]`: public objective index, secret index + 64, -1 for an empty slot.
    pub obj_id: Vec<i16>,
    /// `[FG]`.
    pub glob: Vec<f32>,
    /// The ask kind id (hash bucket of `kind::ask_name`).
    pub ask_kind: i16,
    /// `[K, FA]`.
    pub opt: Vec<f32>,
    /// `[K]`: option kind id (hash bucket of the option's variant name).
    pub opt_kind: Vec<i16>,
    /// `[K, PTRS]`: system slot, objective slot, entity id; -1 for none.
    pub opt_ptr: Vec<i16>,
    /// `[K]`: the option survives the hard filter (all true outside the activations).
    pub mask: Vec<bool>,
}

impl Encoded {
    /// Options.
    #[must_use]
    pub fn k(&self) -> usize {
        self.mask.len()
    }
}

/// The actor-relative seat order: the actor, then clockwise.
#[must_use]
pub fn relative_seats(seen: &Seen<'_>, actor: Seat) -> Vec<Seat> {
    let order = seen.seating_order();
    let order = order.as_slice();
    let at = order.iter().position(|s| *s == actor).unwrap_or(0);
    (0..order.len()).map(|i| order[(at + i) % order.len()]).collect()
}

/// The hash bucket of a name (ask kinds, option kinds).
#[must_use]
pub fn bucket(name: &str) -> i16 {
    i16::try_from(kind::fnv(name) % KIND_BUCKETS).unwrap_or(0)
}

/// `Area.Variant` of an option (as [`kind::ask_name`] for asks).
#[must_use]
pub fn opt_name(o: &Opt) -> String {
    let text = format!("{o:?}");
    let (area, rest) = text.split_once('(').unwrap_or((text.as_str(), ""));
    let variant: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
    if variant.is_empty() { area.to_owned() } else { format!("{area}.{variant}") }
}

/// The context of one encoding: the map slots, the seats, the homes, the actor's reach.
pub struct Frame {
    pub actor: Seat,
    pub seats: Vec<Seat>,
    pub systems: Vec<System>,
    /// Per seat block: the home system's slot.
    pub homes: Vec<Option<usize>>,
    /// Hop distances `[slot][slot]` (u8::MAX unreachable).
    pub dist: Vec<Vec<u8>>,
    /// Neighbour slots per slot.
    pub adj: Vec<Vec<usize>>,
    /// The actor's producers, cannons and the map's adjacency (the filter's board).
    pub board: filter::Board,
    /// Per slot: how many of the actor's ships can move into the system (`Seen::movable_into(actor, system).len()`).
    pub moves: Vec<u16>,
    /// The slot of each system index (-1: not on the map).
    slot_of: Vec<i16>,
}

/// What a [`Frame`] keeps of the map between the decisions of one table: valid while the map's systems and their adjacency
/// are the same (checked on every use).
#[derive(Default, Clone)]
pub struct MapCache {
    systems: Vec<System>,
    adjacent: Vec<ti4_engine_core::SysSet>,
    adj: Vec<Vec<usize>>,
    dist: Vec<Vec<u8>>,
    slot_of: Vec<i16>,
    /// The home slot per faction asked so far.
    homes: Vec<(Faction, Option<usize>)>,
}

impl MapCache {
    fn build(systems: Vec<System>, adjacent: Vec<ti4_engine_core::SysSet>) -> Self {
        let mut slots = systems.clone();
        slots.truncate(MAX_SYS);
        let top = slots.iter().map(|s| s.index() + 1).max().unwrap_or(0);
        let mut slot_of = vec![-1i16; top];
        for (i, s) in slots.iter().enumerate() {
            slot_of[s.index()] = i as i16;
        }
        let slot = |s: System| slot_of.get(s.index()).copied().filter(|x| *x >= 0).map(|x| x as usize);
        let n = slots.len();
        let adj: Vec<Vec<usize>> = (0..n).map(|i| adjacent[i].iter().filter_map(System::from_index).filter_map(slot).collect()).collect();
        let dist = distances(&adj);
        Self { systems, adjacent, adj, dist, slot_of, homes: Vec::new() }
    }

    fn home(&mut self, faction: Faction) -> Option<usize> {
        if let Some((_, h)) = self.homes.iter().find(|(f, _)| *f == faction) {
            return *h;
        }
        let h = self.systems.iter().take(MAX_SYS).position(|s| is_home_of(*s, Some(faction)));
        self.homes.push((faction, h));
        h
    }
}

fn is_home_of(s: System, faction: Option<Faction>) -> bool {
    s.data().planets.iter().any(|p| p.data().homeworld_of.is_some_and(|t| faction.is_some_and(|f| t.factions().contains(&f))))
}

fn distances(adj: &[Vec<usize>]) -> Vec<Vec<u8>> {
    let n = adj.len();
    let mut dist = vec![vec![u8::MAX; n]; n];
    for (from, row) in dist.iter_mut().enumerate() {
        row[from] = 0;
        let mut frontier = vec![from];
        let mut d = 0u8;
        while !frontier.is_empty() && d < 16 {
            d += 1;
            let mut next = Vec::new();
            for &i in &frontier {
                for &j in &adj[i] {
                    if row[j] == u8::MAX {
                        row[j] = d;
                        next.push(j);
                    }
                }
            }
            frontier = next;
        }
    }
    dist
}

impl Frame {
    /// The reference frame: everything computed from `seen`, the reach by `movable_into` per system.
    #[must_use]
    pub fn new(seen: &Seen<'_>, actor: Seat) -> Self {
        let systems = seen.map_systems();
        let adjacent = seen.adjacent_of_each(&systems);
        let mut cache = MapCache::build(systems, adjacent);
        let mut f = Self::assemble(seen, actor, &mut cache);
        f.moves = f.systems.iter().map(|s| u16::try_from(seen.movable_into(actor, *s).len()).unwrap_or(u16::MAX)).collect();
        f
    }

    /// The same frame from a cache of the map, the reach counted for all systems at once (`Seen::movable_counts`).
    #[must_use]
    pub fn cached(seen: &Seen<'_>, actor: Seat, cache: &mut MapCache) -> Self {
        let systems = seen.map_systems();
        let adjacent = seen.adjacent_of_each(&systems);
        if cache.systems != systems || cache.adjacent != adjacent {
            *cache = MapCache::build(systems, adjacent);
        }
        let mut f = Self::assemble(seen, actor, cache);
        f.moves = seen.movable_counts(actor, &f.systems);
        f
    }

    fn assemble(seen: &Seen<'_>, actor: Seat, cache: &mut MapCache) -> Self {
        let seats = relative_seats(seen, actor);
        let board = filter::Board::of_map(seen, actor, cache.systems.clone(), cache.adjacent.clone());
        let mut systems = cache.systems.clone();
        systems.truncate(MAX_SYS);
        let homes = seats.iter().map(|seat| seen.seat(*seat).map(|v| v.faction).and_then(|f| cache.home(f))).collect();
        Self { actor, seats, systems, homes, dist: cache.dist.clone(), adj: cache.adj.clone(), board, moves: Vec::new(), slot_of: cache.slot_of.clone() }
    }

    /// The block of a seat (0 = the actor); `None` for a seat not at the table.
    #[must_use]
    pub fn rel(&self, seat: Seat) -> Option<usize> {
        self.seats.iter().position(|s| *s == seat).filter(|r| *r < SEATS)
    }

    #[must_use]
    pub fn slot(&self, system: System) -> Option<usize> {
        self.slot_of.get(system.index()).copied().filter(|x| *x >= 0).map(|x| x as usize)
    }

    /// Whether some ship of the actor can move into `system` (0 off the map).
    #[must_use]
    pub fn movable(&self, system: System) -> bool {
        self.slot(system).and_then(|i| self.moves.get(i)).is_some_and(|n| *n > 0)
    }
}

fn mecatol(system: System) -> bool {
    system.data().planets.contains(&Planet::MecatolRex)
}

/// Write the system token of `system` (or an empty slot).
fn system_block(s: &mut impl Sink, seen: &Seen<'_>, f: &Frame, slot: usize, system: Option<System>, onboard: &mut [[u32; 11]; SEATS]) {
    let Some(system) = system else {
        for _ in 0..FS {
            s.put(0.0, &|| "empty".into());
        }
        return;
    };
    s.put(1.0, &|| "present".into());
    let view = seen.system(system);
    let planets = view.planets();
    // Units per seat block.
    let mut space = [[0u32; 11]; SEATS];
    let mut ground = [[0u32; 4]; SEATS];
    let mut damaged = [0u32; SEATS];
    let mut count = |p: &Piece, on_planet: bool, space: &mut [[u32; 11]; SEATS], ground: &mut [[u32; 4]; SEATS]| {
        if let Some(r) = f.rel(p.owner()) {
            let base = p.kind().data().base_type as usize;
            onboard[r][base] += 1;
            if on_planet {
                if let Some(g) = GROUND.iter().position(|x| *x as usize == base) {
                    ground[r][g] += 1;
                }
            } else {
                space[r][base] += 1;
            }
            damaged[r] += u32::from(p.sustained());
        }
    };
    for p in view.units() {
        count(p, false, &mut space, &mut ground);
    }
    for planet in planets.as_slice() {
        for p in view.planet(*planet).units {
            count(p, true, &mut space, &mut ground);
        }
    }
    for r in 0..SEATS {
        for (i, base) in BASES.iter().enumerate() {
            s.put(space[r][i] as f32 / 4.0, &|| format!("p{r}_space_{base:?}"));
        }
        for (i, base) in GROUND.iter().enumerate() {
            s.put(ground[r][i] as f32 / 4.0, &|| format!("p{r}_ground_{base:?}"));
        }
        s.put(damaged[r] as f32 / 2.0, &|| format!("p{r}_damaged"));
        s.put(b(f.seats.get(r).is_some_and(|seat| view.has_token(*seat))), &|| format!("p{r}_token"));
    }
    // Planets.
    let (mut res, mut inf) = (0u32, 0u32);
    let mut ctl = [[0u32; 3]; SEATS];
    let (mut free, mut my_exhausted) = (0u32, 0u32);
    let mut traits = [0u32; 3];
    let (mut skip, mut legendary) = (false, false);
    for planet in planets.as_slice() {
        let d = planet.data();
        let pv = view.planet(*planet);
        res += u32::from(d.resources);
        inf += u32::from(d.influence);
        match pv.control.and_then(|c| f.rel(c)) {
            Some(r) => {
                ctl[r][0] += 1;
                ctl[r][1] += u32::from(d.resources);
                ctl[r][2] += u32::from(d.influence);
                if r == 0 && pv.exhausted {
                    my_exhausted += 1;
                }
            }
            None => free += u32::from(pv.control.is_none()),
        }
        for (i, t) in [PlanetTrait::Cultural, PlanetTrait::Hazardous, PlanetTrait::Industrial].iter().enumerate() {
            traits[i] += u32::from(d.traits.contains(t));
        }
        skip |= !d.specialties.is_empty();
        legendary |= d.legendary;
    }
    s.put(planets.len() as f32 / 3.0, &|| "planets".into());
    s.put(res as f32 / 6.0, &|| "resources".into());
    s.put(inf as f32 / 6.0, &|| "influence".into());
    for (r, c) in ctl.iter().enumerate() {
        s.put(c[0] as f32 / 3.0, &|| format!("p{r}_planets"));
        s.put(c[1] as f32 / 6.0, &|| format!("p{r}_resources"));
        s.put(c[2] as f32 / 6.0, &|| format!("p{r}_influence"));
    }
    s.put(free as f32 / 3.0, &|| "uncontrolled".into());
    s.put(my_exhausted as f32 / 3.0, &|| "my_exhausted".into());
    for (i, t) in ["cultural", "hazardous", "industrial"].iter().enumerate() {
        s.put(traits[i] as f32 / 3.0, &|| format!("trait_{t}"));
    }
    s.put(b(skip), &|| "tech_skip".into());
    s.put(b(legendary), &|| "legendary".into());
    // The tile.
    let data = system.data();
    s.put(data.wormholes.len() as f32 / 2.0, &|| "wormholes".into());
    s.put(b(data.anomalies.asteroid_field), &|| "asteroid_field".into());
    s.put(b(data.anomalies.gravity_rift), &|| "gravity_rift".into());
    s.put(b(data.anomalies.nebula), &|| "nebula".into());
    s.put(b(data.anomalies.supernova), &|| "supernova".into());
    s.put(b(data.hyperlane), &|| "hyperlane".into());
    let is_mecatol = mecatol(system);
    s.put(b(is_mecatol), &|| "mecatol".into());
    s.put(b(is_mecatol && !seen.custodians_removed()), &|| "custodians".into());
    for r in 0..SEATS {
        s.put(b(f.homes.get(r).copied().flatten() == Some(slot)), &|| format!("home_of_p{r}"));
    }
    for r in 0..SEATS {
        let d = f.homes.get(r).copied().flatten().and_then(|h| f.dist.get(h).and_then(|row| row.get(slot)).copied()).unwrap_or(u8::MAX);
        s.put(if d == u8::MAX { 1.0 } else { f32::from(d.min(8)) / 8.0 }, &|| format!("dist_home_p{r}"));
    }
    // The actor's reach and the filter's reasons.
    let movable = f.moves.get(slot).copied().unwrap_or(0);
    s.put(f32::from(movable) / 8.0, &|| "my_movable".into());
    let why = f.board.reasons_moving(system, movable > 0);
    s.put(b(why & filter::MOVE != 0), &|| "can_move_in".into());
    s.put(b(why & filter::PRODUCE != 0), &|| "can_produce".into());
    s.put(b(why & filter::SHOOT != 0), &|| "can_shoot".into());
    s.put(b(seen.active_system() == Some(system)), &|| "active".into());
}

fn seat_block(s: &mut impl Sink, seen: &Seen<'_>, f: &Frame, r: usize, onboard: &[u32; 11]) {
    let Some(v) = f.seats.get(r).and_then(|seat| seen.seat(*seat)) else {
        for _ in 0..FP {
            s.put(0.0, &|| format!("empty_p{r}"));
        }
        return;
    };
    let other = !FACTIONS.contains(&v.faction);
    for fac in FACTIONS {
        s.put(b(v.faction == fac), &|| format!("faction_{fac:?}"));
    }
    s.put(b(other), &|| "faction_other".into());
    s.put(v.victory_points as f32 / 10.0, &|| "vp".into());
    s.put(v.trade_goods as f32 / 10.0, &|| "trade_goods".into());
    s.put(v.commodities as f32 / 4.0, &|| "commodities".into());
    s.put(v.tactic_tokens as f32 / 8.0, &|| "tactic_tokens".into());
    s.put(v.fleet_tokens as f32 / 8.0, &|| "fleet_tokens".into());
    s.put(v.strategic_tokens as f32 / 8.0, &|| "strategy_tokens".into());
    s.put(v.action_cards_held as f32 / 7.0, &|| "action_cards_held".into());
    s.put(v.secret_objectives_held as f32 / 3.0, &|| "secrets_held".into());
    s.put(b(v.passed), &|| "passed".into());
    s.put(b(seen.speaker() == v.seat), &|| "speaker".into());
    s.put(b(seen.active() == Some(v.seat)), &|| "active".into());
    let order = seen.initiative_order();
    let rank = order.as_slice().iter().position(|x| *x == v.seat);
    s.put(rank.map_or(1.0, |i| i as f32 / 8.0), &|| "initiative_rank".into());
    for i in 1..=8u8 {
        let held = v.strategy_cards.iter().any(|c| c.data().initiative == i);
        s.put(b(held), &|| format!("holds_card_{i}"));
    }
    for i in 1..=8u8 {
        let used = v.strategy_cards.iter().any(|c| c.data().initiative == i && v.exhausted_strategy_cards.contains(*c));
        s.put(b(used), &|| format!("used_card_{i}"));
    }
    for t in TECH_TYPES {
        let n = v.technologies.iter().filter(|x| x.data().types.contains(&t)).count();
        s.put(n as f32 / 6.0, &|| format!("techs_{t:?}"));
    }
    for t in Technology::ALL {
        s.put(b(v.technologies.contains(t)), &|| format!("tech_{t:?}"));
    }
    let planets = seen.controlled_planets(v.seat);
    let (mut res, mut inf) = (0u32, 0u32);
    for (_, p) in planets.as_slice() {
        res += u32::from(p.data().resources);
        inf += u32::from(p.data().influence);
    }
    s.put(planets.len() as f32 / 10.0, &|| "planets".into());
    s.put(res as f32 / 20.0, &|| "resources".into());
    s.put(inf as f32 / 20.0, &|| "influence".into());
    for (i, base) in BASES.iter().enumerate() {
        s.put(onboard[i] as f32 / 10.0, &|| format!("on_board_{base:?}"));
    }
    s.put(v.relics.len() as f32 / 2.0, &|| "relics".into());
    s.put(v.exploration_cards.len() as f32 / 3.0, &|| "exploration_cards".into());
}

/// One objective slot: `(objective, mine-secret)`.
fn objective_block(s: &mut impl Sink, seen: &Seen<'_>, f: &Frame, obj: Option<ObjectiveRef>, progress: &[Vec<(ObjectiveRef, f32, bool)>]) {
    let Some(o) = obj else {
        for _ in 0..FO {
            s.put(0.0, &|| "empty".into());
        }
        return;
    };
    let (public, stage, vp) = match o {
        ObjectiveRef::Public(p) => (true, p.data().points, p.data().points),
        ObjectiveRef::Secret(_) => (false, 0, 1),
    };
    s.put(1.0, &|| "present".into());
    s.put(b(public), &|| "public".into());
    s.put(b(!public), &|| "secret".into());
    s.put(b(stage == 1), &|| "stage_1".into());
    s.put(b(stage == 2), &|| "stage_2".into());
    s.put(f32::from(vp) / 3.0, &|| "vp".into());
    for r in 0..SEATS {
        let scored = f.seats.get(r).is_some_and(|seat| {
            let set = seen.scored_by(*seat);
            match o {
                ObjectiveRef::Public(p) => set.public.contains(p),
                ObjectiveRef::Secret(x) => set.secret.contains(x),
            }
        });
        s.put(b(scored), &|| format!("scored_p{r}"));
    }
    for r in 0..SEATS {
        let row = progress.get(r).and_then(|rows| rows.iter().find(|(x, _, _)| *x == o));
        s.put(row.map_or(0.0, |x| x.1), &|| format!("progress_p{r}"));
        s.put(b(row.is_some_and(|x| x.2)), &|| format!("satisfied_p{r}"));
    }
}

fn global_block(s: &mut impl Sink, seen: &Seen<'_>, choice: &Choice) {
    s.put(seen.round() as f32 / 10.0, &|| "round".into());
    for ph in [Phase::Strategy, Phase::Action, Phase::Status, Phase::Agenda] {
        s.put(b(seen.phase() == ph), &|| format!("phase_{ph:?}"));
    }
    s.put(b(seen.custodians_removed()), &|| "custodians_removed".into());
    s.put(seen.revealed_objectives().len() as f32 / 10.0, &|| "revealed_objectives".into());
    let tk = kind::tree_kind_of(choice);
    for k in TreeKind::ALL {
        s.put(b(tk == Some(k)), &|| format!("ask_{}", k.name()));
    }
    s.put(b(tk.is_none()), &|| "ask_delegated".into());
    s.put((choice.opts.len() as f32).ln_1p() / 4.0, &|| "log_options".into());
    s.put(b(seen.active() == Some(choice.actor)), &|| "actor_is_active".into());
}

fn option_block(s: &mut impl Sink, seen: &Seen<'_>, f: &Frame, o: &Opt, keep: bool, why: u8, ptr: &mut [i16; PTRS], objs: &[ObjectiveRef]) {
    s.put(b(o.is_decline()), &|| "decline".into());
    s.put(b(matches!(o, Opt::Done)), &|| "done".into());
    s.put(b(keep), &|| "kept".into());
    s.put(b(why & filter::MOVE != 0), &|| "why_move".into());
    s.put(b(why & filter::PRODUCE != 0), &|| "why_produce".into());
    s.put(b(why & filter::SHOOT != 0), &|| "why_shoot".into());
    // The system the option is about.
    let system = match o {
        Opt::Tactical(tactical::Opt::Move { origin, .. }) => Some(*origin),
        _ => filter::system_of(o),
    };
    let slot = system.and_then(|x| f.slot(x));
    ptr[0] = slot.map_or(-1, |x| x as i16);
    s.put(b(slot.is_some()), &|| "has_system".into());
    // Movement.
    let mv = match o {
        Opt::Tactical(tactical::Opt::Move { kind, damaged, capacity, gd, ionian, route, .. }) => Some((*kind, *damaged, *capacity, *gd, *ionian, *route)),
        _ => None,
    };
    s.put(mv.map_or(0.0, |m| f32::from(m.2.max(0)) / 6.0), &|| "move_capacity".into());
    s.put(b(mv.is_some_and(|m| m.1)), &|| "move_damaged".into());
    s.put(b(mv.is_some_and(|m| m.3)), &|| "move_gravity_drive".into());
    s.put(b(mv.is_some_and(|m| m.4)), &|| "move_ionian".into());
    s.put(b(mv.is_some_and(|m| m.5 > 0)), &|| "move_route".into());
    for base in BASES {
        s.put(b(mv.is_some_and(|m| m.0.data().base_type == base)), &|| format!("move_{base:?}"));
    }
    s.put(mv.map_or(0.0, |m| f32::from(m.0.data().movement.unwrap_or(0)) / 4.0), &|| "move_value".into());
    if let Some(m) = mv {
        ptr[2] = m.0.index() as i16;
    }
    // The action menu and the closing menu.
    let t = match o {
        Opt::Turn(t) => Some(*t),
        _ => None,
    };
    let is = |p: fn(&turn::Opt) -> bool| b(t.as_ref().is_some_and(p));
    s.put(is(|t| matches!(t, turn::Opt::Pass)), &|| "turn_pass".into());
    s.put(is(|t| matches!(t, turn::Opt::Tactical)), &|| "turn_tactical".into());
    s.put(is(|t| matches!(t, turn::Opt::Strategic { .. })), &|| "turn_strategic".into());
    s.put(is(|t| matches!(t, turn::Opt::EndTurn)), &|| "turn_end".into());
    s.put(is(|t| matches!(t, turn::Opt::Leader(_) | turn::Opt::Borrowed(_))), &|| "turn_leader".into());
    s.put(is(|t| matches!(t, turn::Opt::Tech(_))), &|| "turn_tech".into());
    s.put(is(|t| matches!(t, turn::Opt::UseRelic(_) | turn::Opt::Purge(_) | turn::Opt::Device(_))), &|| "turn_relic".into());
    s.put(is(|t| matches!(t, turn::Opt::Play { .. } | turn::Opt::Salvaged(_))), &|| "turn_action_card".into());
    s.put(is(|t| matches!(t, turn::Opt::Open { .. })), &|| "turn_trade".into());
    s.put(is(|t| matches!(t, turn::Opt::Faction(_))), &|| "turn_faction".into());
    s.put(is(|t| matches!(t, turn::Opt::Lent(_) | turn::Opt::Token(_))), &|| "turn_other".into());
    let card = match o {
        Opt::Turn(turn::Opt::Strategic { card, .. }) | Opt::Strategy(strategy::Opt::Card(card)) => Some(*card),
        _ => None,
    };
    s.put(card.map_or(0.0, |c| f32::from(c.data().initiative) / 8.0), &|| "card_initiative".into());
    if let Some(c) = card {
        ptr[2] = c.index() as i16;
    }
    // The draft.
    let drafted = matches!(o, Opt::Strategy(strategy::Opt::Card(_)));
    s.put(b(drafted), &|| "draft_card".into());
    s.put(card.filter(|_| drafted).map_or(0.0, |c| seen.strategy_card_goods(c) as f32 / 3.0), &|| "draft_card_goods".into());
    // Scoring.
    let scored = match o {
        Opt::Objectives(objectives::Opt::Score(r)) => Some(*r),
        Opt::Strategy(strategy::Opt::Objective(p)) => Some(ObjectiveRef::Public(*p)),
        _ => None,
    };
    if let Some(r) = scored {
        ptr[1] = objs.iter().position(|x| *x == r).map_or(-1, |i| i as i16);
    }
    s.put(b(scored.is_some()), &|| "score".into());
    s.put(scored.map_or(0.0, |r| match r {
        ObjectiveRef::Public(p) => f32::from(p.data().points) / 3.0,
        ObjectiveRef::Secret(_) => 1.0 / 3.0,
    }), &|| "score_vp".into());
}

pub fn progress_rows(seen: &Seen<'_>, f: &Frame) -> Vec<Vec<(ObjectiveRef, f32, bool)>> {
    let ratio = |have: i64, threshold: i64| if threshold <= 0 { 1.0 } else { (have as f32 / threshold as f32).clamp(0.0, 1.0) };
    (0..SEATS)
        .map(|r| {
            let Some(seat) = f.seats.get(r) else { return Vec::new() };
            let mut rows: Vec<(ObjectiveRef, f32, bool)> = seen.revealed_objective_progress(*seat, Imagined::NONE).unwrap_or_default().into_iter().map(|p| (p.objective, ratio(p.have, p.threshold), p.satisfied)).collect();
            if r == 0 {
                rows.extend(seen.my_secret_progress(Imagined::NONE).unwrap_or_default().into_iter().map(|p| (p.objective, ratio(p.have, p.threshold), p.satisfied)));
            }
            rows
        })
        .collect()
}

/// The objective slots: the revealed ones in reveal order, then the actor's secrets in hand.
#[must_use]
pub fn objective_slots(seen: &Seen<'_>) -> Vec<ObjectiveRef> {
    let mut out: Vec<ObjectiveRef> = seen.revealed_objectives().to_vec();
    for s in seen.my_secrets() {
        let r = ObjectiveRef::Secret(*s);
        if !out.contains(&r) {
            out.push(r);
        }
    }
    out.truncate(MAX_OBJ);
    out
}

/// An encoder that keeps what does not change between the decisions of one table (lane L4's encode speed-up). Gives exactly
/// the output of [`encode`].
#[derive(Default)]
pub struct Encoder {
    map: MapCache,
}

impl Encoder {
    /// [`encode`].
    #[must_use]
    pub fn encode(&mut self, seen: &Seen<'_>, choice: &Choice) -> Encoded {
        let mut e = Encoded::default();
        self.encode_into(seen, choice, &mut e);
        e
    }

    /// [`encode_into`].
    pub fn encode_into(&mut self, seen: &Seen<'_>, choice: &Choice, e: &mut Encoded) {
        let f = Frame::cached(seen, choice.actor, &mut self.map);
        encode_frame(seen, choice, &f, e);
    }
}

/// Encode `choice` for its actor. `seen` must be bound to `choice.actor`.
#[must_use]
pub fn encode(seen: &Seen<'_>, choice: &Choice) -> Encoded {
    let mut e = Encoded::default();
    encode_into(seen, choice, &mut e);
    e
}

/// [`encode`] into reused buffers.
pub fn encode_into(seen: &Seen<'_>, choice: &Choice, e: &mut Encoded) {
    let f = Frame::new(seen, choice.actor);
    encode_frame(seen, choice, &f, e);
}

/// The encoding on a frame already built.
fn encode_frame(seen: &Seen<'_>, choice: &Choice, f: &Frame, e: &mut Encoded) {
    debug_assert_eq!(seen.me(), choice.actor, "the view must be the actor's");
    e.sys.clear();
    e.sys_id.clear();
    e.adj.clear();
    let mut onboard = [[0u32; 11]; SEATS];
    for slot in 0..MAX_SYS {
        let system = f.systems.get(slot).copied();
        system_block(&mut Vals(&mut e.sys), seen, &f, slot, system, &mut onboard);
        e.sys_id.push(system.map_or(-1, |x| x.index() as i16));
        let mut row = [0u8; ADJ_BYTES];
        for &j in f.adj.get(slot).map_or(&[][..], Vec::as_slice) {
            row[j / 8] |= 1 << (j % 8);
        }
        e.adj.extend_from_slice(&row);
    }
    e.seat.clear();
    e.seat_fac.clear();
    for (r, board) in onboard.iter().enumerate() {
        seat_block(&mut Vals(&mut e.seat), seen, &f, r, board);
        e.seat_fac.push(f.seats.get(r).and_then(|x| seen.seat(*x)).map_or(-1, |v| v.faction as i16));
    }
    let objs = objective_slots(seen);
    let progress = progress_rows(seen, &f);
    e.obj.clear();
    e.obj_id.clear();
    for slot in 0..MAX_OBJ {
        let o = objs.get(slot).copied();
        objective_block(&mut Vals(&mut e.obj), seen, &f, o, &progress);
        e.obj_id.push(o.map_or(-1, |o| match o {
            ObjectiveRef::Public(p) => p.index() as i16,
            ObjectiveRef::Secret(s) => s.index() as i16 + 64,
        }));
    }
    e.glob.clear();
    global_block(&mut Vals(&mut e.glob), seen, choice);
    e.ask_kind = bucket(&kind::ask_name(&choice.ask));
    let kept = filter::is_activation(&choice.ask).then(|| filter::activate_with(choice, &f.board, |system| f.movable(system)));
    e.opt.clear();
    e.opt_kind.clear();
    e.opt_ptr.clear();
    e.mask.clear();
    for (i, o) in choice.opts.iter().enumerate() {
        let keep = kept.as_ref().is_none_or(|k| k.keep[i]);
        let why = kept.as_ref().map_or(0, |k| k.why[i]);
        let mut ptr = [-1i16; PTRS];
        option_block(&mut Vals(&mut e.opt), seen, &f, o, keep, why, &mut ptr, &objs);
        e.opt_kind.push(bucket(&opt_name(o)));
        e.opt_ptr.extend_from_slice(&ptr);
        e.mask.push(keep);
    }
}

/// The column names of the five blocks, from an encoding of a real decision: `(system, seat, objective, global, option)`.
#[must_use]
pub fn column_names(seen: &Seen<'_>, choice: &Choice) -> [Vec<String>; 5] {
    let f = Frame::new(seen, choice.actor);
    let mut sys = Names::default();
    system_block(&mut sys, seen, &f, 0, f.systems.first().copied(), &mut [[0; 11]; SEATS]);
    let mut seat = Names::default();
    seat_block(&mut seat, seen, &f, 0, &[0; 11]);
    let mut obj = Names::default();
    // A public objective that is surely present: the names do not depend on the card.
    objective_block(&mut obj, seen, &f, Some(ObjectiveRef::Public(ti4_engine_core::generated::PublicObjective::ALL[0])), &[]);
    let mut glob = Names::default();
    global_block(&mut glob, seen, choice);
    let mut opt = Names::default();
    let mut ptr = [-1i16; PTRS];
    option_block(&mut opt, seen, &f, &Opt::Done, true, 0, &mut ptr, &[]);
    [sys.0, seat.0, obj.0, glob.0, opt.0]
}
