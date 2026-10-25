// Generated from the Rust types by `cargo wire-web` (ti4_wasm::wire_ts). Do not edit.
import type { RouteFact } from "./RouteFact.ts";

/**
 * How a ship gets to the active system.
 */
export type MoveFact = { 
/**
 * The move uses Gravity Drive: the ship cannot arrive without it.
 */
gravity_drive: boolean, 
/**
 * The move exhausts the Ionian Fuel Refinery.
 */
ionian: boolean, 
/**
 * The systems that the ship enters, in order. The last one is the active system.
 */
path: Array<string>, 
/**
 * The gravity rifts that the ship leaves on this path. It rolls once for each.
 */
rifts: Array<string>, 
/**
 * The other ways to the active system, from rules version 29 on, when the ship has more than
 * one: up to `ti4_engine::tactical::MAX_ROUTES`, the default (the path above) first. A move
 * names one by its place in this list (`PlanStep::Move::route`). Empty: the path above is the
 * only route.
 */
routes?: Array<RouteFact>, };
