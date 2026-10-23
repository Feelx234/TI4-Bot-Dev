import { describe, expect, it } from "vitest";
import type { LobbyDto, LobbySlot } from "../protocol/types.ts";
import { statusPlanAvailable } from "./statusPlanAvailable.ts";

const slot = (occupant: string, bot?: string): LobbySlot => ({
  slot_id: `s-${occupant}`,
  position: 1,
  occupant,
  nickname: occupant,
  connected: true,
  can_take_over: false,
  ...(bot ? { bot } : {}),
});
const lobby = (...slots: LobbySlot[]) => ({ slots }) as unknown as LobbyDto;

describe("statusPlanAvailable", () => {
  it("is off in singleplayer: every other seat is an in-process bot", () => {
    expect(statusPlanAvailable(lobby(slot("a"), slot("b", "smart"), slot("c", "random")), "a")).toBe(false);
  });
  it("stays on with another human seat", () => {
    expect(statusPlanAvailable(lobby(slot("a"), slot("b"), slot("c", "smart")), "a")).toBe(true);
  });
  it("stays on with an MLP bot seat (no bot marker, may be slow)", () => {
    expect(statusPlanAvailable(lobby(slot("a"), slot("b", "smart"), slot("c")), "a")).toBe(true);
  });
  it("stays on when unknown", () => {
    expect(statusPlanAvailable(null, "a")).toBe(true);
    expect(statusPlanAvailable(lobby(slot("a")), "a")).toBe(true);
  });
});
