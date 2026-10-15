// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.

/**
 * One side of a space combat in the active system as the state has it, before a ship moves
 * in: the ships there by unit id, how many of them are damaged, and the units of the side that
 * fire SPACE CANNON in that system before the combat. The shape of a side of `ti4_calc`.
 */
export type CombatSide = { faction: string, units?: { [key in string]: number }, damaged?: { [key in string]: number }, guns?: { [key in string]: number }, };
