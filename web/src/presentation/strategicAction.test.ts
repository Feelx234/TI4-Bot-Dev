import { describe, expect, it } from "vitest";
import { cardFamily, detectStrategicAction, prepareEligibility } from "./strategicAction.ts";
import { actionEvent, playedLog, player } from "../test/secondaryPrepFixtures.ts";

const primary = (cards: string[], exhausted: string[] = []) =>
  player("a", { strategy_cards: cards, exhausted_strategy_cards: exhausted });
const base = { phase: "action", activePlayer: "a" };

describe("cardFamily", () => {
  it("maps base and Thunder's Edge cards to the printed family", () => {
    expect(cardFamily("pok7technology")).toBe("technology");
    expect(cardFamily("te4construction")).toBe("construction");
    expect(cardFamily("te6warfare")).toBe("warfare");
    expect(cardFamily("nonsense")).toBeNull();
  });
});

describe("detectStrategicAction", () => {
  it("reads the card from the public 'played' detail of a multi-card holding", () => {
    const action = detectStrategicAction({
      ...base,
      events: playedLog("Technology"),
      players: [primary(["pok7technology", "pok1leadership"]), player("b")],
    });
    expect(action).toMatchObject({ primary: "a", card: "pok7technology", family: "technology", inferred: false });
    expect(action?.key).toContain("action_5");
  });

  it("infers the card from the single unexhausted card when the detail is generic", () => {
    const action = detectStrategicAction({
      ...base,
      events: playedLog(null),
      players: [primary(["pok2diplomacy", "pok1leadership"], ["pok1leadership"]), player("b")],
    });
    expect(action).toMatchObject({ card: "pok2diplomacy", inferred: true });
  });

  it("does not guess when two unexhausted cards could be meant", () => {
    expect(
      detectStrategicAction({
        ...base,
        events: playedLog(null),
        players: [primary(["pok2diplomacy", "pok1leadership"]), player("b")],
      }),
    ).toBeNull();
  });

  it("is over once the primary's card is exhausted", () => {
    expect(
      detectStrategicAction({
        ...base,
        events: playedLog("Technology"),
        players: [primary(["pok7technology"], ["pok7technology"]), player("b")],
      }),
    ).toBeNull();
  });

  it("is not in progress when the turn moved on (cancelled action) or for other action kinds", () => {
    const players = [primary(["pok7technology"]), player("b")];
    expect(
      detectStrategicAction({ ...base, activePlayer: "b", events: playedLog("Technology"), players }),
    ).toBeNull();
    expect(
      detectStrategicAction({
        ...base,
        events: [actionEvent("action_5", "a", { action_type: "tactical" })],
        players,
      }),
    ).toBeNull();
    expect(detectStrategicAction({ ...base, phase: "agenda", events: playedLog("Technology"), players })).toBeNull();
    expect(detectStrategicAction({ ...base, events: [], players })).toBeNull();
  });

  it("follows the latest action only", () => {
    const players = [primary(["pok7technology"]), player("b")];
    const events = [
      ...playedLog("Technology"),
      actionEvent("action_9", "b", { action_type: "tactical", action_actor: "b" }),
    ];
    expect(detectStrategicAction({ ...base, events, players })).toBeNull();
  });
});

describe("prepareEligibility", () => {
  const players = [primary(["pok7technology"]), player("b"), player("c", { strategic_tokens: 0 })];
  const events = playedLog("Technology");
  const action = detectStrategicAction({ ...base, events, players });

  it("allows an unasked follower with a token", () => {
    expect(prepareEligibility(action, "b", players, events)).toEqual({ canPrepare: true });
  });
  it("refuses the primary, spectators, tokenless followers and followers already asked", () => {
    expect(prepareEligibility(action, "a", players, events).reason).toBe("primary");
    expect(prepareEligibility(action, null, players, events).reason).toBe("no_seat");
    expect(prepareEligibility(action, "c", players, events).reason).toBe("no_token");
    const asked = [...events, actionEvent("action_5", "b", { detail: "b followed" })];
    expect(prepareEligibility(action, "b", players, asked).reason).toBe("asked");
    expect(prepareEligibility(null, "b", players, events).canPrepare).toBe(false);
  });
  it("lets Leadership be prepared without a token", () => {
    const leadershipPlayers = [primary(["pok1leadership"]), player("c", { strategic_tokens: 0 })];
    const log = playedLog("Leadership");
    const lead = detectStrategicAction({ ...base, events: log, players: leadershipPlayers });
    expect(prepareEligibility(lead, "c", leadershipPlayers, log).canPrepare).toBe(true);
  });
});
