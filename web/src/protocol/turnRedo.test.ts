import { describe, expect, it } from "vitest";
import { decodeTurnRedoStatus, decodeTurnRedoStatusResponse } from "./turnRedo.ts";

const base = {
  seat: "p2",
  requested_by: "p2",
  turns_back: 1,
  redo_count: 1,
  stage: "new_turn",
  original_decisions: 120,
  rewound_to: 101,
  turn_complete: false,
  handoff_len: null,
  outcome: null,
  can_control: true,
};

describe("decodeTurnRedoStatus", () => {
  it("decodes a redo waiting for the new turn", () => {
    expect(decodeTurnRedoStatus(base)).toEqual({ ...base, handoff_len: null, outcome: null });
  });

  it("decodes a hand-off, a conflict and an exhausted tail", () => {
    const handoff = decodeTurnRedoStatus({
      ...base,
      stage: "auto_played",
      handoff_len: 112,
      outcome: {
        kept: 9,
        tail_total: 9,
        stop: { kind: "handoff", seat: "p2" },
        asking_seat: "p2",
        deck_offsets: [{ deck: "action_card", delta: -1 }],
      },
    });
    expect(handoff.outcome?.stop).toEqual({ kind: "handoff", seat: "p2" });
    expect(handoff.outcome?.deck_offsets).toEqual([{ deck: "action_card", delta: -1 }]);

    const conflict = decodeTurnRedoStatus({
      ...base,
      stage: "auto_played",
      handoff_len: 106,
      outcome: {
        kept: 3,
        tail_total: 9,
        stop: {
          kind: "conflict",
          conflict: {
            original_cursor: 108,
            kind: "deck_cursor",
            seat: "p3",
            prompt: "action phase",
            detail: "the redone turn drew a different number of cards",
            deck_deltas: [{ deck: "action_card", delta: 1 }],
          },
        },
        asking_seat: "p3",
      },
    });
    expect(conflict.outcome?.stop).toMatchObject({ kind: "conflict" });
    expect(conflict.outcome?.deck_offsets).toEqual([]);

    const done = decodeTurnRedoStatus({
      ...base,
      stage: "auto_played",
      handoff_len: 120,
      outcome: { kept: 4, tail_total: 4, stop: { kind: "tail_exhausted" }, asking_seat: null },
    });
    expect(done.outcome?.asking_seat).toBeNull();
  });

  it("reads the response envelope", () => {
    expect(decodeTurnRedoStatusResponse({ status: null })).toBeNull();
    expect(decodeTurnRedoStatusResponse({ status: base })?.seat).toBe("p2");
    expect(() => decodeTurnRedoStatusResponse({})).toThrow(/missing status/);
  });

  it("rejects malformed input instead of guessing", () => {
    expect(() => decodeTurnRedoStatus({ ...base, stage: "later" })).toThrow(/invalid fields/);
    expect(() => decodeTurnRedoStatus({ ...base, rewound_to: -1 })).toThrow(/invalid fields/);
    expect(() =>
      decodeTurnRedoStatus({
        ...base,
        stage: "auto_played",
        outcome: { kept: 1, tail_total: 1, stop: { kind: "mystery" } },
      }),
    ).toThrow(/unknown stop kind/);
    expect(() =>
      decodeTurnRedoStatus({
        ...base,
        outcome: {
          kept: 0,
          tail_total: 1,
          stop: {
            kind: "conflict",
            conflict: { original_cursor: 1, kind: "nope", seat: "p1", prompt: "x", detail: "y" },
          },
        },
      }),
    ).toThrow(/invalid conflict/);
  });
});
