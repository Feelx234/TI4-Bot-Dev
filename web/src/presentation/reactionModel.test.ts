import { describe, expect, it } from "vitest";
import type { GameEvent, PendingChoiceDto } from "../protocol/types.ts";
import { describeReaction } from "./reactionModel.ts";

const labels = {
  playerLabel: (id: string) => ({ p1: "Anna", p2: "Bob" })[id] ?? id,
};

const sabotageOffer = (subtype = "reaction_when_ACTION_CARD_PLAYED"): PendingChoiceDto => ({
  actor: "p2",
  nonce: "1",
  prompt: "when ACTION_CARD_PLAYED",
  context: { subtype, optional: true, source: { Reaction: "ACTION_CARD_PLAYED" } },
  options: [
    {
      id: "reaction:Sol:ACTION_CARD_PLAYED:when",
      kind: "ability",
      label: "Play Sabotage",
      payload: { card: "sabo1", card_name: "Sabotage" },
    },
    { id: "decline", kind: "decline", label: "Pass" },
  ],
});

const logEntry = (actor: string, detail: string): GameEvent => ({
  visibility: "public",
  id: "e1",
  timestamp: "t",
  actor,
  detail,
  event: { kind: "decision_resolved" },
});

describe("describeReaction", () => {
  it("builds the button from the card name, never 'Play play'", () => {
    const inner = sabotageOffer("play_reaction_when_ACTION_CARD_PLAYED");
    inner.options[0] = {
      id: "sabo1",
      kind: "action_card",
      label: "play Sabotage",
      payload: { card: "sabo1", card_name: "Sabotage" },
    };
    const model = describeReaction({ choice: inner, ...labels, viewerSeat: "p2" });
    expect(model.reactions.map((row) => row.buttonLabel)).toEqual(["Play Sabotage"]);
    expect(model.reactions[0].card?.text).toBe("Cancel that action card.");
    expect(model.reactions[0].card?.window).toContain("When another player plays an action card");
    const outer = describeReaction({ choice: sabotageOffer(), ...labels, viewerSeat: "p2" });
    expect(outer.reactions[0].buttonLabel).toBe("Play Sabotage");
    expect(outer.declineOptionId).toBe("decline");
  });

  it("level 2: names the card player from the public log", () => {
    const model = describeReaction({
      choice: sabotageOffer(),
      events: [logEntry("p1", "p1 played Mining Initiative")],
      ...labels,
      viewerSeat: "p2",
    });
    expect(model.source).toBe("log");
    expect(model.sentence).toBe("Anna played the action card Mining Initiative.");
    expect(model.facts.card?.text).toMatch(/\S/);
    expect(model.canNowSentence).toBe("Before this resolves, you can play Sabotage.");
    expect(model.title).toBe("Anna played an action card");
  });

  it("level 2: names the active player and system for an activation", () => {
    const choice = sabotageOffer("reaction_after_SYSTEM_ACTIVATED");
    const model = describeReaction({
      choice,
      activePlayerId: "p1",
      activeSystemId: "27",
      ...labels,
      viewerSeat: "p2",
    });
    expect(model.sentence).toBe("Anna activated System 27.");
    expect(model.canNowSentence).toBe("Now you can play Sabotage.");
  });

  it("level 3: only the window, never a raw engine id", () => {
    const model = describeReaction({
      choice: sabotageOffer("reaction_after_SHIP_DESTROYED"),
      ...labels,
      viewerSeat: "p2",
    });
    expect(model.source).toBe("subtype");
    expect(model.sentence).toBe("A reaction window opened: after a ship is destroyed.");
    expect(`${model.sentence}${model.title}`).not.toMatch(/[A-Z]{3,}_[A-Z_]+/);
  });

  it("never names the viewer as the trigger actor of someone else's card", () => {
    const model = describeReaction({
      choice: sabotageOffer(),
      events: [logEntry("p2", "p2 played Sabotage")],
      ...labels,
      viewerSeat: "p2",
    });
    expect(model.source).toBe("subtype");
  });

  it("turns Instinct Training into a use step with its text", () => {
    const model = describeReaction({
      choice: {
        actor: "p2",
        nonce: "2",
        prompt: "Instinct Training: exhaust ... cancel sabo1",
        context: { subtype: "instinct_training_cancel", source: { Content: "it" } },
        options: [
          { id: "use", kind: "technology", label: "cancel it" },
          { id: "decline", kind: "decline", label: "Pass" },
        ],
      },
      ...labels,
      viewerSeat: "p2",
    });
    expect(model.eventType).toBe("ACTION_CARD_PLAYED");
    expect(model.reactions[0].buttonLabel).toBe("Use Instinct Training");
    expect(model.reactions[0].note).toMatch(/cancel/i);
  });

  it("keeps the engine label for an unnamed multi-card outer offer", () => {
    const choice = sabotageOffer();
    choice.options[0] = { id: "reaction:Sol:X:when", kind: "ability", label: "Choose an action card…" };
    const model = describeReaction({ choice, ...labels, viewerSeat: "p2" });
    expect(model.reactions[0].buttonLabel).toBe("Choose an action card…");
  });
});
