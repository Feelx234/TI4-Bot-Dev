import React, { createContext, useContext, useMemo } from "react";
import { readBreakthroughs } from "./breakthroughs.ts";
import { readFrontierTokens } from "./explorationMarks.ts";
import { EMPTY_HELD_STATE, readHeldState, type HeldState } from "./heldItems.ts";
import { readExplorationLog, type ExplorationRecordDto } from "./explorationToasts.ts";

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
}

const EMPTY: PublicStateFacts = { breakthroughs: {}, explorations: [], frontierSystems: new Set(), held: EMPTY_HELD_STATE };
const PublicStateContext = createContext<PublicStateFacts>(EMPTY);

export function publicStateFacts(state: Record<string, unknown> | null | undefined): PublicStateFacts {
  return {
    breakthroughs: readBreakthroughs(state),
    explorations: readExplorationLog(state),
    frontierSystems: new Set(readFrontierTokens(state)),
    held: readHeldState(state),
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
