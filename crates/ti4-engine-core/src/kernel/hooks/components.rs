//! The codes of the faction component actions of the action menu (package KM0/KM1): `ComponentAction { module, code, arg }` names the
//! offering faction (for Orbital Drop, Production Biomes and Trade Convoys, the faction that prints the ability) and one of the codes
//! below; `arg` is the thing the action is about when it has one (a technology, a planet, a seat). The renderer (`ti4-engine`
//! `render/component.rs`) turns the triple into the string engine's option id, kind and label; the listings are
//! `driver::menu_sources::faction` (the three that belong to no module) and the modules' `component_actions` hooks.
//!
//! The string ids these stand for: `faction|orbital_drop`, `faction|production_biomes`, `faction|trade_convoys`, `faction|muaat|{nucleus,
//! star_forge,inferno,fires}`, `faction|ghost|wg`, `faction|cabal|vortex`, `faction|mahact|vaults|<tech>`, `faction|mentak|pop`,
//! `faction|nekro|c4r|{pds,repair,draw}`, `faction|winnu|lgf`, `faction|titans|terraform|<planet>`, `faction|naaz|{fabrication_relic,
//! fabrication_token,bmf}`, `faction|yssaril|{stall_tactics,mi|<seat>,bt_stall|<seat>}`, `faction|empyrean|{blood_pact,dark_pact}`,
//! and the two of the content this game leaves out (`faction|arborec|psychospore`, `faction|saar|deorbit_barrage`).

pub const ORBITAL_DROP: u16 = 1;
pub const PRODUCTION_BIOMES: u16 = 2;
pub const TRADE_CONVOYS: u16 = 3;

pub const NUCLEUS: u16 = 10;
pub const STAR_FORGE: u16 = 11;
pub const INFERNO: u16 = 12;
pub const FIRES: u16 = 13;

pub const WORMHOLE_GENERATOR: u16 = 20;
pub const VORTEX: u16 = 21;
/// `arg`: the technology (`Technology as u16`).
pub const VAULTS: u16 = 22;
pub const PROMISE_OF_PROTECTION: u16 = 23;

pub const C4R_PLACE: u16 = 30;
pub const C4R_REPAIR: u16 = 31;
pub const C4R_DRAW: u16 = 32;

pub const LAZAX_GATE_FOLDING: u16 = 40;
/// `arg`: the planet (`Planet as u16`).
pub const TERRAFORM: u16 = 41;

pub const FABRICATION_RELIC: u16 = 50;
pub const FABRICATION_TOKEN: u16 = 51;
pub const BLACK_MARKET_FORGERY: u16 = 52;

pub const STALL_TACTICS: u16 = 60;
/// `arg`: the owner's seat.
pub const BORROWED_STALL_TACTICS: u16 = 61;
/// `arg`: the target's seat.
pub const MAGEON_IMPLANTS: u16 = 62;

pub const BLOOD_PACT: u16 = 70;
pub const DARK_PACT: u16 = 71;

/// Out of scope (Thunder's Edge): never listed natively; a position that offers one is a counted gap.
pub const PSYCHOSPORE: u16 = 80;
/// Out of scope (Thunder's Edge).
pub const DEORBIT_BARRAGE: u16 = 81;
