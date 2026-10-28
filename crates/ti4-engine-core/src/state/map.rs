//! The map slice: the existing `CoreGalaxy` (placement, wormholes, lanes, adjacency) behind one
//! `Cow`, and the adjacency computed from it once. `None` until the adapter loads it (a `CoreGalaxy`
//! needs the static lane table).

use std::sync::Arc;

use crate::containers::Cow;
use crate::galaxy::{Adjacency, CoreGalaxy};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MapSlice {
    pub galaxy: Option<Cow<CoreGalaxy>>,
    /// `galaxy.adjacency()`, shared by every rule set built on this map (no recomputation, no map
    /// comparison per query).
    adjacency: Option<Arc<Adjacency>>,
}

impl MapSlice {
    /// A map slice with this galaxy and its adjacency.
    #[must_use]
    pub fn of(galaxy: Cow<CoreGalaxy>, adjacency: Arc<Adjacency>) -> Self {
        Self { galaxy: Some(galaxy), adjacency: Some(adjacency) }
    }

    /// A map slice from a galaxy alone (the adjacency is computed now).
    #[must_use]
    pub fn from_galaxy(galaxy: CoreGalaxy) -> Self {
        let adjacency = Arc::new(galaxy.adjacency());
        Self { galaxy: Some(Cow::new(galaxy)), adjacency: Some(adjacency) }
    }

    /// The adjacency of the galaxy.
    #[must_use]
    pub const fn adjacency(&self) -> Option<&Arc<Adjacency>> {
        self.adjacency.as_ref()
    }

    /// Replace the galaxy; the adjacency follows.
    pub fn set_galaxy(&mut self, galaxy: CoreGalaxy) {
        *self = Self::from_galaxy(galaxy);
    }
}
