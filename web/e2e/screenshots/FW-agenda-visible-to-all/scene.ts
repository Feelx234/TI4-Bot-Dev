import type { MockChoice, MockGameOptions } from "../_shared/mockGame";
import { actor, galleryBoard, galleryPlayers } from "../_shared/fixtures";
import { playerWithHand } from "../_shared/players";

export const SIZES = { desktop: { width: 1440, height: 900 }, "390x844": { width: 390, height: 844 } } as const;

/** Which side of the change a run is. The `before` shots are taken on the commit that precedes it. */
export const prefix = process.env.TI4_SHOT_PREFIX ?? "after";

const other = galleryPlayers[1];
const clone = (id: string, faction: string) => ({ ...other, id, faction });
/** Four seats at the table; the viewer (`actor`) is seat 1, Bob speaks, Cy and Dee vote after the viewer. */
export const players = [
  playerWithHand({ victory_points: 2 }),
  { ...other, id: "bob" },
  clone("cy", "arborec"),
  clone("dee", "letnev"),
];
export const nicknames = { [actor]: "Ada", bob: "Bob", cy: "Cy", dee: "Dee" };

/** The state fields the engine sends for the agenda in play; everything here is public. */
export const inPlay = (agenda: string, extra: Record<string, unknown> = {}) => ({
  agenda_in_play: agenda,
  agenda_choices: agenda === "minister_commerce" ? ["ada", "bob", "cy", "dee"] : ["FOR", "AGAINST"],
  ...extra,
});

const cast = (player: string, outcome: string | null, votes: number) => ({ player, outcome, votes });

export interface Variant {
  name: string;
  /** What the viewer is looking at, for the manifest. */
  options: MockGameOptions;
  /** Texts the banner must show (in the opened card). */
  expect: string[];
  /** The banner is not there at all. */
  gone?: boolean;
}

const base = (extra: Partial<MockGameOptions>): MockGameOptions => ({
  phase: "agenda",
  round: 3,
  board: galleryBoard,
  players,
  nicknames,
  view: { speaker: "bob" } as never,
  ...extra,
});

const voteChoice: MockChoice = {
  prompt: "vote for which outcome",
  context: { subtype: "cast_vote" },
  details: {
    agenda: {
      id: "conventions",
      name: "Conventions of War",
      type: "Law",
      target: "For/Against",
      text1: "For: Players cannot use BOMBARDMENT against units that are on cultural planets.",
      text2: 'Against: Each player that voted "Against" discards all of their action cards.',
    },
  },
  options: [
    { id: "FOR", label: "FOR", kind: "vote", payload: { current_votes: 4 } },
    { id: "AGAINST", label: "AGAINST", kind: "vote", payload: { current_votes: 0 } },
    { id: "decline", label: "Abstain", kind: "decline" },
  ],
};

export const VARIANTS: Variant[] = [
  {
    name: "1-voter",
    options: base({ choice: voteChoice, state: inPlay("conventions", { agenda_casts: [cast("cy", "FOR", 4)] }) }),
    expect: ["Conventions of War", "Cy For: 4 votes", "Running total: FOR 4"],
  },
  {
    // Not asked: another seat votes; no decision, nothing but the public state to go on.
    name: "2-other-seat-votes",
    options: base({
      state: inPlay("conventions", { agenda_casts: [cast("cy", "FOR", 4), cast("dee", "AGAINST", 2)] }),
      turnStatus: { kind: "waiting_for_decision", seat: actor, phase: "agenda", round: 3, stage: "Waiting for player" },
    }),
    expect: ["Conventions of War", "Cy For: 4 votes", "Dee Against: 2 votes", "Running total: For 4 · Against 2", "Still to vote: You, Bob"],
  },
  {
    name: "3-spectator",
    options: base({
      spectator: true,
      state: inPlay("conventions", { agenda_casts: [cast("cy", "FOR", 4)] }),
      turnStatus: { kind: "waiting_for_decision", seat: "dee", phase: "agenda", round: 3, stage: "Waiting for player" },
    }),
    expect: ["Conventions of War", "Cy For: 4 votes", "Still to vote: Dee, Ada, Bob"],
  },
  {
    // The reaction window right after the reveal: this seat has nothing to play and sees nothing asked.
    name: "4-reaction-window",
    options: base({
      state: inPlay("conventions", { agenda_predictions: { cy: "AGAINST" } }),
      turnStatus: { kind: "waiting_for_reactions", phase: "agenda", round: 3, stage: "Waiting for reactions" } as never,
    }),
    expect: ["Conventions of War", "Riders: Cy predicts Against", "Still to vote: Dee, You, Bob"],
  },
  {
    name: "5-second-agenda",
    options: base({
      state: inPlay("minister_commerce", {
        agenda_log: [{ round: 3, agenda: "conventions", outcome: "for", law: true, enacted: true }],
        agenda_casts: [cast("cy", "bob", 3)],
      }),
      turnStatus: { kind: "waiting_for_decision", seat: "dee", phase: "agenda", round: 3, stage: "Waiting for player" },
    }),
    expect: ["Agenda 2/2", "Minister of Commerce", "Elect Player"],
  },
  {
    name: "6-after-resolution",
    options: base({
      state: {
        agenda_log: [{ round: 3, agenda: "conventions", outcome: "for", law: true, enacted: true }],
        agenda_casts: [cast("cy", "FOR", 4)],
        laws: { conventions: "for" },
      },
      table: { laws: { conventions: "for" } } as never,
      turnStatus: { kind: "waiting_for_decision", seat: "bob", phase: "agenda", round: 3, stage: "Waiting for player" },
    }),
    expect: [],
    gone: true,
  },
];

/** Six seats with the engine's real, 64-hex ids: Elect Player must never print one. */
const hexId = (n: number) => `player_${n.toString(16).padStart(2, "0").repeat(32)}`;
const six = [0, 1, 2, 3, 4, 5].map((i) => (i === 0 ? { ...players[0], id: hexId(1) } : { ...other, id: hexId(i + 1), faction: ["hacan", "arborec", "letnev", "sol", "yssaril"][i - 1] }));
const sixNames = ["Felix", "Ada", "Bob", "Cy", "Dee", "Eve"];
export const SIX: Variant = {
  name: "7-six-players-elect-player",
  options: {
    ...base({}),
    seat: hexId(1),
    players: six,
    nicknames: Object.fromEntries(six.map((p, i) => [p.id, sixNames[i]])),
    view: { speaker: hexId(1) } as never,
    choice: {
      prompt: "vote for which outcome",
      context: { subtype: "cast_vote" },
      details: { agenda: { id: "minister_commerce", name: "Minister of Commerce", type: "Law", target: "Elect Player", text1: "The elected player gains this card." } },
      options: [
        ...six.map((p) => ({ id: p.id, label: p.id, kind: "vote", payload: { current_votes: 1 } })),
        { id: "decline", label: "Abstain", kind: "decline" },
      ],
    },
    state: inPlay("minister_commerce", {
      agenda_choices: six.map((p) => p.id),
      agenda_log: [{ round: 3, agenda: "conventions", outcome: "for", law: true, enacted: true }],
      agenda_casts: [cast(hexId(2), hexId(3), 5), cast(hexId(3), hexId(1), 2)],
    }),
  },
  expect: ["Agenda 2/2", "Elect Player", "Bob", "Cy"],
};
VARIANTS.push(SIX);
