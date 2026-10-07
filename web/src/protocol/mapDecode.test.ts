import { describe, expect, it } from "vitest";
import { fixtureLobby, fixturePreview, fixtureTemplates } from "../dev/mapPickerFixtures.ts";
import { decodeLobby } from "./decode.ts";
import { decodeMapChoice, decodeMapList, decodeMapPreview } from "./mapDecode.ts";

describe("map protocol decoding", () => {
  it("keeps only maps that build", () => {
    const list = [
      ...fixtureTemplates(4),
      { ...fixtureTemplates(4)[0], alias: "4pStaticEq", buildable: false },
    ];
    expect(decodeMapList(list).map((m) => m.alias)).not.toContain("4pStaticEq");
    expect(decodeMapList(list)).toHaveLength(4);
    expect(() => decodeMapList({})).toThrow(/map list/);
  });

  it("round-trips a preview and rejects a broken one", () => {
    const preview = fixturePreview(4);
    expect(decodeMapPreview(JSON.parse(JSON.stringify(preview)))).toEqual(preview);
    expect(() => decodeMapPreview({ ...preview, tiles: "nope" })).toThrow(/tiles/);
    expect(() => decodeMapPreview({ ...preview, choice: { kind: "x" } })).toThrow(/choice/);
  });

  it("requires a template choice to name its alias", () => {
    expect(() =>
      decodeMapChoice({
        kind: "template",
        systems: 3,
        hyperlanes: false,
        recommended: false,
      }),
    ).toThrow();
    expect(
      decodeMapChoice({
        kind: "random",
        systems: 3,
        hyperlanes: false,
        recommended: false,
      }).kind,
    ).toBe("random");
  });

  it("decodes the lobby's map and tolerates a lobby without one", () => {
    const lobby = fixtureLobby(3, "3pHyperlanes", 4);
    const decoded = decodeLobby(JSON.parse(JSON.stringify(lobby)), "gallery-map");
    expect(decoded.map?.alias).toBe("3pHyperlanes");
    expect(decoded.map_revision).toBe(4);
    const { map: _map, map_revision: _rev, ...old } = lobby;
    void _map;
    void _rev;
    const legacy = decodeLobby(old, "gallery-map");
    expect(legacy.map).toBeUndefined();
    expect(legacy.map_revision).toBeUndefined();
  });

  it("reads the strategy card set and ignores one it does not know or none at all", () => {
    const raw = (set: unknown) =>
      JSON.parse(JSON.stringify({ ...fixtureLobby(3), strategy_card_set: set }));
    expect(decodeLobby(raw("te"), "gallery-map").strategy_card_set).toBe("te");
    expect(decodeLobby(raw("pok"), "gallery-map").strategy_card_set).toBe("pok");
    expect(decodeLobby(raw("base_game"), "gallery-map").strategy_card_set).toBeUndefined();
    expect(decodeLobby(raw(undefined), "gallery-map").strategy_card_set).toBeUndefined();
  });
});
