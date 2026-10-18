// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.
import type { PlayerId } from "./PlayerId.ts";

/**
 * One seat of a save, for the reader of the file.
 */
export type SaveSeat = { 
/**
 * 0-based.
 */
seat: number, player_id: PlayerId, 
/**
 * The name at the table, if the game is one of a table.
 */
name?: string, 
/**
 * `human` (the page), `smart` (the rule bot) or `random`.
 */
kind: string, faction: string, };
