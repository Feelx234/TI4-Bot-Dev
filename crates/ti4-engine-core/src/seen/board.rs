//! The board: `Observed::{system, controlled_planets, planet_is_ready, systems_with_token,
//! systems_with_units_of, active_system, available_spend}`.

use super::Seen;
use crate::bits::SysSet;
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::query::board::{self, Controlled, PlanetList};
use crate::query::economy::{self, Spend};
use crate::rules::production::{self as production, Cx};
use crate::state::{Piece, SysFlags};

/// One planet of a system: who controls it, what stands on it, whether its card is exhausted.
#[derive(Debug, Clone, Copy)]
pub struct PlanetView<'a> {
    pub planet: Planet,
    pub control: Option<Seat>,
    pub units: &'a [Piece],
    pub exhausted: bool,
}

/// One system's contents (`SystemState`): the space area, the command tokens, the planets in the string order.
#[derive(Clone, Copy)]
pub struct SystemView<'a> {
    seen: &'a Seen<'a>,
    system: System,
}

impl<'a> SystemView<'a> {
    #[must_use]
    pub const fn system(&self) -> System {
        self.system
    }

    /// Units in the space area, in list order.
    #[must_use]
    pub fn units(&self) -> &'a [Piece] {
        board::space_units(self.seen.st(), self.system)
    }

    /// Whether the seat's command token sits in the system.
    #[must_use]
    pub fn has_token(&self, seat: Seat) -> bool {
        board::has_token(self.seen.st(), self.system, seat)
    }

    /// The command tokens in the system.
    #[must_use]
    pub fn tokens(&self) -> crate::containers::SeatSet {
        self.seen.st().board().sys(self.system).cmd
    }

    /// The planets of the system now (printed and placed ones), in planet order.
    #[must_use]
    pub fn planets(&self) -> PlanetList {
        board::planets_in(self.seen.st(), self.seen.scope(), self.system)
    }

    /// One planet's cell.
    #[must_use]
    pub fn planet(&self, planet: Planet) -> PlanetView<'a> {
        let cell = self.seen.st().board().planet(planet);
        PlanetView {
            planet,
            control: cell.control,
            units: cell.units.as_slice(),
            exhausted: board::exhausted(self.seen.st(), planet),
        }
    }

    /// Whether the system was purged.
    #[must_use]
    pub fn purged(&self) -> bool {
        self.seen.st().board().sys(self.system).flags.has(SysFlags::PURGED)
    }
}

impl Seen<'_> {
    /// One system's contents (`Observed::system`).
    #[must_use]
    pub fn system(&self, system: System) -> SystemView<'_> {
        SystemView { seen: self, system }
    }

    /// The system activated for the tactical action in progress (`Observed::active_system`).
    #[must_use]
    pub fn active_system(&self) -> Option<System> {
        self.st().meta().active_system
    }

    /// `(system, planet)` of every planet a seat controls (`Observed::controlled_planets`).
    #[must_use]
    pub fn controlled_planets(&self, seat: Seat) -> Controlled {
        board::controlled_planets(self.st(), seat)
    }

    /// Whether a planet card is ready (`Observed::planet_is_ready`).
    #[must_use]
    pub fn planet_is_ready(&self, planet: Planet) -> bool {
        !board::exhausted(self.st(), planet)
    }

    /// Systems already holding a seat's command token (`Observed::systems_with_token`).
    #[must_use]
    pub fn systems_with_token(&self, seat: Seat) -> SysSet {
        board::systems_with_token(self.st(), seat)
    }

    /// Systems holding any of a seat's units (`Observed::systems_with_units_of`).
    #[must_use]
    pub fn systems_with_units_of(&self, seat: Seat) -> SysSet {
        board::systems_with_units_of(self.st(), seat)
    }

    /// The resources or influence a seat can spend now, counting trade goods and the faction terms
    /// (`Observed::available_spend`).
    #[must_use]
    pub fn available_spend(&self, seat: Seat, kind: Spend) -> i64 {
        let cx = Cx::new(self.st(), self.kernel(), self.scope(), self.blanked());
        let held = production::Held::of(&cx, seat);
        production::available(&cx, seat, kind, &held.extras())
    }

    /// The printed-plus-modifiers value of one planet (`production::planet_value_now` without hook terms).
    #[must_use]
    pub fn planet_value(&self, planet: Planet, kind: Spend) -> i64 {
        economy::value_now(self.st(), self.scope(), self.blanked(), planet, kind, 0)
    }
}
