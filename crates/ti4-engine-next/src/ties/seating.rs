//! From `seating.rs` of the legacy crate (copies).

/// The factions a lobby table can seat: the 24 base and Prophecy of Kings factions the smart-bot
/// survey roster plays (`ti4_host::preset::ROSTER`, a server test keeps the two in step), base
/// game first. Left out: the three Keleres variants (each takes another faction's home system, a
/// table-dependent rule), the Obsidian (cannot be chosen during setup) and the Thunder's Edge
/// factions (their home tiles are not in the Prophecy of Kings source set every server game uses).
pub const LOBBY_FACTIONS: [&str; 24] = [
    "sol", "hacan", "letnev", "xxcha", "jolnar", "l1z1x", "arborec", "mentak", "muaat", "naalu",
    "nekro", "saar", "sardakk", "winnu", "yin", "yssaril", "ghost", "argent", "cabal", "empyrean",
    "mahact", "naaz", "nomad", "titans",
];
