import { describe, expect, it } from "vitest";
import {
  describeAgendaPlacement,
  optionNote,
  replenishReason,
  seatStanding,
  type DecisionTable,
} from "./politicsDecision.ts";

const player = (id: string, faction: string, vp: number, commodities: number) =>
  ({ id, faction, victory_points: vp, commodities }) as DecisionTable["players"][number];
const table: DecisionTable = {
  players: [player("a", "sol", 3, 1), player("b", "hacan", 5, 0), player("c", "xxcha", 2, 3)],
  seating_order: ["a", "b", "c"],
  speaker: "b",
};

describe("politicsDecision", () => {
  it("describes the agenda with its text and falls back to the prompt", () => {
    const view = describeAgendaPlacement({
      prompt: "place x where",
      context: { subtype: "politics_place_agenda" } as never,
      details: { agenda: { name: "Mutiny", type: "Law", target: "For/Against", text1: "Vote.", text2: "" } },
    });
    expect(view).toMatchObject({ name: "Mutiny", kind: "Law", text: ["Vote."] });
    const bare = describeAgendaPlacement({
      prompt: "place minister_of_commerce where",
      context: { subtype: "politics_place_agenda" } as never,
    });
    expect(bare?.name).toBe("Minister Of Commerce");
    expect(describeAgendaPlacement({ prompt: "p", context: { subtype: "other" } as never })).toBeNull();
  });

  it("computes standing and speaker order from the current speaker", () => {
    expect(seatStanding(table, "c")).toMatchObject({ speakerOrder: 2, victoryPoints: 2, faction: "Xxcha" });
    expect(seatStanding(table, "b")?.isSpeaker).toBe(true);
    expect(seatStanding(table, "zz")).toBeNull();
  });

  it("notes speaker, replenish and hacan options without raw ids", () => {
    const speaker = { context: { subtype: "politics_choose_speaker" } as never, actor: "a", details: { seats: { Xxcha: "c" } } };
    const note = optionNote(speaker, { id: "Xxcha", label: "x" }, table);
    expect(note?.seat).toBe("c");
    expect(note?.text).toContain("2 VP");
    const hacan = { context: { subtype: "leader_hacanagent_branch" } as never, actor: "a", options: [{ id: "self", label: "s" }, { id: "b", label: "b" }, { id: "c", label: "c" }] };
    expect(optionNote(hacan, hacan.options[0], table)?.text).toContain("you hold 1");
    expect(optionNote(hacan, hacan.options[1], table)?.text).toContain("0 commodities held");
    expect(replenishReason(hacan, table)).toContain("holds 0");
    const trade = { context: { subtype: "trade_choose_replenish" } as never, actor: "a", details: { seats: {} } };
    expect(optionNote(trade, { id: "done", label: "d", kind: "decline" }, table)?.text).toMatch(/Nobody/);
  });
});
