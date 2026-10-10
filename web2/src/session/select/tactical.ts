// The activation, the movement and the landings of a tactical action, on the screens of these
// steps. The other steps of the action are still decisions in the list (`selectAction`).
import type {
  ActionButtonView,
  ActivationView,
  FooterView,
  StepContentView,
  StepTabView,
  TacticalActionView,
} from "../../model";
import { plural } from "../../model";
import { landed } from "../landingDraft";
import type {
  ActivationFacts,
  InvasionFacts,
  MovementFacts,
  SessionUpdate,
  TacticalFacts,
} from "../wire";
import { type LocalState, openChoice } from "./action";
import { selectInvasion } from "./invasion";
import { selectMovement, systemLabel } from "./movement";
import { ANOMALIES, WORMHOLES, factionName } from "./names";

const STEPS = ["Activation", "Movement", "Space combat", "Invasion", "Production"];

/** The facts of the open choice, when it is the activation, the movement or the landings of the viewer. */
export function tacticalFacts(update: SessionUpdate, local: LocalState): TacticalFacts | null {
  const subtype = openChoice(update, local)?.context?.subtype;
  const facts = update.tactical;
  return (facts?.kind === "activation" && subtype === "activate_system") ||
    (facts?.kind === "movement" && subtype === "movement_step") ||
    (facts?.kind === "invasion" && subtype === "commit_ground_forces")
    ? facts
    : null;
}

const tilesOf = (update: SessionUpdate) =>
  new Map((update.view.board.map_tiles ?? []).map((tile) => [tile.system_id, tile]));

const UNDO: ActionButtonView = { label: "Undo", tone: "quiet", intent: { type: "undo" } };

function activation(
  update: SessionUpdate,
  local: LocalState,
  facts: ActivationFacts,
): { content: ActivationView; footer: FooterView; target: string | null } {
  const tiles = tilesOf(update);
  const reach = facts.systems.find((item) => item.system === local.staged);
  const tile = reach && tiles.get(reach.system);
  const label = reach ? systemLabel(tiles, reach.system) : null;
  return {
    target: label,
    content: {
      kind: "activation",
      editing: true,
      draft: !!local.draft,
      tacticPool: [facts.tactic_tokens, facts.tactic_tokens - 1],
      systems: facts.systems.map((item) => ({
        id: item.system,
        label: systemLabel(tiles, item.system),
      })),
      active: null,
      chosen: reach
        ? {
            system: reach.system,
            label: label!,
            facts:
              [
                ...(tile?.anomalies ?? []).map((name) =>
                  name in ANOMALIES ? name.charAt(0).toUpperCase() + name.slice(1) : name,
                ),
                ...(tile?.wormholes ?? []).map((name) => `${WORMHOLES[name] ?? name} wormhole`),
                plural(tile?.planets?.length ?? 0, "planet"),
              ].join(" · ") || "Empty space",
            commandToken: false,
            shipsInRange: reach.ships,
            originsInRange: reach.origins,
            production: reach.production ?? 0,
            cannon: reach.cannon ?? 0,
          }
        : null,
    },
    footer: {
      note: label ? `Selected: ${label}` : "Choose a system on the map.",
      error: false,
      actions: [
        ...(local.canUndo ? [UNDO] : []),
        ...(reach
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
          label: "Activate system",
          tone: "primary",
          disabled: !reach,
          key: "Enter",
          arrow: true,
          intent: { type: "commitEdit" },
        },
      ],
    },
  };
}

function movement(
  update: SessionUpdate,
  local: LocalState,
  facts: MovementFacts,
): { content: StepContentView; footer: FooterView; target: string } {
  const tiles = tilesOf(update);
  const target = systemLabel(tiles, facts.active);
  if (local.step === 0) {
    // The recorded activation, read-only. The token of this activation is already spent.
    const pool =
      update.view.players.find((player) => player.id === update.pending_choice?.choice.player)
        ?.tactic_tokens ?? 0;
    return {
      target,
      content: {
        kind: "activation",
        editing: false,
        draft: !!local.draft,
        chosen: null,
        systems: [],
        active: { system: facts.active, name: tiles.get(facts.active)?.label ?? "System" },
        tacticPool: [pool + 1, pool],
      },
      footer: {
        note: local.draft
          ? "Viewing activation. Undo takes it back."
          : "Viewing activation. The action is at movement.",
        error: false,
        actions: [
          {
            label: "Continue to movement",
            tone: "primary",
            key: "Enter",
            arrow: true,
            intent: { type: "returnToCurrent" },
          },
        ],
      },
    };
  }
  const nothing = Object.keys(local.movement).length === 0;
  const content = selectMovement(update, facts, local.movement, local.handled);
  if (local.draft) {
    // A draft records every change of the movement at once: there is nothing to send.
    return {
      target,
      content,
      footer: {
        note: local.planNote?.text ?? "Private to you until you apply.",
        error: !!local.planNote?.error,
        actions: [],
      },
    };
  }
  return {
    target,
    content,
    footer: {
      note: local.planNote?.text ?? "This commits to the live game.",
      error: !!local.planNote?.error,
      actions: [
        ...(local.canUndo ? [UNDO] : []),
        ...(nothing
          ? []
          : [
              {
                label: "Reset selection",
                key: "Escape",
                intent: { type: "cancelEdit" as const },
              },
            ]),
        {
          label: nothing ? "Move nothing" : "Move fleet",
          tone: "primary",
          key: "Enter",
          arrow: true,
          intent: { type: "commitEdit" },
        },
      ],
    },
  };
}

function invasion(
  update: SessionUpdate,
  local: LocalState,
  facts: InvasionFacts,
): { content: StepContentView; footer: FooterView; target: string } {
  const nothing = landed(local.landing) === 0;
  return {
    target: systemLabel(tilesOf(update), facts.active),
    content: selectInvasion(update, facts, local.landing, local.landingKind),
    footer: {
      note: local.planNote?.text ?? "This commits to the live game.",
      error: !!local.planNote?.error,
      actions: [
        ...(local.canUndo ? [UNDO] : []),
        ...(nothing
          ? []
          : [
              {
                label: "Reset selection",
                key: "Escape",
                intent: { type: "cancelEdit" as const },
              },
            ]),
        {
          label: nothing ? "Land nothing" : "Commit ground forces",
          tone: "primary",
          key: "Enter",
          arrow: true,
          intent: { type: "commitEdit" },
        },
      ],
    },
  };
}

const CURRENT = { activation: 0, movement: 1, invasion: 3 };

export function selectTactical(
  update: SessionUpdate,
  local: LocalState,
  facts: TacticalFacts,
): TacticalActionView {
  const current = CURRENT[facts.kind];
  const selected = facts.kind === "movement" && local.step === 0 ? 0 : current;
  const view =
    facts.kind === "activation"
      ? activation(update, local, facts)
      : facts.kind === "movement"
        ? movement(update, local, facts)
        : invasion(update, local, facts);
  const units = landed(local.landing);
  const seat = update.pending_choice?.choice.player ?? "";
  const faction = factionName(
    update.view.players.find((player) => player.id === seat)?.faction ?? seat,
  );
  const ships = Object.keys(local.movement).length;
  const tabs = STEPS.map((name, step): StepTabView => {
    const status = step < current ? "done" : step === current ? "decision" : "future";
    return {
      name,
      status,
      caption:
        step === 0 && facts.kind !== "activation"
          ? `#${facts.active}`
          : step === 1 && current === 1 && ships
            ? plural(ships, "ship")
            : step === 3 && current === 3 && units
              ? plural(units, "unit")
              : "",
    };
  });
  return {
    kind: "tactical",
    title: "Tactical action",
    subtitle: `${view.target ?? "No system chosen"} · ${faction}`,
    badge: local.draft
      ? { tone: "draft", label: "Private draft" }
      : { tone: "live", label: "In progress" },
    past: null,
    tabs,
    selected,
    current,
    contentKey: `tactical:${selected}`,
    uncommitted: null,
    interrupt: null,
    closing: [],
    task: {
      step: selected,
      title: STEPS[selected],
      pill: local.draft
        ? selected !== current
          ? { tone: "done", label: "Drafted" }
          : { tone: "draft", label: "Preparing" }
        : selected !== current
          ? { tone: "done", label: "Resolved" }
          : local.remaining === null && local.planNote && !local.planNote.error
            ? {
                tone: "live",
                label: `Your decision · ${facts.kind === "invasion" ? "landings go" : "movement goes"} on`,
              }
            : { tone: "live", label: "Your decision" },
      trail: [],
      edit: null,
      help: [],
      content: view.content,
      footer: view.footer,
    },
  };
}
