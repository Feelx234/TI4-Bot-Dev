import type { LobbyDto } from "../protocol/types.ts";
import { playerDisplay } from "./playerDisplay.ts";

// Only standalone participant tokens in prose are presentation references. In
// particular, a token inside a composite machine/content ID is not a name.
const PARTICIPANT_TOKEN = /(^|[^\w:/|.-])(player_[a-zA-Z0-9_]+)(?!\.[\w])(?=$|[^\w:/|-])/g;
const GENERATED_ID = /^player_[a-f0-9]{64}$/;

export function participantText(
  text: string,
  lobby: LobbyDto | null,
  seatingOrder: readonly string[],
): string {
  const known = new Set(
    lobby?.slots.flatMap((slot) => (slot.occupant ? [slot.occupant] : [])) ?? [],
  );
  return text.replace(PARTICIPANT_TOKEN, (whole, prefix: string, id: string) =>
    known.has(id) || GENERATED_ID.test(id)
      ? prefix + playerDisplay(lobby, seatingOrder, id).label
      : whole,
  );
}
