/**
 * In whose turns an action card can be played, for the per-game "Only in my turn" choice of the
 * reaction setting (the server mutes such a card while another seat is the active player of the
 * action phase, and never outside the action phase).
 *
 * The choice says something only for a card that can come up in both: a card whose window is
 * always the owner's own action ("After you activate a system") or always another player's
 * ("After another player activates a system") would make "Only in my turn" the same as Always or
 * as Never, and a card of another phase never meets an active player.
 */
export type CardTurns = "both" | "own" | "others";

/** Cards whose window text does not tell, by printed name (an invasion is the active player's). */
const BY_NAME: Readonly<Record<string, CardTurns>> = {
  Blitz: "own",
  Disable: "own",
  "War Machine": "own",
  Bunker: "others",
  Rout: "others",
  "Experimental Battlestation": "others",
  "Reverse Engineer": "others",
  "Puppets on a String": "others",
};

const OWN_WINDOWS = [/^After you (activate|perform an action)/i, /^When you gain control of a planet/i];
const OTHERS_WINDOWS = [
  /^After another player (activates|commits units|gains control)/i,
  /^When another player would perform a strategic action/i,
  /^At the start of another player's turn/i,
];

/** The turns a card can come up in, from its printed name and window text. */
export function actionCardTurns(name: string, window: string | undefined): CardTurns {
  const byName = BY_NAME[name];
  if (byName) return byName;
  const text = window ?? "";
  if (OWN_WINDOWS.some((re) => re.test(text))) return "own";
  if (OTHERS_WINDOWS.some((re) => re.test(text))) return "others";
  return "both";
}

/** Whether "Only in my turn" says something for a card (a reaction of the action phase, in both). */
export function offersOwnTurnForCard(card: { name: string; phase?: string; window?: string } | null | undefined): boolean {
  if (!card) return false;
  if (card.phase !== "Action" && card.phase !== "Any") return false;
  if (!card.window || card.window === "Action") return false;
  return actionCardTurns(card.name, card.window) === "both";
}
