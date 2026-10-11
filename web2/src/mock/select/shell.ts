// The whole screen: toolbar, player table, board, action panel, reference drawers.
import type {
  ActionCardView,
  ComponentCardView,
  ExplorationDeck,
  ExplorationDeckView,
  ListSectionView,
  LogEntryView,
  PlayerTableView,
  ReferenceView,
  Rich,
  ShellView,
  ToolbarView,
} from "../../model";
import { ACTION_CARDS, CARDS, DISCARDED_CARDS, HISTORY, REACTION_CARDS, SEATS } from "../data";
import { E } from "../loose";
import { OBJECTIVES, strategyEditor } from "../strategy";
import {
  currentState,
  decisionShape,
  pastState,
  seatReason,
  type State,
  type World,
} from "../world";
import { selectBoard } from "./board";
import { selectFlow } from "./flows";
import { isTactical, seatViews, stagesOf, statusText, summaryOf } from "./shared";
import { applyItems, selectTactical } from "./tactical";

const actor = (state: State): string => state.flow?.owner || "sol";

/** Who holds each strategy card, by index (card number − 1). Free cards have no owner. */
function cardOwners(world: World, state: State) {
  const table = world.table;
  if (table.cards) {
    return CARDS.map((_, index) => ({
      owner: table.seats.find((seat) => table.cards![seat]?.includes(index + 1)) ?? "",
      used: world.usedCards.includes(index + 1),
      tg: 0,
    }));
  }
  const owners = (
    [
      ["sol"],
      ["xxcha", true],
      ["letnev"],
      ["jolnar", true],
      ["hacan"],
      ["mentak", true],
      ["yssaril", true],
      ["naalu"],
    ] as [string, boolean?, number?][]
  ).map(([owner, used, tg]) => ({ owner, used: !!used, tg: tg ?? 0 }));
  if (state.kind === "strategic") {
    // The open card is with the player who plays it; that player's card goes to the other seat.
    const at = (state.flow.number ?? 1) - 1;
    const other = owners.findIndex((item) => item.owner === state.flow.owner);
    [owners[at].owner, owners[other].owner] = [owners[other].owner, owners[at].owner];
    owners[other].used = owners[at].used;
    owners[at].used = true;
  }
  for (const number of world.usedCards) {
    owners[number - 1].used = true;
  }
  return owners;
}

function toolbar(world: World, state: State): ToolbarView {
  const draft = state.mode === "draft" && world.mode === "draft";
  return {
    round: 3,
    phase: "Action phase",
    status: statusText(world, state),
    workspace: world.mode,
    draftLocked: world.viewer !== "sol" ? "A draft is private to the acting player" : null,
    past: world.workspaces.history ? { label: world.workspaces.history.past.label } : null,
    draft: draft
      ? {
          canUndo: state.cursor > 0,
          canRedo: state.cursor < state.history.length - 1,
          canApply: E.canApply(state) && world.viewer === "sol" && !E.pendingEdit(state),
          applyHint: E.applyReadiness(state),
        }
      : null,
  };
}

/** The relic fragments of the demo. */
/** A card of the demo: no timing, no state, nothing to unlock. */
const CARD: ComponentCardView = {
  name: "",
  kind: "",
  window: "",
  text: "",
  state: null,
  unlock: "",
};
const SHARD =
  "When you gain this card or lose this card, gain 1 victory point or lose 1 victory point.";
const FRAGMENTS: Record<string, { deck: ExplorationDeck; count: number }[]> = {
  sol: [
    { deck: "cultural", count: 2 },
    { deck: "hazardous", count: 1 },
  ],
  hacan: [{ deck: "industrial", count: 1 }],
};

const card = (
  name: string,
  kind: string,
  text: string,
  left: number,
  total: number,
  drawn: string[] = [],
) => ({ name, kind, text, left, total, drawn });
/** Two decks of the demo, short: the sheet is the same for every deck. */
const EXPLORATION: Omit<ExplorationDeckView, "fragments">[] = [
  {
    deck: "cultural",
    title: "Cultural",
    left: 5,
    total: 8,
    cards: [
      card(
        "Cultural Relic Fragment",
        "Fragment",
        "ACTION: Purge 3 of your cultural relic fragments to gain 1 relic.",
        2,
        4,
        ["Round 1 · Sol · Lodor", "Round 2 · Sol · Corneeq"],
      ),
      card(
        "Dyson Sphere",
        "Attach",
        "This planet's resource value is increased by 2 and its influence value is increased by 1.",
        0,
        1,
        ["Round 2 · Sol · Quann"],
      ),
      card(
        "Freelancers",
        "Instant",
        "You may produce 1 unit in this system; you may spend influence as if it were resources to produce this unit.",
        2,
        2,
      ),
      card("Paradise World", "Attach", "This planet's influence value is increased by 2.", 1, 1),
    ],
  },
  {
    deck: "hazardous",
    title: "Hazardous",
    left: 4,
    total: 6,
    cards: [
      card(
        "Core Mine",
        "Instant",
        "If you have at least 1 mech on this planet, or if you remove 1 infantry from this planet, gain 1 trade good.",
        2,
        2,
      ),
      card(
        "Hazardous Relic Fragment",
        "Fragment",
        "ACTION: Purge 3 of your hazardous relic fragments to gain 1 relic.",
        2,
        3,
        ["Round 2 · Sol · Starpoint"],
      ),
      card(
        "Warfare Research Facility",
        "Attach",
        "This planet has a red technology specialty. If this planet already has a technology specialty, this planet's resource and influence values are each increased by 1 instead.",
        0,
        1,
        ["Round 1 · Sol · Vefut"],
      ),
    ],
  },
];

/** One row for each player, in seat order. Rows do not move. */
function players(world: World, state: State): PlayerTableView {
  const owners = cardOwners(world, state);
  const seats = world.table.seats;
  const now = world.mode === "history" ? null : actor(state);
  // The lowest card of a seat gives its place in the initiative order.
  const card = (seat: string) => owners.findIndex((item) => item.owner === seat);
  const queue = seats
    .filter((seat) => !SEATS[seat].passed && card(seat) >= 0)
    .sort((a, b) => card(a) - card(b));
  const next = now && queue.length > 1 ? queue[(queue.indexOf(now) + 1) % queue.length] : null;
  const me = world.viewer === "observer" ? "sol" : world.viewer;
  const picking = world.mode === "live" && decisionShape(state) === "seat";
  return {
    pick:
      world.mode === "live" && state.kind === "strategic" && E.flowEditing(state)
        ? strategyEditor(state, seats).pick
        : picking
          ? {
              verb: "Choose",
              seats: Object.fromEntries(
                seats.map((seat) => [seat, { reason: seatReason(world, seat) }]),
              ),
              chosen: state.flow.chosen ? [state.flow.chosen] : [],
            }
          : null,
    rows: seats.map((seat) => {
      const player = SEATS[seat];
      const tokens =
        seat === "sol" && isTactical(state) && state.mode === "live" && state.done[0]
          ? [player.tokens[0] - 1, ...player.tokens.slice(1)]
          : player.tokens;
      return {
        seat,
        isMe: seat === me,
        turn: now === seat ? "now" : next === seat ? "next" : null,
        speaker: !!player.speaker,
        passed: !!player.passed,
        victoryPoints: player.vp,
        strategyCards: owners.flatMap((item, index) =>
          item.owner === seat ? [{ number: index + 1, name: CARDS[index], used: item.used }] : [],
        ),
        resources: player.res,
        influence: player.inf,
        tradeGoods: player.tg,
        commodities: player.comm,
        tokens,
        actionCards: player.ac,
        secrets: player.so,
        planets: player.planets,
        abilities: (player.abilities as string[]).map((line) => {
          const [name, text] = line.split(": ");
          return { ...CARD, name, kind: "Faction ability", text };
        }),
        technologies: player.tech,
        // "Agent ready · Commander unlocked · Hero locked"
        leaders: (player.leaders as string).split(" · ").map((leader) => {
          const [kind, state] = leader.split(" ");
          return {
            ...CARD,
            name: kind,
            kind,
            state: state === "unlocked" ? "ready" : (state as ComponentCardView["state"]),
          };
        }),
        components:
          seat === "hacan"
            ? [{ ...CARD, name: "Shard of the Throne", kind: "Relic", text: SHARD }]
            : [],
        fragments: FRAGMENTS[seat] ?? [],
      };
    }),
    freeCards: owners.flatMap((item, index) =>
      item.owner ? [] : [{ number: index + 1, name: CARDS[index], tradeGoods: item.tg }],
    ),
  };
}

function reference(world: World, state: State): ReferenceView {
  // The hand of the viewer. The demo has hands for the two sides of the battle only.
  const hand: string[] = state.hands[world.viewer] ?? [];
  const sym = (seat: string) => ({ seat });
  const section = (title: string, rows: (Rich | string)[]): ListSectionView => ({
    title,
    rows: rows.map((row) => (typeof row === "string" ? [row] : row)),
  });
  // A card of a reaction window is printed with its timing first.
  const printed = (name: string): ActionCardView => {
    const reaction = REACTION_CARDS[name];
    const [window, ...rest] = (reaction ?? "").split(": ");
    return reaction
      ? { name, window: `${window}:`, text: rest.join(": "), action: false }
      : { name, window: "ACTION:", text: ACTION_CARDS[name] ?? "", action: true };
  };
  const held = [...(E.own() ? world.table.actionCards : []), ...hand];
  const count = (cards: string[], name: string) => cards.filter((card) => card === name).length;
  const names = [...Object.keys(REACTION_CARDS), ...Object.keys(ACTION_CARDS)].sort();
  const copies = (name: string) => (REACTION_CARDS[name] ? 4 : 1);
  const total = names.reduce((sum, name) => sum + copies(name), 0);
  const live = world.workspaces.live;
  const seats = world.table.seats;
  const entry = (item: (typeof HISTORY)[number]): LogEntryView => {
    const past = pastState(item);
    return {
      id: item.id,
      seat: item.seat,
      type: `${item.type}${isTactical(past) ? "" : " · " + past.flow.card}`,
      summary: summaryOf(past),
      stages: stagesOf(past).map(([name, text]) => ({ name, text })),
      current: false,
    };
  };
  const current: LogEntryView[] =
    live.kind !== "picker" && live.kind !== "waiting"
      ? [
          {
            id: "current",
            seat: actor(live),
            type: `${isTactical(live) ? "Tactical" : live.kind === "strategic" ? "Strategic" : "Component"} · in progress`,
            summary: summaryOf(live) || "Not started",
            stages: [],
            current: true,
          },
        ]
      : [];
  return {
    objectives: [
      section(
        "Public objectives · stage I",
        OBJECTIVES.map((item): Rich => [
          `${item.name} · ${item.text} · ${item.scored.length ? "scored by " : "not scored"}`,
          ...item.scored.flatMap((seat) => [sym(seat), " "]),
        ]),
      ),
      section("Your secret objectives", [
        "Destroy Their Greatest Ship · scored",
        "Occupy the Seat of the Empire · not scored",
      ]),
      section(
        "Points",
        seats.map((id) => [
          sym(id),
          ` ${SEATS[id].name} · ${SEATS[id].vp} of 10 · ${SEATS[id].so[0]} from secrets`,
        ]),
      ),
    ],
    technology: seats.map((id) =>
      section(`${SEATS[id].name} · ${SEATS[id].faction}`, SEATS[id].tech),
    ),
    cards: [
      section(
        "Strategy cards",
        cardOwners(world, state).map((card, index): Rich =>
          card.owner
            ? [
                `${index + 1} ${CARDS[index]} · `,
                sym(card.owner),
                ` ${SEATS[card.owner].name}${card.used ? " · used" : ""}`,
              ]
            : [`${index + 1} ${CARDS[index]} · unclaimed · ${card.tg} TG`],
        ),
      ),
      section("Promissory notes", [
        "Trade Agreement (Hacan) · held by you",
        "Ceasefire (Sol) · held by Blair",
      ]),
    ],
    actionCards: {
      held: held.length,
      limit: 7,
      deck: {
        left: total - DISCARDED_CARDS.length - held.length - 5,
        total,
        discarded: DISCARDED_CARDS.length,
      },
      hand: [...new Set(held)].sort().map((name) => ({
        ...printed(name),
        copies: count(held, name),
        never: printed(name).action ? null : world.neverOffer.includes(name),
      })),
      faked: world.fakedCards.filter((name) => !held.includes(name)).map(printed),
      reference: names.map((name) => ({
        ...printed(name),
        total: copies(name),
        discarded: count(DISCARDED_CARDS, name),
        held: count(held, name),
        open: Math.max(0, copies(name) - count(DISCARDED_CARDS, name) - count(held, name)),
        faked: !held.includes(name) && world.fakedCards.includes(name),
      })),
    },
    exploration: EXPLORATION.map((deck) => ({
      ...deck,
      fragments: seats.flatMap((seat) =>
        (FRAGMENTS[seat] ?? [])
          .filter((item) => item.deck === deck.deck)
          .map((item) => ({ seat, count: item.count })),
      ),
    })),
    log: [3, 2].map((round) => ({
      round,
      current: round === 3,
      label: `Round ${round}${round === 3 ? " · Action phase" : " · complete"}`,
      entries: [
        ...(round === 3 ? current : []),
        ...HISTORY.filter((item) => item.round === round && seats.includes(item.seat)).map(entry),
      ],
    })),
  };
}

export function selectShell(world: World): ShellView {
  E.env.viewer = world.viewer;
  E.env.neverOffer = world.neverOffer;
  const state = currentState(world);
  const tactical = isTactical(state);
  const canApply =
    world.mode === "draft" &&
    tactical &&
    state.mode === "draft" &&
    E.canApply(state) &&
    !E.pendingEdit(state);
  return {
    seats: seatViews,
    accent:
      state.mode === "draft" ||
      E.flowStaged(state) ||
      E.decisionStaged(state) ||
      E.reactionPick(state)
        ? "draft"
        : state.frontier === null
          ? "done"
          : "live",
    toolbar: toolbar(world, state),
    players: players(world, state),
    board: selectBoard(world, state),
    action: tactical ? selectTactical(world, state) : selectFlow(world, state),
    reference: reference(world, state),
    apply: canApply
      ? {
          items: applyItems(state),
          cost: "Costs 1 tactic token.",
          note:
            state.frontier === null
              ? "The whole action is prepared."
              : "Dice and later decisions continue in Live.",
        }
      : null,
    reveal: state.reveal ?? null,
    toast: world.toast,
    announcement: world.announcement,
  };
}
