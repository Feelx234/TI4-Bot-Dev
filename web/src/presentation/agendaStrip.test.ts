import { describe, expect, it } from "vitest";
import type { PendingChoiceDto } from "../protocol/types.ts";
import {
  advanceAgendaMemory,
  agendaInfoFromTable,
  EMPTY_AGENDA_TABLE,
  mergeAgendaInfo,
  readAgendaTable,
  votingOrder,
  agendaInfoFromChoice,
  agendaNameFromId,
  agendaPillLabel,
  agendaStripText,
  deriveAgendaStrip,
  EMPTY_AGENDA_MEMORY,
  isAgendaDecision,
  type AgendaMemory,
} from "./agendaStrip.ts";
import { agendaOutcomeLabel } from "./agendaToasts.ts";
import { partialContext } from "../test/wire.ts";

const PEACE = {
  id: "minister_of_peace",
  name: "Minister of Peace",
  type: "Law",
  target: "For/Against",
  text1: "For: something.",
  text2: "Against: nothing.",
};
const COMMERCE = { ...PEACE, id: "minister_of_commerce", name: "Minister of Commerce", target: "Elect Player" };

const choice = (
  subtype: string,
  actor: string,
  extra: Partial<PendingChoiceDto> = {},
  options: PendingChoiceDto["options"] = [
    { id: "FOR", label: "FOR", kind: "vote", payload: { current_votes: 3 } },
    { id: "AGAINST", label: "AGAINST", kind: "vote", payload: { current_votes: 1 } },
    { id: "decline", label: "Abstain", kind: "decline", payload: {} },
  ],
): PendingChoiceDto =>
  ({ nonce: "n", actor, prompt: "p", context: partialContext({ subtype }), options, ...extra }) as PendingChoiceDto;

const label = (seat: string) => (seat === "p1" ? "You" : seat === "p2" ? "Alice" : seat);
const strip = (c: PendingChoiceDto | null, memory: AgendaMemory = EMPTY_AGENDA_MEMORY, extra = {}) =>
  deriveAgendaStrip({ phase: "agenda", choice: c, memory, viewerSeat: "p1", speaker: "p2", label, ...extra });

describe("agendaInfoFromChoice", () => {
  it("reads the printed agenda from the decision's details", () => {
    const info = agendaInfoFromChoice(choice("cast_vote", "p1", { details: { agenda: PEACE } }));
    expect(info).toMatchObject({ id: "minister_of_peace", name: "Minister of Peace", target: "For/Against" });
    expect(info?.text).toEqual(["For: something.", "Against: nothing."]);
  });
  it("reads the old fixture shape and a reaction trigger's agenda id", () => {
    const fixture = choice("cast_vote", "p1", {
      details: { agenda_card: { name: "Sling Relay", yes_outcome: "Y", no_outcome: "N" } },
    });
    expect(agendaInfoFromChoice(fixture)?.name).toBe("Sling Relay");
    expect(agendaInfoFromChoice(fixture)?.text).toEqual(["For: Y", "Against: N"]);
    const reaction = choice("reaction_when_AGENDA_REVEALED", "p1", {
      context: partialContext({ subtype: "reaction_when_AGENDA_REVEALED", trigger: { kind: "agenda_revealed", event_type: "X", event_id: 1, relation: "when", agenda: "minister_of_peace" } }),
    });
    expect(agendaInfoFromChoice(reaction)?.name).toBe("Minister of Peace");
  });
  it("is null for a decision that names no agenda", () => {
    expect(agendaInfoFromChoice(choice("predict_agenda_outcome", "p1"))).toBeNull();
    expect(agendaInfoFromChoice(null)).toBeNull();
  });
  it("title-cases an alias without capitalising small words", () => {
    expect(agendaNameFromId("minister_of_peace")).toBe("Minister of Peace");
    expect(agendaNameFromId("the_crown_of_emphidia")).toBe("The Crown of Emphidia");
  });
});

describe("the strip in each agenda sub-state", () => {
  it("reveal and reactions (riders, when/after cards): the trigger names the agenda", () => {
    const reaction = choice("reaction_when_AGENDA_REVEALED", "p1", {
      context: partialContext({ subtype: "reaction_when_AGENDA_REVEALED", trigger: { kind: "agenda_revealed", event_type: "X", event_id: 1, relation: "when", agenda: "minister_of_peace" } }),
      options: [{ id: "decline", label: "Pass", kind: "decline", payload: {} }],
    });
    expect(isAgendaDecision(reaction)).toBe(true);
    const memory = advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info: agendaInfoFromChoice(reaction), decision: true });
    const s = strip(reaction, memory)!;
    expect(agendaStripText(s)).toBe("Agenda 1/2: Minister of Peace · Your reaction");
    expect(agendaPillLabel(s, reaction)).toBe("Agenda: Minister of Peace");
  });

  it("my vote (outcome choice): number, name, For/Against, tally", () => {
    const c = choice("cast_vote", "p1", { details: { agenda: PEACE } });
    const memory = advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info: agendaInfoFromChoice(c) });
    const s = strip(c, memory)!;
    expect(agendaStripText(s)).toBe("Agenda 1/2: Minister of Peace · For/Against · Your vote");
    expect(s.tally).toBe("FOR 3 · AGAINST 1");
    expect(s.outcomes).toEqual(["FOR", "AGAINST"]);
    expect(s.speaker).toBe("Alice");
    expect(agendaPillLabel(s, c)).toBe("Voting: Minister of Peace — For/Against");
  });

  it("my planet selection and trade goods keep the agenda and list no outcomes", () => {
    const c = choice(
      "vote_exhaust_planet",
      "p1",
      { details: { agenda: PEACE } },
      [{ id: "jord", label: "exhaust Jord for 2 votes", kind: "vote_planet", payload: { votes: 2, planet_name: "Jord" } }],
    );
    const s = strip(c)!;
    expect(agendaStripText(s)).toBe("Agenda: Minister of Peace · For/Against · Your vote");
    expect(s.outcomes).toEqual([]);
    expect(isAgendaDecision(choice("vote_spend_trade_goods", "p1"))).toBe(true);
    expect(isAgendaDecision(choice("vote_predictive_intelligence", "p1"))).toBe(true);
  });

  it("a rider's prediction names no agenda: the phase memory does", () => {
    const vote = choice("cast_vote", "p2", { details: { agenda: PEACE } });
    const memory = advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info: agendaInfoFromChoice(vote) });
    const predict = choice("predict_agenda_outcome", "p1", {}, [
      { id: "FOR", label: "predict FOR", kind: "prediction", payload: {} },
      { id: "AGAINST", label: "predict AGAINST", kind: "prediction", payload: {} },
    ]);
    const s = strip(predict, memory)!;
    expect(agendaStripText(s)).toBe("Agenda 1/2: Minister of Peace · For/Against · Your prediction");
    expect(s.outcomes).toEqual(["FOR", "AGAINST"]);
    expect(agendaPillLabel(s, predict)).toBe("Predict: Minister of Peace — For/Against");
  });

  it("another seat's vote while I wait", () => {
    const c = choice("cast_vote", "p2", { details: { agenda: PEACE } });
    expect(agendaStripText(strip(c)!)).toContain("Alice's vote");
  });

  it("the speaker's tie-break names the speaker, not the actor field alone", () => {
    const c = choice("vote_tiebreak", "p2", { details: { agenda: PEACE } }, [
      { id: "FOR", label: "FOR", kind: "tiebreak", payload: {} },
      { id: "AGAINST", label: "AGAINST", kind: "tiebreak", payload: {} },
    ]);
    const s = strip(c)!;
    expect(s.whose?.text).toBe("Alice breaks the tie");
    expect(agendaPillLabel(s, c)).toBe("Tie-break: Minister of Peace — For/Against");
    expect(strip(c, EMPTY_AGENDA_MEMORY, { speaker: "p1" })!.whose?.text).toBe("You break the tie");
  });

  it("an elect agenda says Elect Player and has no For/Against tally words", () => {
    const c = choice("cast_vote", "p1", { details: { agenda: COMMERCE } }, [
      { id: "p1", label: "p1", kind: "vote", payload: { current_votes: 0 } },
      { id: "p2", label: "p2", kind: "vote", payload: { current_votes: 2 } },
      { id: "decline", label: "Abstain", kind: "decline", payload: {} },
    ]);
    const s = strip(c)!;
    expect(s.kind).toBe("elect");
    expect(agendaStripText(s)).toBe("Agenda: Minister of Commerce · Elect Player · Your vote");
  });

  it("resolution and waiting without a decision keep the agenda; reactions without a seat say so", () => {
    const vote = choice("cast_vote", "p1", { details: { agenda: PEACE } });
    const memory = advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info: agendaInfoFromChoice(vote) });
    const resolving = strip(null, memory)!;
    expect(agendaStripText(resolving)).toBe("Agenda 1/2: Minister of Peace · For/Against");
    expect(agendaStripText(strip(null, memory, { waitingForReactions: true })!)).toContain("Reactions");
  });

  it("no strip before the agenda phase or for an unrelated decision", () => {
    expect(deriveAgendaStrip({ phase: "action", choice: choice("activate_system", "p1"), memory: EMPTY_AGENDA_MEMORY, viewerSeat: "p1", label })).toBeNull();
  });

  it("without any name the line still says what is going on", () => {
    expect(agendaStripText(strip(choice("cast_vote", "p1"))!)).toBe("Agenda: For/Against · Your vote");
  });
});

describe("agenda memory across the phase", () => {
  const info = (agenda: typeof PEACE) => agendaInfoFromChoice(choice("cast_vote", "p1", { details: { agenda } }));

  it("counts the second agenda and keeps the first number while it is current", () => {
    let memory = advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info: info(PEACE) });
    memory = advanceAgendaMemory(memory, { phase: "agenda", round: 3, info: info(PEACE) });
    expect(memory.agendas).toHaveLength(1);
    const first = strip(choice("cast_vote", "p1", { details: { agenda: PEACE } }), memory)!;
    expect(first.number).toBe(1);
    memory = advanceAgendaMemory(memory, { phase: "agenda", round: 3, info: info(COMMERCE) });
    const second = strip(choice("cast_vote", "p1", { details: { agenda: COMMERCE } }), memory)!;
    expect(second.number).toBe(2);
    expect(agendaStripText(second)).toMatch(/^Agenda 2\/2: Minister of Commerce/);
  });

  it("clears when the phase ends and restarts in a new round", () => {
    let memory = advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info: info(PEACE) });
    memory = advanceAgendaMemory(memory, { phase: "status", round: 3, info: null });
    expect(memory).toEqual(EMPTY_AGENDA_MEMORY);
    expect(strip(null, memory, { phase: "status" })).toBeNull();
    memory = advanceAgendaMemory(
      advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info: info(PEACE) }),
      { phase: "agenda", round: 4, info: info(PEACE) },
    );
    expect(memory.agendas).toHaveLength(1);
    expect(memory.round).toBe(4);
  });

  it("returns the same object when nothing changed", () => {
    const memory = advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info: info(PEACE) });
    expect(advanceAgendaMemory(memory, { phase: "agenda", round: 3, info: info(PEACE) })).toBe(memory);
    expect(advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "action", round: 3, info: null })).toBe(EMPTY_AGENDA_MEMORY);
  });
});

describe("the agenda from the game state (visible to every seat)", () => {
  const lookup = (alias: string) =>
    alias === "minister_of_peace"
      ? { id: alias, name: "Minister of Peace", kind: "Law", elect: "For/Against", text: "For: Ready Mecatol Rex.", against: "Against: Discard this card." }
      : alias === "minister_of_commerce"
        ? { id: alias, name: "Minister of Commerce", kind: "Law", elect: "Elect Player", text: "Gain trade goods." }
        : undefined;
  const state = (extra: Record<string, unknown> = {}) => ({ agenda_in_play: "minister_of_peace", agenda_choices: ["FOR", "AGAINST"], ...extra });
  const base = (extra: Partial<Parameters<typeof deriveAgendaStrip>[0]> = {}) => ({
    phase: "agenda",
    round: 3,
    choice: null,
    memory: EMPTY_AGENDA_MEMORY,
    viewerSeat: "p1",
    speaker: "p2",
    seatingOrder: ["p1", "p2", "p3", "p4"],
    label: (seat: string) => seat.toUpperCase(),
    outcomeLabel: (o: string) => (o === "FOR" ? "For" : o === "AGAINST" ? "Against" : o.toUpperCase()),
    ...extra,
  });

  it("reads nothing from a state without the fields", () => {
    expect(readAgendaTable({})).toEqual(EMPTY_AGENDA_TABLE);
    expect(readAgendaTable(null)).toEqual(EMPTY_AGENDA_TABLE);
    expect(agendaInfoFromTable(EMPTY_AGENDA_TABLE, lookup)).toBeNull();
  });

  it("builds the card from the catalog, with both outcomes", () => {
    const info = agendaInfoFromTable(readAgendaTable(state()), lookup)!;
    expect(info).toMatchObject({ id: "minister_of_peace", name: "Minister of Peace", type: "Law", target: "For/Against" });
    expect(info.text).toEqual(["For: Ready Mecatol Rex.", "Against: Discard this card."]);
    expect(agendaInfoFromTable(readAgendaTable(state({ agenda_in_play: "minister_of_commerce" })), lookup)!.text).toEqual(["Gain trade goods."]);
    expect(agendaInfoFromTable(readAgendaTable(state({ agenda_in_play: "mystery" })), lookup)).toMatchObject({ name: "Mystery", text: [] });
  });

  it("a seat that is not asked still sees the agenda, the casts, the total, who is next and the riders", () => {
    const table = readAgendaTable(
      state({
        agenda_casts: [
          { player: "p3", outcome: "FOR", votes: 4 },
          { player: "p4", outcome: "AGAINST", votes: 2 },
          { player: "p1", votes: 0 },
        ],
        agenda_predictions: { p2: "AGAINST" },
      }),
    );
    const info = agendaInfoFromTable(table, lookup);
    const memory = advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info });
    const strip = deriveAgendaStrip(base({ memory, table }))!;
    expect(strip.name).toBe("Minister of Peace");
    expect(strip.kindLabel).toBe("For/Against");
    expect(strip.number).toBe(1);
    expect(strip.casts).toEqual([
      { label: "P3", you: false, outcome: "For", votes: 4, badge: null },
      { label: "P4", you: false, outcome: "Against", votes: 2, badge: null },
      { label: "P1", you: true, outcome: null, votes: 0, badge: null },
    ]);
    expect(strip.tally).toBe("For 4 · Against 2");
    expect(strip.waiting).toEqual([]);
    expect(strip.riders).toEqual([{ label: "P2", outcome: "Against" }]);
  });

  it("lists the seats still to vote in voting order, speaker last", () => {
    const table = readAgendaTable(state({ agenda_casts: [{ player: "p3", outcome: "FOR", votes: 1 }] }));
    const strip = deriveAgendaStrip(base({ table, memory: advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info: agendaInfoFromTable(table, lookup) }) }))!;
    expect(strip.waiting).toEqual(["P4", "You", "P2"]);
    expect(votingOrder(["a", "b", "c"], "a")).toEqual(["b", "c", "a"]);
    expect(votingOrder(["a", "b"], null)).toEqual(["a", "b"]);
  });

  it("numbers the second agenda from the log and prefers the decision's printed card", () => {
    const table = readAgendaTable(
      state({ agenda_in_play: "minister_of_commerce", agenda_log: [{ round: 3, agenda: "minister_of_peace" }, { round: 2, agenda: "old" }] }),
    );
    const info = mergeAgendaInfo(null, agendaInfoFromTable(table, lookup));
    const strip = deriveAgendaStrip(base({ table, memory: advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info }) }))!;
    expect(strip.number).toBe(2);
    expect(strip.kindLabel).toBe("Elect Player");
    const printed = { id: "minister_of_commerce", name: "Printed", type: "Law", target: "Elect Player", text: ["From the decision."] };
    expect(mergeAgendaInfo(printed, info)).toMatchObject({ name: "Printed", text: ["From the decision."] });
  });

  it("is gone once the agenda is resolved and nothing is in play, and back at the next reveal", () => {
    const resolved = readAgendaTable({ agenda_log: [{ round: 3, agenda: "minister_of_peace" }], agenda_casts: [{ player: "p1", outcome: "FOR", votes: 3 }] });
    const memory = advanceAgendaMemory(EMPTY_AGENDA_MEMORY, { phase: "agenda", round: 3, info: { id: "minister_of_peace", name: "Minister of Peace", type: null, target: null, text: [] } });
    expect(deriveAgendaStrip(base({ table: resolved, memory }))).toBeNull();
    // an older server (no log): unchanged, the strip stays for the phase
    expect(deriveAgendaStrip(base({ table: EMPTY_AGENDA_TABLE, memory }))).not.toBeNull();
    // the log of an earlier round does not hide it
    const earlier = readAgendaTable({ agenda_log: [{ round: 2, agenda: "minister_of_peace" }] });
    expect(deriveAgendaStrip(base({ table: earlier, memory }))).not.toBeNull();
  });

  it("shows no casts after the vote closed (nothing in play)", () => {
    const strip = deriveAgendaStrip(base({ table: readAgendaTable({ agenda_casts: [{ player: "p1", outcome: "FOR", votes: 3 }] }), memory: EMPTY_AGENDA_MEMORY }))!;
    expect(strip.casts).toEqual([]);
    expect(strip.tally).toBeNull();
  });
});

describe("no raw player id ever reaches the banner text", () => {
  const hex = (n: number) => `player_${n.toString(16).padStart(2, "0").repeat(32)}`;
  const names = ["Felix", "Ada", "Bob", "Cy", "Dee", "Eve"];
  for (const count of [3, 4, 6]) {
    it(`${count} players, Elect Player`, () => {
      const seats = Array.from({ length: count }, (_, i) => hex(i + 1));
      const nameOf = (seat: string) => names[seats.indexOf(seat)] ?? "Unknown";
      const table = readAgendaTable({
        agenda_in_play: "prophecy",
        agenda_choices: seats,
        agenda_casts: seats.slice(0, 2).map((p, i) => ({ player: p, outcome: seats[count - 1 - i], votes: 3 + i })),
        agenda_predictions: { [seats[2]]: seats[1] },
      });
      const choice = {
        actor: seats[0],
        prompt: "vote",
        nonce: "n",
        context: { subtype: "cast_vote" },
        options: [
          ...seats.map((id) => ({ id, label: id, kind: "vote", payload: { current_votes: 2 } })),
          { id: "decline", label: "Abstain", kind: "decline" },
        ],
      } as unknown as PendingChoiceDto;
      const strip = deriveAgendaStrip({
        phase: "agenda",
        round: 2,
        choice,
        memory: EMPTY_AGENDA_MEMORY,
        viewerSeat: seats[0],
        speaker: seats[count - 1],
        seatingOrder: seats,
        table,
        label: nameOf,
        outcomeLabel: (o) => agendaOutcomeLabel(o, (id) => (seats.includes(id) ? nameOf(id) : null)),
      })!;
      const { whose, ...shown } = strip; // whose.seat is an internal key, only its text is shown
      const text = JSON.stringify({ ...shown, whose: whose?.text }) + agendaStripText(strip);
      expect(text).not.toMatch(/player_[0-9a-f]{8}/);
      expect(strip.outcomes).toEqual(seats.map(nameOf));
      expect(strip.tally).toContain("Felix 2");
    });
  }

  it("falls back to a neutral word for an unknown player id and names strategy cards and laws", () => {
    const nobody = () => null;
    expect(agendaOutcomeLabel(hex(9), nobody)).toBe("a player");
    expect(agendaOutcomeLabel("pok3politics", nobody)).not.toMatch(/pok3/);
  });
});

describe("a reaction window offers cards, not outcomes", () => {
  it("lists no outcomes for the reaction options", () => {
    const choice = {
      actor: "p1",
      prompt: "reaction",
      nonce: "n",
      context: { subtype: "reaction_when_AGENDA_REVEALED", trigger: { kind: "agenda_revealed", agenda: "minister_peace" } },
      options: [{ id: "reaction:letnev:AGENDA_REVEALED:after", label: "Play Sabotage", kind: "ability" }, { id: "decline", label: "Pass", kind: "decline" }],
    } as unknown as PendingChoiceDto;
    const strip = deriveAgendaStrip({ phase: "agenda", round: 1, choice, memory: EMPTY_AGENDA_MEMORY, viewerSeat: "p1", label: (s) => s });
    expect(strip!.outcomes).toEqual([]);
  });
});
