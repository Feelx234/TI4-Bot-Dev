import { describe, expect, it } from "vitest";
import { buildBoardPresentationModel, resolveNexusFace } from "./boardPresentation.ts";
import type { BoardView } from "../protocol/types.ts";

const tile = (system_id: string, q: number, r: number, extra: Record<string, unknown> = {}) => ({
  system_id, label: `#${system_id}`, q, r, ...extra,
});
const tiles = [
  tile("18", 0, 0),
  tile("26", 0, 1),
  tile("82a", 0, 0, { special_area: "nexus", wormholes: ["GAMMA"] }),
  tile("82b", 0, 0, { special_area: "nexus", wormholes: ["ALPHA", "BETA", "GAMMA"] }),
];
const board = {
  active_system: null,
  map_tiles: tiles,
  systems: { "18": { system_id: "18", command_tokens: [], planets: {}, units: [] } },
} as unknown as BoardView;
const activate = {
  nonce: "n", actor: "p", prompt: "Activate", options: [
    { id: "26", label: "26", kind: "activate" }, { id: "82a", label: "82a", kind: "activate" },
  ],
  context: { subtype: "activate_system" },
} as never;

describe("Wormhole Nexus on the board", () => {
  it("draws one Nexus tile, always id 82a (the engine's system id), locked or open", () => {
    const locked = resolveNexusFace(board.map_tiles!, false);
    expect(locked.filter((t) => t.system_id.startsWith("82")).map((t) => t.system_id)).toEqual(["82a"]);
    expect(locked.find((t) => t.system_id === "82a")!.wormholes).toEqual(["GAMMA"]);
    const open = resolveNexusFace(board.map_tiles!, true);
    expect(open.filter((t) => t.system_id.startsWith("82")).map((t) => t.system_id)).toEqual(["82a"]);
    expect(open.find((t) => t.system_id === "82a")!.wormholes).toEqual(["ALPHA", "BETA", "GAMMA"]);
  });

  it("makes the Nexus a target when the engine offers 82a, and links it to gamma systems", () => {
    const model = buildBoardPresentationModel(board, ["p"], [], activate, "p", null, undefined, {
      nexusUnlocked: false,
      wormholeTokens: { GAMMA: "26" },
    });
    const nexus = model.tiles.filter((t) => t.specialArea === "nexus");
    expect(nexus.map((t) => t.systemId)).toEqual(["82a"]);
    expect(nexus[0].isCandidateTarget).toBe(true);
    expect(model.tiles.find((t) => t.systemId === "26")!.wormholes.map((w) => w.kind)).toEqual(["GAMMA"]);
    expect(model.nexusLinks.map((l) => l.fromSystemId)).toEqual(["26"]);
  });

  it("draws no links without a gamma token or when the Nexus is not offered", () => {
    const none = buildBoardPresentationModel(board, ["p"], [], activate, "p");
    expect(none.nexusLinks).toEqual([]);
    const idle = buildBoardPresentationModel(board, ["p"], [], null, "p", null, undefined, {
      wormholeTokens: { GAMMA: "26" },
    });
    expect(idle.nexusLinks).toEqual([]);
  });
});
