import { findFactionRecord } from "./factionInfo.ts";
import type { LobbyDto, LobbySlot } from "../protocol/types.ts";

/** Printed name without the article ("The Barony of Letnev" -> "Barony of Letnev"). */
export function lobbyFactionName(id: string): string {
  const name = findFactionRecord(id)?.name ?? id;
  return name.replace(/^The\s+/i, "");
}

/** Two-letter monogram for the faction badge (the project ships no faction art). */
export function factionMonogram(id: string): string {
  switch (id) {
    case "l1z1x":
      return "L1";
    case "jolnar":
      return "JN";
    case "xxcha":
      return "XX";
    default:
      return id.slice(0, 2).toUpperCase();
  }
}

/** Badge colours per faction: distinct hues, readable text on each. */
export const FACTION_HUE: Readonly<Record<string, string>> = {
  sol: "#38bdf8",
  hacan: "#f59e0b",
  letnev: "#ef4444",
  xxcha: "#22c55e",
  jolnar: "#a78bfa",
  l1z1x: "#f472b6",
};

/** The factions the server lets a seat choose; empty on a server that predates the choice. */
export function lobbyFactionPool(lobby: LobbyDto): readonly string[] {
  return lobby.factions ?? [];
}

/** Who holds each chosen faction, by faction id -> seat (except the seat being edited). */
export function takenFactions(
  lobby: LobbyDto,
  exceptPosition?: number,
): Map<string, LobbySlot> {
  const taken = new Map<string, LobbySlot>();
  for (const slot of lobby.slots) {
    if (slot.faction && slot.position !== exceptPosition) taken.set(slot.faction, slot);
  }
  return taken;
}
