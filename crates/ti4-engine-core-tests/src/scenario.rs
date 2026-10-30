//! `Scenario`: a builder of a real [`CoreState`]. Only real factions can be seated (no constructor takes a name; there is no
//! "generic"), pieces must be placed where the rules allow them, and every builder call is recorded as an [`Op`] so that
//! the property P5 can rebuild the same position in another construction order ([`Scenario::permuted`]).

use ti4_engine_core::galaxy::CoreGalaxy;
use ti4_engine_core::generated::{ActionCard, Faction, Planet, System, Technology, Unit};
use ti4_engine_core::hex::Hex;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::state::{CoreState, MapSlice, Phase, Piece, Player, Players, Seats};
use ti4_engine_core::tables::{LaneTable, SYSTEM_COUNT};

use crate::kernel::kernel_all;

/// The rules version a scenario plays by unless told otherwise (`RULES_VERSION` of `ti4-model`, 35 when this was written).
pub const LATEST_RULES: u32 = 35;

/// The 24 lobby factions, in the order of the generated enum (Thunder's Edge and the dropped factions are not in the list).
pub const LOBBY: [Faction; 24] = [
    Faction::TheArborec,
    Faction::TheArgentFlight,
    Faction::TheVuilraithCabal,
    Faction::TheEmpyrean,
    Faction::TheGhostsOfCreuss,
    Faction::TheEmiratesOfHacan,
    Faction::TheUniversitiesOfJolNar,
    Faction::TheL1Z1XMindnet,
    Faction::TheBaronyOfLetnev,
    Faction::TheMahactGeneSorcerers,
    Faction::TheMentakCoalition,
    Faction::TheEmbersOfMuaat,
    Faction::TheNaaluCollective,
    Faction::TheNaazRokhaAlliance,
    Faction::TheNekroVirus,
    Faction::TheNomad,
    Faction::TheClanOfSaar,
    Faction::SardakkNorr,
    Faction::TheFederationOfSol,
    Faction::TheTitansOfUl,
    Faction::TheWinnu,
    Faction::TheXxchaKingdom,
    Faction::TheYinBrotherhood,
    Faction::TheYssarilTribes,
];

static NO_LANES: [Option<&[(u8, u8)]>; SYSTEM_COUNT] = [None; SYSTEM_COUNT];

/// One recorded builder call.
#[derive(Debug, Clone)]
pub enum Op {
    Unit { system: System, planet: Option<Planet>, unit: Unit, seat: u8, n: usize },
    Control { planet: Planet, seat: u8 },
    Tech { seat: u8, tech: Technology },
    Goods { seat: u8, n: i16 },
    Commodities { seat: u8, n: i16 },
    Tokens { seat: u8, tactic: i16, fleet: i16, strategic: i16 },
    Hand { seat: u8, cards: Vec<ActionCard> },
    CommandToken { system: System, seat: u8 },
    Exhaust { planet: Planet },
}

impl Op {
    /// The part of the position this call writes: calls on different cells commute, calls on the same cell keep their order.
    fn cell(&self) -> String {
        match self {
            Self::Unit { system, planet, seat, .. } => format!("u:{system:?}:{planet:?}:{seat}"),
            Self::Control { planet, .. } | Self::Exhaust { planet } => format!("p:{planet:?}"),
            Self::Tech { seat, .. } => format!("t:{seat}"),
            Self::Goods { seat, .. } | Self::Commodities { seat, .. } | Self::Tokens { seat, .. } => format!("e:{seat}"),
            Self::Hand { seat, .. } => format!("h:{seat}"),
            Self::CommandToken { system, .. } => format!("c:{system:?}"),
        }
    }
}

/// A position under construction. Consume it with [`Scenario::build`].
#[derive(Clone)]
pub struct Scenario {
    factions: Vec<Faction>,
    base: CoreState,
    kernel: Kernel,
    ops: Vec<Op>,
}

impl std::fmt::Debug for Scenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scenario").field("factions", &self.factions).field("ops", &self.ops).finish()
    }
}

/// The systems [`Scenario::map_ring`] uses: plain tiles (planets, no anomaly, no hyperlane, not a home system), in id order.
#[must_use]
pub fn plain_systems() -> Vec<System> {
    System::ALL
        .iter()
        .copied()
        .filter(|s| {
            let d = s.data();
            !d.planets.is_empty() && !d.hyperlane && !d.home_system && !(d.anomalies.asteroid_field || d.anomalies.gravity_rift || d.anomalies.nebula || d.anomalies.supernova || d.anomalies.scar)
        })
        .collect()
}

/// `n` hexes of the hex spiral around the origin (origin first), the nearest first.
#[must_use]
pub fn spiral(n: usize) -> Vec<Hex> {
    let mut all = Vec::new();
    for q in -6..=6i32 {
        for r in -6..=6i32 {
            let d = q.abs().max(r.abs()).max((q + r).abs());
            if d <= 6 {
                all.push((d, q, r));
            }
        }
    }
    all.sort_unstable();
    all.into_iter().take(n).map(|(_, q, r)| Hex::new(q, r)).collect()
}

/// The system a planet is printed on.
///
/// # Panics
/// When the planet is on no tile.
#[must_use]
pub fn system_of(planet: Planet) -> System {
    planet.data().system.expect("a planet printed on a tile")
}

impl Scenario {
    /// A table of `factions.len()` seats (2 to 8), seat `i` playing `factions[i]`, rules version [`LATEST_RULES`], round 1,
    /// the real hook table ([`kernel_all`]).
    ///
    /// # Panics
    /// With fewer than 2 or more than 8 factions, a repeated faction, or a faction that is not in the lobby list or Thunder's Edge.
    #[must_use]
    pub fn new(factions: &[Faction]) -> Self {
        assert!((1..=8).contains(&factions.len()), "1 to 8 seats, got {}", factions.len());
        for (i, f) in factions.iter().enumerate() {
            assert!(!factions[..i].contains(f), "{f:?} is seated twice");
            assert!(*f != Faction::Neutral, "Neutral is not a player faction");
        }
        let mut s = CoreState::all_loaded();
        let seats: Vec<Seat> = (0..factions.len() as u8).map(Seat).collect();
        s.players = Players::from_vec(seats.iter().zip(factions).map(|(seat, f)| Player::new(*seat, *f)).collect());
        s.seats = Seats::new(&seats, &seats, factions.len() as u8);
        s.meta.rules_version = LATEST_RULES;
        s.meta.round = 1;
        s.meta.phase = Phase::Action;
        s.meta.active = Some(Seat(0));
        s.meta.speaker = Seat(0);
        s.map = MapSlice::from_galaxy(CoreGalaxy::new(&NO_LANES));
        Self { factions: factions.to_vec(), base: s, kernel: kernel_all(), ops: Vec::new() }
    }

    /// The first `n` lobby factions.
    #[must_use]
    pub fn lobby(n: usize) -> Self {
        Self::new(&LOBBY[..n])
    }

    /// Replace the kernel (use [`crate::kernel::kernel_plain`] only for a rule that must not depend on a faction).
    #[must_use]
    pub fn kernel(mut self, kernel: Kernel) -> Self {
        self.kernel = kernel;
        self
    }

    /// The rules version of the game.
    #[must_use]
    pub fn rules(mut self, version: u32) -> Self {
        self.base.meta.rules_version = version;
        self
    }

    #[must_use]
    pub fn round(mut self, round: u32) -> Self {
        self.base.meta.round = round;
        self
    }

    #[must_use]
    pub fn phase(mut self, phase: Phase) -> Self {
        self.base.meta.phase = phase;
        self
    }

    /// The active player (`None`: nobody's turn).
    #[must_use]
    pub fn active(mut self, seat: Option<u8>) -> Self {
        self.base.meta.active = seat.map(Seat);
        self
    }

    #[must_use]
    pub fn speaker(mut self, seat: u8) -> Self {
        self.base.meta.speaker = Seat(seat);
        self
    }

    #[must_use]
    pub fn active_system(mut self, system: System) -> Self {
        self.base.meta.active_system = Some(system);
        self
    }

    /// The seats, in seat order.
    #[must_use]
    pub fn seats(&self) -> Vec<Seat> {
        (0..self.factions.len() as u8).map(Seat).collect()
    }

    #[must_use]
    pub fn faction(&self, seat: u8) -> Faction {
        self.factions[usize::from(seat)]
    }

    /// A map of the first `n` plain systems on the hex spiral (the first is the centre). No hyperlanes.
    #[must_use]
    pub fn map_ring(self, n: usize) -> Self {
        let systems = plain_systems();
        assert!(n <= systems.len(), "only {} plain systems", systems.len());
        self.map_from(&systems[..n])
    }

    /// A map of exactly these systems on the hex spiral, the first at the centre.
    ///
    /// # Panics
    /// When a system is listed twice or does not fit.
    #[must_use]
    pub fn map_from(self, systems: &[System]) -> Self {
        self.map_with_lanes(systems, &NO_LANES)
    }

    /// [`Self::map_from`] with the printed lanes of hyperlane tiles.
    #[must_use]
    pub fn map_with_lanes(mut self, systems: &[System], lanes: &'static LaneTable) -> Self {
        let mut galaxy = CoreGalaxy::new(lanes);
        for (system, hex) in systems.iter().zip(spiral(systems.len())) {
            galaxy.place(*system, hex).unwrap_or_else(|e| panic!("cannot place {system:?}: {e:?}"));
        }
        galaxy.refresh_lane_links();
        self.base.map = MapSlice::from_galaxy(galaxy);
        self
    }

    /// `n` units of `unit` for `seat`. Ships and fighters go into `system`'s space (`planet` must be `None`), ground forces and structures onto
    /// a planet of that system.
    ///
    /// # Panics
    /// When the piece has no legal place (a ground force in space, a ship on a planet, a planet that is not on the tile) or the seat is not seated.
    #[must_use]
    pub fn unit(mut self, system: System, planet: Option<Planet>, unit: Unit, seat: u8, n: usize) -> Self {
        assert!(usize::from(seat) < self.factions.len(), "seat {seat} is not seated");
        let d = unit.data();
        let needs_planet = !d.space_only && (d.ground_force || d.structure);
        match planet {
            Some(p) => {
                assert!(needs_planet, "{unit:?} lives in space, not on {p:?}");
                assert!(system.data().planets.contains(&p), "{p:?} is not a planet of {system:?}");
            }
            None => assert!(!needs_planet, "{unit:?} must be placed on a planet of {system:?}"),
        }
        self.ops.push(Op::Unit { system, planet, unit, seat, n });
        self
    }

    /// `seat` controls `planet`.
    #[must_use]
    pub fn control(mut self, planet: Planet, seat: u8) -> Self {
        assert!(usize::from(seat) < self.factions.len(), "seat {seat} is not seated");
        self.ops.push(Op::Control { planet, seat });
        self
    }

    #[must_use]
    pub fn exhaust(mut self, planet: Planet) -> Self {
        self.ops.push(Op::Exhaust { planet });
        self
    }

    #[must_use]
    pub fn tech(mut self, seat: u8, tech: Technology) -> Self {
        self.ops.push(Op::Tech { seat, tech });
        self
    }

    #[must_use]
    pub fn goods(mut self, seat: u8, n: i16) -> Self {
        self.ops.push(Op::Goods { seat, n });
        self
    }

    #[must_use]
    pub fn commodities(mut self, seat: u8, n: i16) -> Self {
        self.ops.push(Op::Commodities { seat, n });
        self
    }

    #[must_use]
    pub fn tokens(mut self, seat: u8, tactic: i16, fleet: i16, strategic: i16) -> Self {
        self.ops.push(Op::Tokens { seat, tactic, fleet, strategic });
        self
    }

    #[must_use]
    pub fn hand(mut self, seat: u8, cards: &[ActionCard]) -> Self {
        self.ops.push(Op::Hand { seat, cards: cards.to_vec() });
        self
    }

    /// A command token of `seat` on `system`.
    #[must_use]
    pub fn command_token(mut self, system: System, seat: u8) -> Self {
        self.ops.push(Op::CommandToken { system, seat });
        self
    }

    /// The recorded builder calls.
    #[must_use]
    pub fn ops(&self) -> &[Op] {
        &self.ops
    }

    /// The same position built in another construction order: the recorded calls are shuffled by `seed`, but calls that write the same
    /// cell (the same planet, the same unit list, the same hand) keep their order. The result must be indistinguishable to the rules (P5).
    #[must_use]
    pub fn permuted(&self, seed: u64) -> Self {
        let mut keyed: Vec<(u64, usize)> = Vec::new();
        let mut cells: Vec<String> = Vec::new();
        for (i, op) in self.ops.iter().enumerate() {
            let c = op.cell();
            let id = cells.iter().position(|x| *x == c).unwrap_or_else(|| {
                cells.push(c);
                cells.len() - 1
            });
            keyed.push((id as u64, i));
        }
        // Order the cells by a hash of (seed, cell id); inside a cell keep the recorded order.
        let mix = |x: u64| {
            let mut z = x.wrapping_add(seed).wrapping_add(0x9E37_79B9_7F4A_7C15);
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        keyed.sort_by_key(|(cell, i)| (mix(*cell), *i));
        let mut out = self.clone();
        out.ops = keyed.into_iter().map(|(_, i)| self.ops[i].clone()).collect();
        out
    }

    /// The state and the kernel.
    #[must_use]
    pub fn build(self) -> (CoreState, Kernel) {
        let mut s = self.base;
        for op in &self.ops {
            apply(&mut s, op);
        }
        (s, self.kernel)
    }

    /// The state only.
    #[must_use]
    pub fn state(self) -> CoreState {
        self.build().0
    }
}

fn apply(s: &mut CoreState, op: &Op) {
    match op {
        Op::Unit { system, planet, unit, seat, n } => {
            for _ in 0..*n {
                let piece = Piece::new(*unit, Seat(*seat));
                match planet {
                    Some(p) => s.board.planet_mut(*p).units.push(piece),
                    None => s.board.sys_mut(*system).units.push(piece),
                }
            }
        }
        Op::Control { planet, seat } => s.board.planet_mut(*planet).control = Some(Seat(*seat)),
        Op::Exhaust { planet } => s.board.planet_mut(*planet).flags.set(ti4_engine_core::state::PlanetFlags::EXHAUSTED, true),
        Op::Tech { seat, tech } => {
            s.players.by_seat_mut(Seat(*seat)).expect("seated").tech.owned.insert(*tech);
        }
        Op::Goods { seat, n } => s.players.by_seat_mut(Seat(*seat)).expect("seated").econ.trade_goods = *n,
        Op::Commodities { seat, n } => s.players.by_seat_mut(Seat(*seat)).expect("seated").econ.commodities = *n,
        Op::Tokens { seat, tactic, fleet, strategic } => {
            let e = &mut s.players.by_seat_mut(Seat(*seat)).expect("seated").econ;
            (e.tactic_tokens, e.fleet_tokens, e.strategic_tokens) = (*tactic, *fleet, *strategic);
        }
        Op::Hand { seat, cards } => {
            let hand = &mut s.players.by_seat_mut(Seat(*seat)).expect("seated").cards.hand;
            for c in cards {
                hand.push(*c);
            }
        }
        Op::CommandToken { system, seat } => {
            s.board.sys_mut(*system).cmd.insert(Seat(*seat));
        }
    }
}

/// The first system and planet of a tile list, for tests that only need "some planet": the first plain system and its first planet.
#[must_use]
pub fn first_planet() -> (System, Planet) {
    let system = plain_systems()[0];
    (system, system.data().planets[0])
}
