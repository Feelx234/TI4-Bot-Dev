import type { LobbyDto } from "../protocol/types.ts";

/**
 * Whether the viewer's seat can plan its status-phase command tokens ahead of time. Planning exists
 * so a person can decide while other people act; a table where every other seat is an in-process
 * bot (singleplayer, offline or on the server) never waits on anyone, so the plan is only clutter.
 * Any other seat that is not a `random`/`smart` bot (a person, or an MLP bot, which carries no
 * `bot` marker) keeps it available.
 */
export function statusPlanAvailable(lobby: LobbyDto | null | undefined, viewerSeat: string | null | undefined): boolean {
  if (!lobby || !viewerSeat) return true;
  const others = lobby.slots.filter((slot) => slot.occupant !== viewerSeat);
  if (others.length === 0) return true;
  return !others.every((slot) => slot.bot === "random" || slot.bot === "smart");
}
