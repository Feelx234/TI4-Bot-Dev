// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.

/**
 * Outcome statistics from [`arena`] rollouts.
 */
export type BattleResponse = { simulations: number, attacker_win_rate: number, defender_win_rate: number, mutual_destruction_rate: number, unresolved_rate: number, average_rounds: number, attacker_expected_survivors: { [key in string]: number }, defender_expected_survivors: { [key in string]: number }, attacker_fielded: { [key in string]: number }, defender_fielded: { [key in string]: number }, };
