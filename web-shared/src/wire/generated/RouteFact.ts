// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.

/**
 * One route of a ship to the active system and what the ship does on it.
 */
export type RouteFact = { 
/**
 * The systems that the ship enters, in order; the origin first, the active system last.
 */
path: Array<string>, 
/**
 * The route that the engine takes when a move names none.
 */
default: boolean, 
/**
 * The route spends Gravity Drive.
 */
gravity_drive: boolean, 
/**
 * The route exhausts the Ionian Fuel Refinery.
 */
ionian: boolean, 
/**
 * The gravity rifts that the ship leaves on this route. It rolls once for each.
 */
rifts: Array<string>, 
/**
 * The hops of the route through a wormhole.
 */
wormholes: number, 
/**
 * The systems between the origin and the active system that are anomalies.
 */
anomalies: Array<string>, 
/**
 * What the ship can load on this route: places in [`MovementFacts::cargo`].
 */
loads?: Array<number>, };
