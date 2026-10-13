import { describe, it, expect } from "vitest";
import type { PendingChoiceDto } from "../protocol/types.ts";
import { reactionCardNotice } from "./reactionNotice.ts";

const labels = { actor: (id: string) => `Seat ${id}` };

const choice = (trigger: Record<string, unknown>, actor = "p1"): PendingChoiceDto =>
  ({
    actor,
    prompt: "when ACTION_CARD_PLAYED",
    nonce: "n",
    options: [],
    context: { subtype: "reaction_when_ACTION_CARD_PLAYED", trigger },
  }) as unknown as PendingChoiceDto;

describe("reactionCardNotice", () => {
  it("names the card another player played with its text and every target the trigger carries", () => {
    const notice = reactionCardNotice(
      choice({
        kind: "action_card_played",
        event_type: "ACTION_CARD_PLAYED",
        event_id: 7,
        relation: "when",
        actor: "p2",
        card: "sabotage",
        system: "27",
        planet: "mecatol_rex",
        subject: "p3",
      }),
      "p1",
      labels,
    );
    expect(notice?.id).toBe("reactcard:7:action_card_played");
    expect(notice?.text).toContain("Seat p2 played the action card Sabotage");
    expect(notice?.text).toContain("Target: system #27, planet Mecatol Rex, player Seat p3.");
  });

  it("covers a discard (Reverse Engineer) and ignores own plays and other triggers", () => {
    const discard = {
      kind: "action_card_discarded",
      event_type: "ACTION_CARD_DISCARDED",
      event_id: 9,
      relation: "after",
      actor: "p2",
      card: "lucky_shot",
    };
    expect(reactionCardNotice(choice(discard), "p1", labels)?.text).toContain("discarded the action card Lucky Shot");
    expect(reactionCardNotice(choice({ ...discard, actor: "p1" }), "p1", labels)).toBeNull();
    expect(reactionCardNotice(choice({ ...discard, kind: "ship_moved", event_type: "SHIP_MOVED" }), "p1", labels)).toBeNull();
    expect(reactionCardNotice(choice(discard, "p9"), "p1", labels)).toBeNull();
  });
});
