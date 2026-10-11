// The action panel: every decision as one list. A dedicated screen later takes one kind of
// decision out of this list; the list stays for every kind that has none.
import type { ApplyView, BoardTaskView, FlowActionView, MenuRowView } from "../../model";
import type { LandingDraft } from "../landingDraft";
import type { MovementDraft } from "../movementDraft";
import type { MovementPlan } from "../movementPlan";
import type { ProductionDraft } from "../productionDraft";
import type { Choice, ExplorationCatalog, SessionUpdate } from "../wire";
import { exploringCard } from "./exploration";
import { factionName, sentence } from "./names";

/** The private draft that is in view: what its toolbar and its screens say. */
export interface DraftInfo {
  canUndo: boolean;
  canRedo: boolean;
  canApply: boolean;
  applyHint: string;
  /** What "Apply to Live" will send. */
  apply: ApplyView | null;
  /** Why the game did not play the whole draft. */
  problem: string | null;
}

/** What the player does in this page, between two updates. Nothing of it is sent. */
export interface LocalState {
  /** The system in the inspector. */
  inspected: string | null;
  /** The staged option of the pending choice, by id. */
  staged: string | null;
  /** The nonce of the choice that was answered. Its update is still the latest one. */
  sent: string | null;
  /** Why the game stopped, if it did. */
  error: string | null;
  /** The game is played again up to where it was; the update is out of date until it is there. */
  replaying: { done: number; total: number } | null;
  /** The last answer can be taken back. */
  canUndo: boolean;
  /** The staged movement of the open movement step. */
  movement: MovementDraft;
  /** The systems of the open movement that the player marked as done. */
  handled: string[];
  /** The staged landings of the open invasion step. */
  landing: LandingDraft;
  /** The force of the invasion facts that a tap lands: a choice of the hand. */
  landingKind: number | null;
  /** The staged production of the open production step: what is built, and what pays. */
  production: ProductionDraft;
  /** The build is chosen first; then the map is the payment. */
  productionStage: "build" | "pay";
  /** The step of the tactical action that is in view, when it is not the open one. */
  step: number | null;
  /**
   * The steps of a movement, of the landings or of a production that another decision stopped.
   * They are staged again when that step goes on.
   */
  remaining: MovementPlan | null;
  /** What the last movement that was sent came to, when it did not reach its end. */
  planNote: { text: string; error: boolean } | null;
  /** The seat that the game waits for before it lets it decide: "Step" goes on. */
  stepping: string | null;
  /** Why the viewer cannot draft now. Null when the Draft tab opens. */
  draftLocked: string | null;
  /** The private draft, when it is in view: the update is then what the draft came to. */
  draft: DraftInfo | null;
}

/** The choice that the viewer can answer now. */
export const openChoice = (update: SessionUpdate, local: LocalState): Choice | null =>
  update.pending_choice &&
  update.pending_choice.nonce !== local.sent &&
  !local.error &&
  !local.replaying
    ? update.pending_choice.choice
    : null;

/** A choice of systems is also made on the board: every option is a system. */
export function boardTask(update: SessionUpdate, local: LocalState): BoardTaskView | null {
  const choice = openChoice(update, local);
  if (!choice?.options.length || !choice.options.every((option) => option.kind === "activate")) {
    return null;
  }
  const facts = update.tactical?.kind === "activation" ? update.tactical : null;
  return {
    target: "system",
    kind: "pick",
    interactive: true,
    values: Object.fromEntries(choice.options.map((option) => [option.id, 0])),
    // Every system of the choice can be activated. The action does something in these: ships
    // can move there, the seat produces there, or its guns fire at the ships of another player.
    ...(facts
      ? {
          highlight: facts.systems
            .filter((reach) => reach.ships > 0 || !!reach.production || !!reach.cannon)
            .map((reach) => reach.system),
        }
      : {}),
    chosen: local.staged ? { [local.staged]: true } : {},
    verb: "Activate",
    unit: "system",
  };
}

const KEYS = "123456789";

export function selectAction(
  update: SessionUpdate,
  local: LocalState,
  catalog: ExplorationCatalog | null = null,
): FlowActionView {
  const choice = openChoice(update, local);
  // The source is on top: an exploration card that asks is shown as printed.
  const card = choice ? exploringCard(update, catalog) : null;
  const onBoard = boardTask(update, local) !== null;
  const staged = choice?.options.find((option) => option.id === local.staged) ?? null;
  const rows: MenuRowView[] = (choice?.options ?? []).map((option, index) => ({
    key: KEYS[index] ?? "",
    title: sentence(option.label || option.id),
    selected: option.id === staged?.id,
    disabled: false,
    intent: { type: "chooseOption", option: option.id },
  }));
  const status = update.turn_status;
  const faction = (seat: string) =>
    factionName(update.view.players.find((player) => player.id === seat)?.faction ?? seat);
  const stepping = !choice && !local.error && !local.replaying ? local.stepping : null;
  const idle = local.draft
    ? (local.draft.problem ?? "The draft has nothing to decide here.")
    : local.error
      ? local.error
      : local.replaying
        ? `Playing the saved game again: answer ${local.replaying.done} of ${local.replaying.total}.`
        : status.kind === "game_over"
          ? "The game is over."
          : stepping
            ? `${faction(stepping)} decides next.`
            : "The other seats are playing.";
  return {
    kind: "flow",
    title: local.draft ? "Tactical action" : choice ? "Decision" : "Waiting",
    subtitle: "",
    badge: local.draft
      ? { tone: "draft", label: "Private draft" }
      : staged
        ? { tone: "draft", label: "Not sent" }
        : null,
    past: null,
    tabs: [],
    selected: 0,
    current: null,
    contentKey: update.pending_choice?.nonce ?? "idle",
    interrupt: null,
    closing: [],
    heading: local.draft ? "Draft" : choice ? sentence(choice.prompt) : "Waiting",
    pill: choice ? { tone: "live", label: "Your decision" } : null,
    trail: [],
    help: [],
    blocks: !choice
      ? [{ kind: "note", tone: local.error ? "accent" : "quiet", text: idle }]
      : onBoard
        ? // The board is where a system is chosen. The panel has the choice, not the candidates.
          [
            {
              kind: "note",
              tone: staged ? "accent" : "quiet",
              strong: staged ? `System ${staged.id}` : undefined,
              text: staged
                ? "Selected on the board."
                : `${choice.options.length} systems can be chosen on the board.`,
            },
          ]
        : [...(card ? [{ kind: "source" as const, ...card }] : []), { kind: "menu", rows }],
    footer: {
      note: local.planNote
        ? local.planNote.text
        : !choice
          ? ""
          : staged
            ? `Selected: ${sentence(staged.label || staged.id)}`
            : `Choose one of ${choice.options.length}.`,
      error: !!local.planNote?.error,
      actions: [
        ...(stepping
          ? [
              {
                label: "Step",
                tone: "primary" as const,
                key: "Enter",
                intent: { type: "simulate" as const },
              },
            ]
          : []),
        ...(local.canUndo && !local.replaying
          ? [
              {
                label: "Undo",
                tone: "quiet" as const,
                intent: { type: "undo" as const },
              },
            ]
          : []),
        ...(choice
          ? [
              ...(staged
                ? [
                    {
                      label: "Clear",
                      tone: "quiet" as const,
                      key: "Escape",
                      intent: { type: "clearChoice" as const },
                    },
                  ]
                : []),
              {
                label: "Send",
                tone: "primary" as const,
                disabled: !staged,
                key: "Enter",
                intent: { type: "flow" as const, action: "resolve" as const },
              },
            ]
          : []),
      ],
    },
  };
}
