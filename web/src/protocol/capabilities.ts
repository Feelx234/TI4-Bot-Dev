/**
 * What a game session can do. The UI reads these flags to hide features a transport cannot
 * offer (the offline single-player transport has no server behind it). The server transport
 * offers everything.
 */
export interface TransportCapabilities {
  /** "What if this past decision were different" preview (`previewSplice`). */
  splicePreview: boolean;
  /** Turn redo (`fetchTurnRedoStatus` / `turnRedoCommand`). */
  turnRedo: boolean;
  /** Lobby, other seats, spectators, presence: a game shared with other people. */
  multiplayer: boolean;
  /** The in-game feedback button (`POST /api/feedback`). */
  feedback: boolean;
}

export const SERVER_CAPABILITIES: TransportCapabilities = {
  splicePreview: true,
  // Switched off (docs/DISABLE_XF123.md): the server refuses /turn-redo (403) because the deck
  // reserve of a redone timeline is not carried by the core. The offline transport replays
  // answers instead and keeps its own flag. Only the screenshot runner sets the variable, so the
  // folders that show the bar (Q, AW, CF) can still be rebuilt against their mocked server.
  turnRedo: import.meta.env.VITE_TI4_SERVER_TURN_REDO === "1",
  multiplayer: true,
  feedback: true,
};


/*
 * Server `fetch` calls outside the transport (not refactored; offline mode hides or replaces them):
 *  - lobby (multiplayer):   components/Lobby.tsx (/api/games), hooks/useLobbySession.ts
 *                           (/api/games/:id/lobby/*), hooks/usePresence.ts (heartbeat),
 *                           components/MyGamesDialog.tsx (terminate)
 *  - maps:                  hooks/useMapCatalog.ts
 *  - feedback:              feedback/FeedbackSheet.tsx (/api/feedback)
 *  - access / build info:   access/accessGate.ts (/api/access), buildInfo.ts (/health)
 *  - dev only:              dev/ScenarioLauncher.tsx (/api/dev/scenarios)
 *  - history/replay/redo/splice/snapshot: inside GameSessionClient (behind GameTransport)
 */
