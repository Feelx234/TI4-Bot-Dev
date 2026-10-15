// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.
import type { SideInput } from "./SideInput.ts";

/**
 * The two sides of a space combat, as [`ti4_space_odds`] and `{"query":"space_odds"}` of
 * [`ti4_host_query`] read them.
 */
export type SpaceOddsRequest = { attacker: SideInput, defender: SideInput, };
