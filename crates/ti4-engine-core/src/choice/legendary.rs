//! Decision types of the `legendary` area: the abilities of the legendary planets (package W16; string `legendary.rs`).
//!
//! The offer ("use a legendary planet ability", at the end of the holder's turn or as they pass) and the follow-up question
//! of each ability. Thunder's Edge's Jupiter Brain asks nothing after the offer. The adapter side is
//! `ti4-engine/src/render/legendary.rs`.

use crate::generated::{ActionCard, Leader, Planet, Relic, SecretObjective, System, Technology, Unit};

/// Where a planet is on the map, for the option's `system` payload: it compares equal, orders equal and hashes to nothing, so an
/// answer parsed from an id that does not name the system equals the option that was offered.
#[derive(Debug, Clone, Copy, Default)]
pub struct At(pub Option<System>);

impl PartialEq for At {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for At {}
impl PartialOrd for At {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for At {
    fn cmp(&self, _: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
}
impl std::hash::Hash for At {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}

/// When the offer is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum When {
    /// `legendary::end_turn`: "at the end of your turn".
    EndOfTurn,
    /// `legendary::pass`: "when you pass".
    Pass,
}

/// What the area asks. Parameters only; the option list is the window's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// "use a legendary planet ability" (as you pass, when `When::Pass`): one option per ability whose card is ready.
    Offer { when: When },
    /// "Exterrix Headquarters" (Mallice): two trade goods, or all the commodities as trade goods (`commodities` is the
    /// count the second option names).
    Exterrix { commodities: i16 },
    /// "Imperial Arms Vault" (Hope's End): place a mech, or draw an action card.
    ArmsVault,
    /// "The Atrament: place infantry where" (Primor) / "Imperial Arms Vault: place the mech where" (Hope's End): a planet the seat
    /// controls; `unit` is the form the seat places.
    PlaceOnPlanet { source: Planet, unit: Unit },
    /// "Mirage Flight Academy: place fighters where": a system that holds a ship of the seat.
    PlaceFighters,
    /// "The Acropolis: ready what" (Emelpar): another exhausted component of the seat.
    Acropolis,
    /// "The Galactic Council: discard which secret" (Thunder's Edge Mecatol Rex).
    Council,
    /// "Maxis Central Control: gain control of which planet" (Faunus).
    Maxis,
    /// "Dok 'N Pic's Salvage Yard: salvage which card" (Garbozia).
    Salvage,
    /// "Aurex Mechanica: place which ship where" (Industrex).
    Aurex,
}

/// The component The Acropolis readies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ready {
    Planet(Planet),
    Technology(Technology),
    Relic(Relic),
    /// The ability card of a legendary planet.
    Legendary(Planet),
    Leader(Leader),
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Use the ability of this planet's card.
    Use(Planet),
    /// Exterrix Headquarters: two trade goods.
    Goods,
    /// Exterrix Headquarters: every commodity becomes a trade good.
    Convert,
    /// Imperial Arms Vault: place a mech.
    Mech,
    /// Imperial Arms Vault: draw an action card.
    Card,
    /// A planet of the seat, with the system the map shows it in.
    OnPlanet(Planet, At),
    /// A system that holds a ship of the seat.
    InSystem(System),
    /// The Acropolis: ready this component (`At` is where a planet is).
    Ready(Ready, At),
    /// The Galactic Council: discard this secret objective.
    DiscardSecret(SecretObjective),
    /// Maxis Central Control: take this planet of this system.
    Take(System, Planet),
    /// Dok 'N Pic's Salvage Yard: put this card of the discard pile on the yard.
    Salvage(ActionCard),
    /// Aurex Mechanica: place this ship in this system.
    Ship(Unit, System),
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    /// `legendary_end_of_turn`.
    EndOfTurn,
    /// `legendary_pass`.
    Pass,
    /// `legendary_exterrix`.
    Exterrix,
    /// `legendary_arms_vault`.
    ArmsVault,
    /// `legendary_place` (the planet of the Atrament and the Arms Vault, the system of the Mirage).
    Place,
    /// `legendary_acropolis`.
    Acropolis,
    /// `legendary_galactic_council`.
    GalacticCouncil,
    /// `legendary_maxis`.
    Maxis,
    /// `legendary_salvage`.
    Salvage,
    /// `legendary_aurex`.
    Aurex,
}
