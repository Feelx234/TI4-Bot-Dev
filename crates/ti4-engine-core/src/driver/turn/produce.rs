//! The end of a use of PRODUCTION by a leader's ability (`leaders_core::announce_uses`, the string adapter's tail after the child): Umbat and
//! Letani Miasmiala run one `Spawn::Produce` child per system; what the string does once each use is over is done here, for the units the
//! child reported.
//!
//! In the order of the string: a planet-value swap lapses (Hegemonic Trade Policy), Hacan's Auto-Factories pays a fleet token for three ships that
//! are no fighters, the Prophecy of Ixth is discarded unless two fighters were made, and a use that made something stages `UNITS_PRODUCED` (source
//! `ability`, no timing handle: the next reconcile announces it).

use crate::choice::production::Spot;
use crate::generated::{Agenda, Breakthrough, System, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::Made;
use crate::kernel::stage::Sx;
use crate::kernel::fx;
use crate::state::{EventKind, Key, List, Payload, Val};

/// The effects at the end of each use of `producer` (the player the use was for: Umbat's chosen player), in the order of the uses.
pub fn finish_uses(sx: &mut Sx<'_>, producer: Seat, uses: &[(System, Made)]) {
    for (system, made) in uses {
        crate::driver::windows::production::end_value_swap(sx.state);
        let ships = made.as_slice().iter().filter(|(unit, _)| unit.data().ship && unit.data().base_type != UnitBaseType::Fighter).count();
        if ships >= 3 && sx.state.players().by_seat(producer).is_some_and(|p| p.cards.breakthrough == Some(Breakthrough::AutoFactories)) {
            fx::gain_token(sx.state, producer, crate::choice::strategy::Pool::Fleet, 1);
        }
        let fighters = made.as_slice().iter().filter(|(unit, _)| unit.data().base_type == UnitBaseType::Fighter).count();
        let blanked = crate::query::reach::blanked(sx.state);
        if fighters < 2 && crate::query::laws::holds_ministry(sx.state, blanked, producer, Agenda::ProphecyOfIxth) {
            sx.state.agenda_mut().laws.remove(&Agenda::ProphecyOfIxth);
        }
        if made.as_slice().is_empty() {
            continue;
        }
        let codes = sx.table.codes.production;
        let spots: crate::containers::SmallList<(crate::generated::Unit, Spot), 12> = made.as_slice().iter().map(|(unit, planet)| (*unit, planet.map_or(Spot::Space, Spot::Planet))).collect();
        let mut payload = Payload::new()
            .with(Key::Player, Val::Seat(producer))
            .with(Key::System, Val::System(*system))
            .with(Key::Source, Val::Code(codes.ability))
            .with(Key::Count, Val::Int(i64::try_from(made.as_slice().len()).unwrap_or(i64::MAX)));
        payload.set_list(Key::Other(codes.units), List::Produced(spots));
        fx::stage(sx.state, fx::event(EventKind::UnitsProduced, payload));
    }
}
