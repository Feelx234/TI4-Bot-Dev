import React, { createContext, useContext, useMemo } from "react";
import { readBreakthroughs } from "./breakthroughs.ts";
import { readFrontierTokens } from "./explorationMarks.ts";
import { EMPTY_HELD_STATE, readHeldState, type HeldState } from "./heldItems.ts";
import { readExplorationLog, type ExplorationRecordDto } from "./explorationToasts.ts";
import type { ScoreFacts } from "../protocol/types.ts";
import { readExpeditionSlices, readThundersEdge } from "./expedition.ts";

/**
 * Public facts the engine's state carries that the projected views leave out: each seat's
 * breakthrough, the exploration log and the frontier tokens still on the map. Read by the player
 * sheet, the map tiles and the system inspector without threading props through the shell.
 */
export interface PublicStateFacts {
  breakthroughs: Readonly<Record<string, string | null>>;
  explorations: readonly ExplorationRecordDto[];
  frontierSystems: ReadonlySet<string>;
  /** The promissory fields the Held items panel reads. */
  held: HeldState;
  /** The table plays Thunder's Edge content (`thunders_edge`; left out of the state while off). */
  thundersEdge: boolean;
  /** Expedition slice to the seat that claimed it. */
  expeditionSlices: Readonly<Record<string, string>>;
  /** Where each seat's points come from (the snapshot's `score`); null for a server that sends none. */
  score: ScoreFacts | null;
  /** Laws in play: agenda alias to its outcome (the elected seat for the ministers). */
  laws: Readonly<Record<string, string>>;
}

const EMPTY: PublicStateFacts = {
  breakthroughs: {},
  explorations: [],
  frontierSystems: new Set(),
  held: EMPTY_HELD_STATE,
  thundersEdge: false,
  expeditionSlices: {},
  laws: {},
  score: null,
};
const PublicStateContext = createContext<PublicStateFacts>(EMPTY);

function readLaws(state: Record<string, unknown> | null | undefined): Record<string, string> {
  const raw = state?.laws;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
  return Object.fromEntries(Object.entries(raw).filter((entry): entry is [string, string] => typeof entry[1] === "string"));
}

export function publicStateFacts(
  state: Record<string, unknown> | null | undefined,
  score: ScoreFacts | null = null,
): PublicStateFacts {
  const thundersEdge = readThundersEdge(state);
  return {
    // Breakthroughs come with the Thunder's Edge content (`thunders_edge` source): without it none shows.
    breakthroughs: thundersEdge ? readBreakthroughs(state) : {},
    explorations: readExplorationLog(state),
    frontierSystems: new Set(readFrontierTokens(state)),
    held: readHeldState(state),
    thundersEdge,
    expeditionSlices: readExpeditionSlices(state),
    score,
    laws: readLaws(state),
  };
}

export const PublicStateProvider: React.FC<{
  state: Record<string, unknown> | null | undefined;
  score?: ScoreFacts | null;
  children: React.ReactNode;
}> = ({ state, score, children }) => {
  const facts = useMemo(() => publicStateFacts(state, score ?? null), [state, score]);
  return <PublicStateContext.Provider value={facts}>{children}</PublicStateContext.Provider>;
};

export const usePublicState = (): PublicStateFacts => useContext(PublicStateContext);
