import { describe, expect, it } from "vitest";
import { CONTENT } from "../protocol/catalogStore.ts";
import { describeComponent } from "./componentInfo.ts";
import { describeFaction, sheetLeaderIds } from "./factionInfo.ts";

interface Rec {
  id: string;
  faction: string;
  type: string;
  name: string;
  abilityText?: string;
}
const LEADERS = CONTENT.leaders as unknown as Record<string, Rec>;
const FACTIONS = CONTENT.factions as unknown as Record<string, { id: string; leaders: string[] }>;

const component = (id: string) => ({ id: `component|leader|${id}`, label: id, kind: "component", payload: {} });

describe("one leader text everywhere", () => {
  it("the Components menu prints exactly the catalog text of every leader", () => {
    for (const leader of Object.values(LEADERS)) {
      expect(describeComponent(component(leader.id)).text, leader.id).toBe(leader.abilityText ?? null);
    }
  });

  it("the faction card prints the catalog text of every sheet leader", () => {
    for (const faction of Object.values(FACTIONS)) {
      const info = describeFaction(faction.id);
      for (const line of info?.leaders ?? []) {
        expect(line.text, line.id).toBe(LEADERS[line.id].abilityText);
      }
    }
  });

  it("a seat holding another printing than the sheet's sees the held card on the faction card", () => {
    // Naalu's sheet lists the Thunder's Edge Z'eu; a Prophecy of Kings game deals `naaluagent`.
    const dealt = ["naaluagent", "naalucommander", "naaluhero"];
    const info = describeFaction("naalu", [], dealt)!;
    const agent = info.leaders.find((l) => l.type === "agent")!;
    expect(agent.id).toBe("naaluagent");
    expect(agent.text).toBe(describeComponent(component("naaluagent")).text);
    expect(agent.text).toMatch(/tactical action in a non-home system/);
    // without a seat the sheet's own printing stays
    expect(describeFaction("naalu")!.leaders.find((l) => l.type === "agent")!.id).toBe("naaluagent-te");
  });

  it("lists the factions whose dealt printing can differ from the sheet (every one is substituted)", () => {
    const alternates: string[] = [];
    for (const faction of Object.values(FACTIONS)) {
      for (const leader of Object.values(LEADERS)) {
        if (leader.faction !== faction.id || faction.leaders.includes(leader.id)) continue;
        const stands = faction.leaders.some((id) => LEADERS[id]?.type === leader.type);
        if (!stands) continue;
        const dealt = faction.leaders.map((id) => (LEADERS[id]?.type === leader.type ? leader.id : id));
        expect(sheetLeaderIds(faction, dealt), leader.id).toContain(leader.id);
        alternates.push(leader.id);
      }
    }
    console.log("alternate printings:", alternates.join(", "));
  });
});
