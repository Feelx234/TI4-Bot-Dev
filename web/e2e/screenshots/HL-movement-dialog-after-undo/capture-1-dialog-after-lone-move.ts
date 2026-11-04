import { expect, test } from "@playwright/test";
import { PROTOCOL_VERSION, type BoardView, type ClientMessage, type PlacedUnitView } from "../../../src/protocol/types";
import { GAME_ID, openMockedGame } from "../_shared/mockGame";
import { shot } from "../_shared/shot";
import { galleryPlayers } from "../_shared/fixtures";
import { actor } from "../_shared/players";

// Alpha report fb_1793818512 ("Why is no move dialog shown?"). The offline engine numbers the
// questions of a run from 1; an undo starts a new run. A lone movement question (no unit can move)
// is answered by the client with its dialog kept closed, and the client remembered that question by
// its nonce only. After an undo, the movement question of the second try (ships to move) had the
// same nonce and its dialog stayed closed. Now the dialog is closed only for the lone question itself.
const SIZES = { desktop: { width: 1440, height: 900 }, "360x601": { width: 360, height: 601 } } as const;

const planet = (id: string, label: string) => ({ id, label, resources: 2, influence: 2, traits: [], tech_specialties: [] });
const unit = (unit_type: string, extra: Partial<PlacedUnitView> = {}): PlacedUnitView => ({ owner: actor, unit_type, damaged: false, ...extra });
const board: BoardView = {
  active_system: "69",
  map_tiles: [
    { system_id: "69", label: "Accoen/Jeol-ir", q: 1, r: 0, planets: [planet("accoen", "Accoen"), planet("jeolir", "Jeol Ir")] },
    { system_id: "01", label: "Jord - Sol", q: 0, r: 1, planets: [planet("jord", "Jord")] },
  ],
  systems: {
    "69": { system_id: "69", command_tokens: [actor], planets: {}, units: [] },
    "01": {
      system_id: "01",
      command_tokens: [],
      planets: {},
      units: [unit("sol_carrier"), unit("sol_carrier"), unit("destroyer"), unit("fighter"), unit("fighter"), unit("fighter"), ...Array.from({ length: 5 }, () => unit("infantry", { planet: "jord" }))],
    },
  },
};

const move = (id: string, unit_type: string, capacity: number) => ({
  id,
  kind: "move",
  label: `move ${unit_type} from 01`,
  payload: { origin: "01", unit: unit_type, damaged: false, gravity_drive: false, capacity },
});

for (const [name, size] of Object.entries(SIZES)) {
  test(`after an undo the movement question with ships opens its dialog (${name})`, async ({ page }, testInfo) => {
    await page.setViewportSize(size);
    const submissions: ClientMessage[] = [];
    const game = await openMockedGame(page, {
      players: galleryPlayers,
      board,
      choice: null,
      state: { active_system: "69" },
      history: { cursor: 5, redo_count: 0, generation: 0 },
      version: 1,
    });
    await page.getByTestId("ti4-board-svg").waitFor();
    game.socket.onMessage((data) => {
      const message = JSON.parse(String(data)) as ClientMessage;
      if (message.type === "submit_choice") submissions.push(message);
    });
    const { events: _events, ...base } = game.snapshot;
    let version = 1;
    const send = (cursor: number, generation: number, choice: { nonce: string; prompt: string; options: unknown[]; subtype: string } | null) => {
      version += 1;
      game.send({
        ...base,
        type: "state_update",
        protocol_version: PROTOCOL_VERSION,
        game_id: GAME_ID,
        game_version: version,
        history: { cursor, redo_count: 0, generation },
        pending_choice: choice
          ? { nonce: choice.nonce, choice: { player: actor, prompt: choice.prompt, context: { subtype: choice.subtype } as never, options: choice.options as never } }
          : null,
      });
    };
    const done = { id: "done_moving", kind: "decline", label: "finish movement", payload: {} };

    // First try: a system that no ship reaches. The movement question is lone and the client
    // answers it (nonce "6") without opening a dialog.
    send(5, 0, null);
    send(6, 0, { nonce: "6", prompt: "movement", subtype: "movement_step", options: [done] });
    await expect.poll(() => submissions.length).toBe(1);
    expect(submissions[0]).toMatchObject({ option_id: "done_moving", nonce: "6" });
    await expect(page.getByTestId("tactical-movement-tray")).toHaveCount(0);

    // Undo (a new run of the game), the second try: activate #69, whose movement question has
    // ships to move and, as the new run counts again from 1, the nonce "6" once more.
    send(5, 1, null);
    send(6, 1, { nonce: "6", prompt: "movement", subtype: "movement_step", options: [move("move|01|0", "sol_carrier", 6), move("move|01|2", "destroyer", 0), done] });
    await expect(page.getByTestId("tactical-movement-tray")).toBeVisible();
    await expect(page.getByTestId("tactical-movement-tray")).toContainText("Destination: #69");
    await shot(page, testInfo, `1-dialog-after-undo-${name}`);
  });
}
