import { describe, expect, it } from "vitest";
import type { PendingChoiceDto } from "../protocol/types.ts";
import {
  PAUSABLE_PROMPTS,
  askStorageKey,
  holdsPrompt,
  lawsOwnedBy,
  pausablePrompt,
  promptForOption,
  promptsForSubtype,
  promptsHeldBy,
  promptsOfChoice,
} from "./pausablePrompts.ts";
import { pausedDecline } from "./pausedOffer.ts";

/** The option id the engine puts in the reaction window for each ability that is offered there. */
const WINDOW_OPTION: Record<string, string> = {
  i48s: "leader:l1z1x:l1z1xagent:SYSTEM_ACTIVATED:after",
  aetherpassage: "ability:empyrean:aetherpassage:SYSTEM_ACTIVATED:after",
  instinct_training: "technology:xxcha:it:ACTION_CARD_PLAYED:when",
  arborec_commander: "leader:arborec:arboreccommander:SYSTEM_ACTIVATED:after",
  argent_agent: "leader:argent:argentagent:GROUND_FORCES_BEING_PRODUCED:when",
  argent_commander: "leader:argent:argentcommander:UNIT_ABILITY_ROLLED:when",
  bastion_agent: "leader:bastion:bastionagent:SHIP_DESTROYED:when",
  phoenix_standard: "ability:bastion:phoenixstandard:SPACE_COMBAT_ENDED:after",
  proxima_targeting: "technology:bastion:proxima:GROUND_COMBAT_ROUND_BEGAN:after",
  cabal_agent: "leader:cabal:cabalagent:COMMODITIES_REPLENISHED:after",
  crimson_exile: "unit:crimson:crimson_destroyer:SPACE_COMBAT_ENDED:after",
  subatomic_splicer: "technology:crimson:subatomic:SHIP_DESTROYED:after",
  null_reference: "technology:nekro:nekroc4y:SHIP_DESTROYED:after",
  technological_singularity: "ability:nekro:technological_singularity:SHIP_DESTROYED:after",
  aetherstream: "technology:empyrean:as:SYSTEM_ACTIVATED:after",
  empyrean_agent: "leader:empyrean:empyreanagent:MOVEMENT_FINISHED:after",
  empyrean_commander: "leader:empyrean:empyreancommander:SHIP_MOVED:after",
  dynamo: "unit:empyrean:empyrean_flagship:SUSTAIN_DAMAGE_USED:after",
  watcher: "unit:empyrean:empyrean_mech:ACTION_CARD_PLAYED:when",
  void_tether: "breakthrough:empyrean:empyreanbt_allow:SYSTEM_ACTIVATED:after",
  firmament_agent: "leader:firmament:firmamentagent:SYSTEM_ACTIVATED:after",
  ghost_agent: "leader:ghost:ghostagent:SYSTEM_ACTIVATED:after",
  icarus_drive: "unit:ghost:ghost_mech:SYSTEM_ACTIVATED:after",
  laws_order: "ability:keleres:laws_order:TURN_BEGAN:after",
  pillage: "ability:mentak:pillage:TRADE_GOODS_GAINED:after",
  muaat_commander: "leader:muaat:muaatcommander:STRATEGY_TOKEN_SPENT:after",
  naalu_agent: "leader:naalu:naaluagent-te:COMMAND_TOKEN_PLACED:after",
  foresight: "ability:naalu:foresight:MOVEMENT_FINISHED:after",
  naaz_agent: "leader:naaz:naazagent:TURN_PASSED:after",
  thunders_paradox: "breakthrough:nomad:nomadbt:TURN_BEGAN:after",
  temporal_command_suite: "technology:nomad:tcs:AGENT_EXHAUSTED:after",
  nomad_artuno: "leader:nomad:nomadagentartuno:TRADE_GOODS_GAINED:after",
  nomad_mercer: "leader:nomad:nomadagentmercer:TACTICAL_ACTION_ENDED:after",
  nomad_thundarian: "leader:nomad:nomadagentthundarian:GROUND_COMBAT_ROLL_STEP_ENDED:after",
  obsidian_agent: "leader:obsidian:obsidianagent:SHIP_DESTROYED:when",
  alarum: "unit:ralnel:ralnel_mech:GROUND_COMBAT_ROUND_ENDED:after",
  saar_agent: "leader:saar:saaragent:SYSTEM_ACTIVATED:after",
  exotrireme: "technology:sardakk:exo2:SPACE_COMBAT_ROUND_ENDED:after",
  sardakk_agent: "leader:sardakk:sardakkagent:ACTION_COMPLETED:after",
  titans_agent: "leader:titans:titansagent:HITS_TO_ASSIGN:when",
  titans_commander: "leader:titans:titanscommander:PRODUCTION_USED:when",
  hegemonic_trade_policy: "technology:winnu:htp:PRODUCTION_USED:after",
  winnu_agent: "leader:winnu:winnuagent:PRODUCTION_USED:after",
  yin_spinner: "technology:yin:yso:UNITS_PRODUCED:after",
  yin_agent: "leader:yin:yinagent:SHIP_DESTROYED:after",
  yssaril_commander: "leader:yssaril:yssarilcommander:SYSTEM_ACTIVATED:after",
  letnev_agent: "leader:letnev:letnevagent:COMBAT_ROUND_STARTED:after",
  sol_agent: "leader:sol:solagent:GROUND_COMBAT_ROUND_STARTED:after",
  sol_commander: "leader:sol:solcommander:GROUND_COMBAT_STARTED:after",
};

/** Yssaril's copies of an agent: the Ssruu ids carry the borrowed agent's id. */
const SSRUU_COPY: Record<string, string> = {
  letnev_agent: "leader:ssruu-copy:yssaril:letnevagent:letnev:COMBAT_ROUND_STARTED:after",
  i48s: "leader:ssruu-copy:yssaril:l1z1xagent:l1z1x:SYSTEM_ACTIVATED:after",
  argent_agent: "leader:yssaril:argent:yssarilagent:argentagent:GROUND_FORCES_BEING_PRODUCED:when",
  ghost_agent: "leader:yssaril:ghost:yssarilagent:ghostagent:SYSTEM_ACTIVATED:after",
  naalu_agent: "leader:yssaril:yssaril:yssarilagent:naaluagent-te:COMMAND_TOKEN_PLACED:after",
  saar_agent: "leader:yssaril:saar:yssarilagent:yssaril:saaragent:SYSTEM_ACTIVATED:after",
  winnu_agent: "leader:yssaril:winnu:yssarilagent:winnuagent:PRODUCTION_USED:after",
  yin_agent: "leader:yssaril:yin:yssarilagent:yinagent:GROUND_FORCE_DESTROYED:after",
};

const SUBTYPE_ONLY = PAUSABLE_PROMPTS.filter((p) => !p.option);

function choiceOf(options: { id: string; kind: string }[], subtype = "reaction_after_SYSTEM_ACTIVATED", actor = "p1") {
  return {
    prompt: "after SYSTEM_ACTIVATED",
    actor,
    nonce: "n1",
    context: { subtype, optional: true },
    options: options.map((o) => ({ ...o, label: o.id })),
  } as unknown as PendingChoiceDto;
}
const decline = { id: "decline", kind: "decline" };
const paused = (...ids: string[]) => (id: string) => ids.includes(id);

describe("the pausable prompt registry", () => {
  it("keeps the storage keys of the first two abilities", () => {
    expect(askStorageKey("i48s")).toBe("player_ask_i48s");
    expect(askStorageKey("aetherpassage")).toBe("player_ask_aetherpassage");
    expect(pausablePrompt("i48s")?.subtypes).toEqual(["l1z1x_agent_swap"]);
  });

  it("has unique ids, and every entry says how its question is recognised", () => {
    const ids = PAUSABLE_PROMPTS.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const p of PAUSABLE_PROMPTS) {
      expect(p.id).toMatch(/^[a-z0-9_]+$/);
      expect(Boolean(p.option) || p.subtypes.length > 0, p.id).toBe(true);
      expect(p.factions.length + p.technologies.length + p.relics.length + p.laws.length, p.id).toBeGreaterThan(0);
      expect(p.label.length).toBeGreaterThan(0);
    }
  });

  it("matches the engine's option id of each window ability, and only that ability", () => {
    const windowEntries = PAUSABLE_PROMPTS.filter((p) => p.option);
    expect(Object.keys(WINDOW_OPTION).sort()).toEqual(windowEntries.map((p) => p.id).sort());
    for (const [id, optionId] of Object.entries(WINDOW_OPTION)) {
      expect(promptForOption(optionId)?.id, optionId).toBe(id);
    }
  });

  it("matches Ssruu's copies of an agent as that agent", () => {
    for (const [id, optionId] of Object.entries(SSRUU_COPY)) {
      expect(promptForOption(optionId)?.id, optionId).toBe(id);
    }
  });

  it("does not mistake an action card slot or another ability for a registered one", () => {
    expect(promptForOption("reaction:empyrean:SYSTEM_ACTIVATED:after")).toBeUndefined();
    expect(promptForOption("ability:empyrean:other:SYSTEM_ACTIVATED:after")).toBeUndefined();
    expect(promptForOption("leader:empyrean:aetherpassage:SYSTEM_ACTIVATED:after")).toBeUndefined();
  });

  it("recognises the abilities that ask by decision subtype", () => {
    for (const p of SUBTYPE_ONLY) {
      for (const s of p.subtypes) expect(promptsForSubtype(s, p.contentSources[0]).map((x) => x.id)).toContain(p.id);
    }
    expect(promptsForSubtype("self_assembly_mech").map((p) => p.id)).toEqual(["self_assembly_routines"]);
    expect(promptsForSubtype("produce_unit")).toEqual([]);
    expect(promptsForSubtype(undefined)).toEqual([]);
  });

  it("no subtype is claimed by two abilities", () => {
    const seen = new Map<string, string>();
    for (const p of PAUSABLE_PROMPTS) {
      for (const s of p.subtypes) {
        expect(seen.get(s), s).toBeUndefined();
        seen.set(s, p.id);
      }
    }
  });

  it("knows who has an ability: by faction (Keleres covers its three), technology or relic", () => {
    expect(holdsPrompt(pausablePrompt("laws_order")!, { faction: "keleresa" })).toBe(true);
    expect(holdsPrompt(pausablePrompt("laws_order")!, { faction: "sol" })).toBe(false);
    expect(holdsPrompt(pausablePrompt("self_assembly_routines")!, { faction: "sol", technologies: ["sar"] })).toBe(true);
    expect(holdsPrompt(pausablePrompt("heart_of_ixth")!, { faction: "sol", relics: ["heartofixth"] })).toBe(true);
    expect(holdsPrompt(pausablePrompt("i48s")!, null)).toBe(false);
    expect(promptsHeldBy({ faction: "empyrean" }).map((p) => p.id)).toContain("aetherpassage");
    expect(promptsHeldBy({ faction: "hacan" }).map((p) => p.id)).toEqual(["wrath_of_kenara"]);
  });

  it("lists the abilities a question is about", () => {
    const window = choiceOf([
      { id: WINDOW_OPTION.aetherpassage, kind: "ability" },
      { id: WINDOW_OPTION.i48s, kind: "ability" },
      decline,
    ]);
    expect(promptsOfChoice(window).map((p) => p.id)).toEqual(["aetherpassage", "i48s"]);
    expect(promptsOfChoice(choiceOf([{ id: "use", kind: "leader" }, decline], "l1z1x_agent_swap")).map((p) => p.id)).toEqual(["i48s"]);
    expect(promptsOfChoice(null)).toEqual([]);
  });
});

describe("the agents and commanders that ask in a combat round", () => {
  it("registers the Letnev agent, the Sol agent window and the Farran question", () => {
    expect(promptForOption("leader:letnev:letnevagent:COMBAT_ROUND_STARTED:after")?.id).toBe("letnev_agent");
    expect(promptForOption("leader:sol:solagent:GROUND_COMBAT_ROUND_STARTED:after")?.id).toBe("sol_agent");
    expect(promptsForSubtype("letnev_commander_trade_good").map((p) => p.id)).toEqual(["letnev_commander"]);
    expect(promptsHeldBy({ faction: "letnev" }).map((p) => p.id)).toContain("letnev_agent");
  });
});

describe("Minister of Peace", () => {
  const offer = (alias: string, actor = "p1"): PendingChoiceDto =>
    ({
      prompt: "Minister of Peace: discard to end the active player's turn",
      actor,
      nonce: "n1",
      context: { subtype: "offer_discard_law", optional: false, source: { Content: alias } },
      details: { kind: "offer" },
      options: [
        { id: "discard", kind: "law", label: "discard minister_peace" },
        { id: "decline", kind: "decline", label: "Decline" },
      ],
    }) as unknown as PendingChoiceDto;

  it("is told apart from the other ministers that share the discard-offer subtype", () => {
    expect(promptsOfChoice(offer("minister_peace")).map((p) => p.id)).toEqual(["minister_of_peace"]);
    expect(promptsOfChoice(offer("minister_war"))).toEqual([]);
    expect(promptsForSubtype("offer_discard_law")).toEqual([]);
    expect(askStorageKey("minister_of_peace")).toBe("player_ask_minister_of_peace");
  });

  it("is declined for the owner only while paused, and never for the other ministers", () => {
    expect(pausedDecline(offer("minister_peace"), "p1", paused("minister_of_peace"))?.optionId).toBe("decline");
    expect(pausedDecline(offer("minister_peace"), "p1", paused())).toBeNull();
    expect(pausedDecline(offer("minister_peace"), "p2", paused("minister_of_peace"))).toBeNull();
    expect(pausedDecline(offer("minister_war"), "p1", paused("minister_of_peace"))).toBeNull();
  });

  it("is listed on the sheet of whoever owns the law", () => {
    const owned = lawsOwnedBy("p1", { minister_peace: "p1", minister_war: "p2", conventions: "for" });
    expect(owned).toEqual(["minister_peace"]);
    expect(promptsHeldBy({ faction: "hacan", laws: owned }).map((p) => p.id)).toEqual(["minister_of_peace", "wrath_of_kenara"]);
    expect(promptsHeldBy({ faction: "hacan", laws: [] }).map((p) => p.id)).not.toContain("minister_of_peace");
    expect(lawsOwnedBy(null, { minister_peace: "p1" })).toEqual([]);
  });
});

describe("pausedDecline", () => {
  const sole = (id: string) => choiceOf([{ id: WINDOW_OPTION[id], kind: "ability" }, decline]);

  it("declines the window of every registered ability while it is paused, and only then", () => {
    for (const id of Object.keys(WINDOW_OPTION)) {
      expect(pausedDecline(sole(id), "p1", paused(id))?.optionId, id).toBe("decline");
      expect(pausedDecline(sole(id), "p1", paused()), id).toBeNull();
      expect(pausedDecline(sole(id), "p1", paused("pillage_not_this_one")), id).toBeNull();
    }
  });

  it("declines only the offered actor's own question", () => {
    expect(pausedDecline(sole("dynamo"), "p2", paused("dynamo"))).toBeNull();
    expect(pausedDecline(sole("dynamo"), null, paused("dynamo"))).toBeNull();
    expect(pausedDecline(sole("dynamo"), undefined, paused("dynamo"))).toBeNull();
  });

  it("leaves the question open when another answer than a paused ability is in the window", () => {
    const withCard = choiceOf([
      { id: WINDOW_OPTION.aetherpassage, kind: "ability" },
      { id: "reaction:empyrean:SYSTEM_ACTIVATED:after", kind: "ability" },
      decline,
    ]);
    expect(pausedDecline(withCard, "p1", paused("aetherpassage"))).toBeNull();
    const withOther = choiceOf([
      { id: WINDOW_OPTION.aetherpassage, kind: "ability" },
      { id: WINDOW_OPTION.saar_agent, kind: "ability" },
      decline,
    ]);
    expect(pausedDecline(withOther, "p1", paused("aetherpassage"))).toBeNull();
    expect(pausedDecline(withOther, "p1", paused("saar_agent"))).toBeNull();
  });

  it("declines a window of several abilities when every one of them is paused", () => {
    const both = choiceOf([
      { id: WINDOW_OPTION.aetherpassage, kind: "ability" },
      { id: WINDOW_OPTION.saar_agent, kind: "ability" },
      decline,
    ]);
    const result = pausedDecline(both, "p1", paused("aetherpassage", "saar_agent"));
    expect(result?.optionId).toBe("decline");
    expect(result?.prompts.map((p) => p.label)).toEqual(["Aetherpassage", "Captain Mendosa"]);
  });

  it("declines an ability that asks its own question by subtype", () => {
    const own = choiceOf([{ id: "pick", kind: "self_assembly" }, decline], "self_assembly_mech");
    expect(pausedDecline(own, "p1", paused("self_assembly_routines"))?.optionId).toBe("decline");
    expect(pausedDecline(own, "p1", paused())).toBeNull();
    const noDecline = choiceOf([{ id: "pick", kind: "self_assembly" }], "self_assembly_mech");
    expect(pausedDecline(noDecline, "p1", paused("self_assembly_routines"))).toBeNull();
  });

  it("never answers a question that has no decline", () => {
    const forced = choiceOf([{ id: WINDOW_OPTION.aetherpassage, kind: "ability" }]);
    expect(pausedDecline(forced, "p1", paused("aetherpassage"))).toBeNull();
  });
});
