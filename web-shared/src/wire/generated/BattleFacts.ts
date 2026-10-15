// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.
import type { CombatSide } from "./CombatSide.ts";

/**
 * The space combat in the active system: the seat that defends, and both sides. The attacker
 * is the seat that moves; the ships it moves in are added by the client.
 */
export type BattleFacts = { defender: string, attacker: CombatSide, defender_side: CombatSide, };
