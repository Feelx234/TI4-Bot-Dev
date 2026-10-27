//! The map slice: the existing `CoreGalaxy` (placement, wormholes, lanes, adjacency) behind one
//! `Cow`. `None` until the adapter loads it (a `CoreGalaxy` needs the static lane table).

use crate::containers::Cow;
use crate::galaxy::CoreGalaxy;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MapSlice {
    pub galaxy: Option<Cow<CoreGalaxy>>,
}
