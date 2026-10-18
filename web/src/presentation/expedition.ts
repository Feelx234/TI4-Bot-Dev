/**
 * The Thunder's Edge expedition as the turn bar shows it. The engine offers one component action
 * per slice the seat can pay (`component|expedition|<slice>`); the state says which slices are
 * taken and by whom (`expedition_slices`) and whether the table plays Thunder's Edge content at
 * all (`thunders_edge`). Nothing here decides anything: it only lays the facts out so the six
 * slices are visible, also when none of them can be paid right now.
 */

export interface ExpeditionSliceMeta {
  id: string;
  /** What the slice costs, in the words of the rules. */
  cost: string;
}

/** The six slices in the engine's order (`thunders_edge.rs` SLICES). */
export const EXPEDITION_SLICES: readonly ExpeditionSliceMeta[] = [
  { id: "resources", cost: "Spend 5 resources" },
  { id: "influence", cost: "Spend 5 influence" },
  { id: "trade_goods", cost: "Spend 3 trade goods" },
  { id: "action_cards", cost: "Discard 2 action cards" },
  { id: "secret", cost: "Discard 1 unscored secret objective" },
  { id: "tech_planet", cost: "Exhaust 1 technology-specialty planet" },
];

export const EXPEDITION_INTRO =
  "Claim one of six expedition slices; first claim gives your breakthrough; the sixth completes the expedition. Committed at the end of your turn: it uses no action.";

export const EXPEDITION_PLACEMENT =
  "Each slice can be claimed once per game. The sixth claim picks the system; whoever holds the most slices places that many infantry on Thunder's Edge and takes the planet.";

/** What the turn bar needs to know about the expedition. */
export interface ExpeditionFacts {
  /** The table plays Thunder's Edge content, or the engine offered a slice anyway. */
  enabled: boolean;
  /** Slice id to the seat that claimed it. */
  slices: Readonly<Record<string, string>>;
  /** The viewing seat already holds its breakthrough, so another slice grants nothing new. */
  breakthroughHeld: boolean;
}

export function sliceOfOptionId(id: string): string | null {
  const prefix = "component|expedition|";
  return id.startsWith(prefix) ? id.slice(prefix.length) : null;
}

export function sliceMeta(slice: string): ExpeditionSliceMeta | null {
  return EXPEDITION_SLICES.find((s) => s.id === slice) ?? null;
}

/** `state.expedition_slices` as slice to seat (unknown shapes read as nothing claimed). */
export function readExpeditionSlices(
  state: Record<string, unknown> | null | undefined,
): Record<string, string> {
  const raw = state?.expedition_slices;
  const out: Record<string, string> = {};
  if (!raw || typeof raw !== "object") return out;
  for (const [slice, seat] of Object.entries(raw as Record<string, unknown>)) {
    if (typeof seat === "string") out[slice] = seat;
  }
  return out;
}

/** Whether the table switched Thunder's Edge content on (`thunders_edge` is left out of the state while off). */
export function readThundersEdge(state: Record<string, unknown> | null | undefined): boolean {
  return state?.thunders_edge === true;
}

/** The system Thunder's Edge was placed in, once the expedition is complete. */
export function readThundersEdgeSystem(state: Record<string, unknown> | null | undefined): string | null {
  const system = state?.thunders_edge_system;
  return typeof system === "string" && system ? system : null;
}

/** How many slices each seat holds, for the placement line ("Sol 2, Hacan 1"). */
export function sliceCounts(slices: Readonly<Record<string, string>>): Record<string, number> {
  const out: Record<string, number> = {};
  for (const seat of Object.values(slices)) out[seat] = (out[seat] ?? 0) + 1;
  return out;
}
