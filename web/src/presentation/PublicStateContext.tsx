import React, { createContext, useContext, useMemo } from "react";
import { readBreakthroughs } from "./breakthroughs.ts";
import { readFrontierTokens } from "./explorationMarks.ts";
import { readExplorationLog, type ExplorationRecordDto } from "./explorationToasts.ts";
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
  /** The table plays Thunder's Edge content (`thunders_edge`; left out of the state while off). */
  thundersEdge: boolean;
  /** Expedition slice to the seat that claimed it. */
  expeditionSlices: Readonly<Record<string, string>>;
}

const EMPTY: PublicStateFacts = {
  breakthroughs: {},
  explorations: [],
  frontierSystems: new Set(),
  thundersEdge: false,
  expeditionSlices: {},
};
const PublicStateContext = createContext<PublicStateFacts>(EMPTY);

export function publicStateFacts(state: Record<string, unknown> | null | undefined): PublicStateFacts {
  return {
    breakthroughs: readBreakthroughs(state),
    explorations: readExplorationLog(state),
    frontierSystems: new Set(readFrontierTokens(state)),
    thundersEdge: readThundersEdge(state),
    expeditionSlices: readExpeditionSlices(state),
  };
}

export const PublicStateProvider: React.FC<{
  state: Record<string, unknown> | null | undefined;
  children: React.ReactNode;
}> = ({ state, children }) => {
  const facts = useMemo(() => publicStateFacts(state), [state]);
  return <PublicStateContext.Provider value={facts}>{children}</PublicStateContext.Provider>;
};

export const usePublicState = (): PublicStateFacts => useContext(PublicStateContext);
