// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.

/**
 * A game of a fixed setup: a seeded game on the recommended map for its count, or a scenario
 * of `ti4_host::view::scenario` (its seed and its count win). Seat `i` has the id
 * [`SEATS`]`[i]`.
 */
export type Local = { seed: number, players: number, 
/**
 * The seats that the page plays (bit 0 is seat `a`).
 */
humans: number, scenario?: string, 
/**
 * The seats that the rule bot plays where the page does not. It decides from the seed, the
 * seat and the number of the decision, as on the server; the other seats decide at random.
 */
rule_seats: number, };
