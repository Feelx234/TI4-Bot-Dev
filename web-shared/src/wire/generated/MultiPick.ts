// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.

/**
 * The rule a multi-pick answer follows ([`Choice::multi_pick`]). The engine checks it; a client may check it as you type.
 */
export type MultiPick = { 
/**
 * `payment`: the options spent must cover `owed`, and each must be needed (without the largest the rest is short).
 * `purchase`: `buy:N` tokens of `owed` each, between `buy[0]` and `buy[1]` of them, paid the same way (nothing for none).
 */
kind: string, owed: number, 
/**
 * The options that may be named more than once, with the most times (trade goods, commodities).
 */
repeat?: { [key in string]: number }, 
/**
 * A purchase: the fewest and the most tokens.
 */
buy?: [number, number], };
