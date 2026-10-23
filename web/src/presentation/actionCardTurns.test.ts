import { describe, it, expect } from "vitest";
import { actionCardTurns, offersOwnTurnForCard } from "./actionCardTurns.ts";
import { ACTION_CARDS } from "../protocol/contentCatalog.ts";

type Meta = { name: string; phase?: string; window?: string };
const cards = () => Object.values(ACTION_CARDS as Record<string, Meta>);
const names = (pick: (card: Meta) => boolean) => [...new Set(cards().filter(pick).map((c) => c.name))].sort();

describe("action card turns", () => {
  it("reads the printed window", () => {
    expect(actionCardTurns("Flank Speed", "After you activate a system")).toBe("own");
    expect(actionCardTurns("Counterstroke", "After another player activates a system that contains 1 of your command tokens")).toBe("others");
    expect(actionCardTurns("Sabotage", "When another player plays an action card other than 'Sabotage'")).toBe("both");
    expect(actionCardTurns("Skilled Retreat", "At the start of a combat round")).toBe("both");
  });

  it("an invasion window is told apart by the card", () => {
    const invasion = "At the start of an invasion";
    expect(actionCardTurns("Blitz", invasion)).toBe("own");
    expect(actionCardTurns("Bunker", invasion)).toBe("others");
  });

  it("offers it for reactions of the action phase that come up in both turns", () => {
    expect(offersOwnTurnForCard({ name: "Sabotage", phase: "Any", window: "When another player plays an action card" })).toBe(true);
    expect(offersOwnTurnForCard({ name: "Direct Hit", phase: "Action", window: "After another player's ship uses SUSTAIN DAMAGE" })).toBe(true);
    // A component action is not a reaction; agenda and strategy cards meet no active player.
    expect(offersOwnTurnForCard({ name: "Brilliance", phase: "Action", window: "Action" })).toBe(false);
    expect(offersOwnTurnForCard({ name: "Veto", phase: "Agenda", window: "When an agenda is revealed" })).toBe(false);
    expect(offersOwnTurnForCard({ name: "Summit", phase: "Strategy", window: "At the start of the strategy phase" })).toBe(false);
    expect(offersOwnTurnForCard(undefined)).toBe(false);
  });

  it("splits the real action cards as decided", () => {
    const eligible = names((c) => offersOwnTurnForCard(c));
    expect(eligible).toContain("Sabotage");
    expect(eligible).toContain("Shields Holding");
    expect(eligible).toContain("Direct Hit");
    expect(eligible).toContain("Skilled Retreat");
    expect(eligible).toContain("Courageous to the End");
    for (const own of ["Flank Speed", "Nav Suite", "Rally", "Upgrade", "Master Plan", "Blitz", "Disable", "War Machine", "Infiltrate"]) {
      expect(eligible, own).not.toContain(own);
    }
    for (const others of ["Counterstroke", "Decoy Operation", "Forward Supply Base", "Ghost Squad", "Parley", "Reparations", "Bunker", "Rout", "Experimental Battlestation", "Coup d'Etat", "Extreme Duress"]) {
      expect(eligible, others).not.toContain(others);
    }
  });
});
