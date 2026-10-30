//! Creation-time guards (package D2-1): refuse, when a game is created, what the typed state cannot represent, so
//! that a refusal (`Unmapped`) can only come from a corrupted save and not from a table somebody could start.
//!
//! * [`ids_supported`]: at most [`MAX_PLAYERS`] seats, ASCII ids, and **no id a proper prefix of another**. The
//!   typed keys order a seat by the byte after the owner id in the key text (`ti4_engine_core::keys`); `p1` and `p10`
//!   would compare the other way round than the seats, so [`keys::Owners::new`](super::keys::Owners::new) refuses
//!   such a table and the marks and outcomes of that game stay unmapped. Generated ids never do this
//!   (`player_<64 hex>`, `seat_NN`, `p{i}` below 10 seats): this makes it impossible by construction.
//! * [`factions_known`]: every seated faction is a corpus faction (`generic`, the scaffolding of `fixtures::game`,
//!   is not, and the Players codec refuses it).

use ti4_engine_core::generated::Faction;
use ti4_model::id::PlayerId;

use crate::generated::IdStr;

/// The most seats the typed state holds (`ti4_engine_core::state::MAX_PLAYERS`).
pub const MAX_PLAYERS: usize = ti4_engine_core::state::MAX_PLAYERS;

/// A message when the ids of a table cannot be represented as owners of the typed state.
///
/// # Errors
/// More than [`MAX_PLAYERS`] ids, a non-ASCII or empty id, a repeated id, or an id that is a proper prefix of another.
pub fn ids_supported(ids: &[PlayerId]) -> Result<(), String> {
    if ids.len() > MAX_PLAYERS {
        return Err(format!("a table seats at most {MAX_PLAYERS} players, not {}", ids.len()));
    }
    for (i, a) in ids.iter().enumerate() {
        let a = a.as_str();
        if a.is_empty() || !a.is_ascii() {
            return Err(format!("player id {a:?} is not a non-empty ASCII text"));
        }
        for b in &ids[i + 1..] {
            let b = b.as_str();
            if a == b {
                return Err(format!("player id {a:?} is used twice"));
            }
            if b.starts_with(a) || a.starts_with(b) {
                // equal lengths cannot occur here: a prefix of equal length is the same id, refused as `used twice` above
                let (short, long) = if a.len() < b.len() { (a, b) } else { (b, a) }; // mutants:skip
                return Err(format!(
                    "player id {short:?} is a prefix of {long:?}: the typed keys cannot order the two seats"
                ));
            }
        }
    }
    Ok(())
}

/// A message when a seated faction is not a corpus faction.
///
/// # Errors
/// A faction id that is not in [`Faction::ALL`].
pub fn factions_known<'a>(factions: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
    for f in factions {
        if Faction::from_id(f).is_none() {
            return Err(format!("faction {f:?} is not a corpus faction"));
        }
    }
    Ok(())
}
