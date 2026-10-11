// The whole screen from one update of the game and what the player staged in this page.
import type { ShellView } from "../../model";
import type { Catalog, SessionUpdate } from "../wire";
import { type LocalState, boardTask, openChoice, selectAction } from "./action";
import { selectBoard } from "./board";
import { type CardSettings, NO_CARD_SETTINGS, selectActionCards, withoutNever } from "./cards";
import { selectExploration } from "./exploration";
import { factionName, phaseName } from "./names";
import { isEmpty } from "../productionDraft";
import { selectPlayers, selectSeats } from "./players";
import { selectScore } from "./score";
import { selectSecrets } from "./secrets";
import { selectTactical, tacticalFacts } from "./tactical";

function status(update: SessionUpdate, local: LocalState): string {
  const turn = update.turn_status;
  const faction = (seat: string) =>
    factionName(update.view.players.find((player) => player.id === seat)?.faction ?? seat);
  if (local.error) {
    return "The game stopped";
  }
  if (local.replaying) {
    return `Replaying ${local.replaying.done} of ${local.replaying.total}`;
  }
  if (local.draft) {
    return "Private draft";
  }
  if (openChoice(update, local)) {
    return "Your decision";
  }
  switch (turn.kind) {
    case "game_over":
      return turn.winner ? `Game over · ${faction(turn.winner)} wins` : "Game over";
    case "waiting_for_decision":
      return local.sent ? "Sent · the game goes on" : `Waiting for ${faction(turn.seat)}`;
    case "active_turn":
      return `Turn of ${faction(turn.player)}`;
    case "phase_transition":
      return "Between phases";
  }
}

export function selectShell(
  shown: SessionUpdate,
  local: LocalState,
  /** The printed cards. Without it they are shown by id. */
  printed: Catalog | null = null,
  cards: CardSettings = NO_CARD_SETTINGS,
): ShellView {
  // A card that the viewer is never offered is not an option of the open choice.
  const update = withoutNever(shown, cards.never);
  const catalog = printed?.exploration ?? null;
  // The activation, the movement, the landings and the production have their own screens;
  // every other decision is a list.
  const facts = tacticalFacts(update, local);
  const movement =
    facts?.kind === "movement" && local.step === null
      ? { facts, draft: local.movement, handled: local.handled }
      : null;
  const landing =
    facts?.kind === "invasion" ? { facts, draft: local.landing, chosen: local.landingKind } : null;
  const production =
    facts?.kind === "production"
      ? {
          facts,
          draft: local.production,
          stage: local.productionStage,
        }
      : null;
  return {
    seats: selectSeats(update),
    accent:
      local.draft ||
      local.staged ||
      (movement && Object.keys(local.movement).length) ||
      (landing && Object.keys(local.landing).length) ||
      (production && !isEmpty(local.production))
        ? "draft"
        : "live",
    toolbar: {
      round: update.view.round,
      phase: phaseName(update.view.phase),
      status: status(update, local),
      workspace: local.draft ? "draft" : "live",
      draftLocked: local.draftLocked,
      past: null,
      draft: local.draft && {
        canUndo: local.draft.canUndo,
        canRedo: local.draft.canRedo,
        canApply: local.draft.canApply,
        applyHint: local.draft.applyHint,
      },
    },
    players: selectPlayers(update, printed),
    board: selectBoard(
      update,
      local.inspected,
      boardTask(update, local),
      movement,
      landing,
      catalog,
      production,
    ),
    action: facts
      ? selectTactical(update, local, facts, catalog)
      : selectAction(update, local, catalog),
    reference: {
      objectives: selectScore(update),
      technology: [],
      cards: [],
      actionCards: selectActionCards(update, printed, cards),
      secrets: selectSecrets(update, printed),
      exploration: selectExploration(update, catalog),
      log: [],
    },
    apply: local.draft?.apply ?? null,
    reveal: null,
    toast: null,
    announcement: "",
  };
}
