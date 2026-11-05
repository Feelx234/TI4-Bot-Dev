//! What can go wrong when a table is set up.

/// A table the setup refuses, or cannot build.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SetupError {
    #[error("a game needs at least one player")]
    NoPlayers,
    #[error("the typed core seats at most 8 players, not {0}")]
    TooManyPlayers(usize),
    #[error("one faction per seat is required ({players} seats, {factions} factions)")]
    FactionCount { players: usize, factions: usize },
    #[error("no strategy card set for the requested sources")]
    NoStrategyCardSet,
    #[error("Thunder's Edge content is switched off")]
    ThundersEdge,
    #[error("unknown faction {0}")]
    UnknownFaction(String),
    #[error("faction {0} has no home system")]
    NoHomeSystem(String),
    #[error("{0}")]
    Seating(String),
    #[error("the typed core cannot hold {what} `{text}`")]
    Unmapped { what: &'static str, text: String },
    #[error("more than 16 owners")]
    Owners,
    #[error("the galaxy cannot be expressed in the typed core")]
    Galaxy,
    #[error("the speaker seat {0} is not at the table")]
    SpeakerSeat(usize),
    #[error("rules version {0} is not supported (the core plays {1} to {2})")]
    RulesVersion(u32, u32, u32),
}

pub(crate) fn unmapped(what: &'static str, text: impl ToString) -> SetupError {
    SetupError::Unmapped { what, text: text.to_string() }
}
