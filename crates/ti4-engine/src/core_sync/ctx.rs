//! The per-game tables of the codecs: the owner table and the text interner.

use std::collections::{BTreeSet, HashMap};

use ti4_engine_core::ids::Seat;
use ti4_engine_core::state::{Code, Slice};
use ti4_model::state::GameState;

use super::error::{R, bad};

/// The owners of one table in text order: seat `i` is the `i`-th (docs 1.3). At most 16.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Owners {
    ids: Vec<String>,
}

impl Owners {
    /// Every player and every other owner that can appear on the board or in the state: unit owners,
    /// command tokens, planet control and coexistence, the Styx holder, captured units' owners,
    /// plot tokens, the keys of the per-player maps.
    ///
    /// # Errors
    /// More than 16 owners.
    pub fn of(state: &GameState) -> R<Self> {
        let plot_tokens: Vec<ti4_model::id::PlayerId> =
            state.players.iter().flat_map(|p| ti4_model::plots::read(&p.plots)).flat_map(|pl| pl.tokens).collect();
        let mut ids: BTreeSet<&str> = BTreeSet::new();
        ids.extend(plot_tokens.iter().map(|p| p.as_str()));
        ids.extend(state.players.iter().map(|p| p.id.as_str()));
        ids.extend(state.seating_order.iter().map(|p| p.as_str()));
        ids.insert(state.speaker.as_str());
        ids.extend(state.active.iter().map(|p| p.as_str()));
        ids.extend(state.styx_holder.iter().map(|p| p.as_str()));
        ids.extend(state.influence_pays_for_units.iter().map(|p| p.as_str()));
        ids.extend(state.agenda_elected_override.iter().map(|p| p.as_str()));
        for system in state.board.values() {
            ids.extend(system.units.iter().map(|u| u.owner.as_str()));
            ids.extend(system.planet_units.values().flatten().map(|u| u.owner.as_str()));
            ids.extend(system.command_tokens.iter().map(|p| p.as_str()));
            ids.extend(system.planet_control.values().map(|p| p.as_str()));
            ids.extend(system.coexisting.values().flatten().map(|p| p.as_str()));
        }
        for p in &state.players {
            ids.extend(p.duress_by.iter().map(|x| x.as_str()));
            ids.extend(p.captured_units.iter().map(|(o, _)| o.as_str()));
        }
        ids.extend(state.scored_objectives.keys().map(|p| p.as_str()));
        ids.extend(state.promissory_notes.values().map(|p| p.as_str()));
        ids.extend(state.support_holders.iter().flat_map(|(a, b)| [a.as_str(), b.as_str()]));
        ids.extend(state.expedition_slices.values().map(|p| p.as_str()));
        if ids.len() > 16 {
            return Err(bad(Slice::Seats, "more than 16 owners", ids.len()));
        }
        Ok(Self { ids: ids.into_iter().map(str::to_owned).collect() })
    }

    #[must_use]
    pub fn seat(&self, id: &str) -> Option<Seat> {
        self.ids.binary_search_by(|s| s.as_str().cmp(id)).ok().map(|i| Seat(i as u8))
    }

    #[must_use]
    pub fn id(&self, seat: Seat) -> Option<&str> {
        self.ids.get(usize::from(seat.0)).map(String::as_str)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// Interner for the open texts of the state (`Code(u16)`): reason texts, presentation phases,
/// faction-mark keys and values, promise texts. Persistent for a game, so codes stay stable between
/// lends; package X1 replaces a use by an enum when its set closes. `Code(0)` is the diplomacy rules
/// tag, so a default `Diplomacy` equals a loaded default `DiplomacyState`.
#[derive(Debug, Clone)]
pub struct Texts {
    items: Vec<Box<str>>,
    index: HashMap<Box<str>, u16>,
}

impl Default for Texts {
    fn default() -> Self {
        let mut t = Self { items: Vec::new(), index: HashMap::new() };
        t.intern_unchecked(ti4_model::diplomacy::DIPLOMACY_RULES_V1);
        t
    }
}

impl Texts {
    fn intern_unchecked(&mut self, text: &str) -> Code {
        if let Some(i) = self.index.get(text) {
            return Code(*i);
        }
        let i = self.items.len() as u16;
        self.items.push(text.into());
        self.index.insert(text.into(), i);
        Code(i)
    }

    /// # Errors
    /// More than 65 535 distinct texts.
    pub fn intern(&mut self, slice: Slice, text: &str) -> R<Code> {
        if !self.index.contains_key(text) && self.items.len() >= usize::from(u16::MAX) {
            return Err(bad(slice, "text table full", text));
        }
        Ok(self.intern_unchecked(text))
    }

    /// # Errors
    /// A code this table never issued.
    pub fn text(&self, slice: Slice, code: Code) -> R<&str> {
        self.items.get(usize::from(code.0)).map(|b| &**b).ok_or_else(|| bad(slice, "unknown text code", code.0))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// The tables of one game.
#[derive(Debug, Clone, Default)]
pub struct Ctx {
    pub owners: Owners,
    pub texts: Texts,
}

impl Ctx {
    /// # Errors
    /// More than 16 owners.
    pub fn of(state: &GameState) -> R<Self> {
        Ok(Self { owners: Owners::of(state)?, texts: Texts::default() })
    }

    /// Rebuild the owner table for `state` (an owner may have appeared since the last lend), keeping
    /// the text codes.
    ///
    /// # Errors
    /// More than 16 owners.
    pub fn refresh(&mut self, state: &GameState) -> R<()> {
        let owners = Owners::of(state)?;
        if owners != self.owners {
            self.owners = owners;
        }
        Ok(())
    }

    pub(crate) fn seat(&self, slice: Slice, id: &str) -> R<Seat> {
        self.owners.seat(id).ok_or_else(|| bad(slice, "owner not in the table", id))
    }

    pub(crate) fn owner(&self, slice: Slice, seat: Seat) -> R<&str> {
        self.owners.id(seat).ok_or_else(|| bad(slice, "seat not in the table", seat.0))
    }
}
