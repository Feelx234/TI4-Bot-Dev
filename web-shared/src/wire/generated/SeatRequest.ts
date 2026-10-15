// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.
import type { SeatKind } from "./SeatKind.ts";

/**
 * One player of a table: who plays the seat, and the faction that was chosen for it.
 */
export type SeatRequest = { kind: SeatKind, faction: string | null, 
/**
 * The name of the player at the table. With a name for every player, the updates of the
 * game say who sits in each seat.
 */
name: string | null, };
