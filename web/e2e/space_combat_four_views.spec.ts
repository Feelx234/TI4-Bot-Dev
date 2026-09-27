import { test, expect, type Page, type APIRequestContext } from "@playwright/test";
import { openPlayerGame } from "./lobbyHelpers";
import type { InitialSnapshotMsg, ChoiceOptionDto } from "../src/protocol/types";

const backend = `http://127.0.0.1:${process.env.TI4_E2E_BACKEND_PORT ?? "8180"}`;

async function snapshot(
  request: APIRequestContext,
  game: string,
  token?: string,
): Promise<InitialSnapshotMsg> {
  const response = await request.get(
    `${backend}/api/games/${game}/snapshot`,
    token ? { headers: { "x-ti4-player-session": token } } : {},
  );
  expect(response.ok()).toBe(true);
  return response.json();
}

test("a complete human battle stays public in four independent views", async ({
  browser,
  request,
}) => {
  test.setTimeout(110_000);
  const launch = await request.post(`${backend}/api/dev/scenarios/launch`, {
    data: { scenario_id: "ongoing_combat_four_views", seed: 42 },
  });
  expect(launch.ok(), await launch.text()).toBe(true);
  const {
    game_id: game,
    player_id: sol,
    test_seats: seats,
  } = (await launch.json()) as {
    game_id: string;
    player_id: string;
    test_seats: Record<string, string>;
  };
  expect(Object.keys(seats)).toHaveLength(3);
  const contexts = await Promise.all(Array.from({ length: 4 }, () => browser.newContext()));
  const pages = await Promise.all(contexts.map((context) => context.newPage()));
  const ids = Object.keys(seats);
  try {
    await Promise.all(ids.map((id, index) => openPlayerGame(pages[index], game, seats[id])));
    await pages[3].goto(`/games/${game}`);
    await pages[3].getByRole("button", { name: "Watch" }).click();
    const views = new Map(ids.map((id, index) => [id, pages[index]]));
    let played = 0;
    let sawGroupedCopies = false;
    let finished = false;
    for (let step = 0; step < 180; step++) {
      const current = await snapshot(request, game, seats[sol]);
      if (!current.view.board.combat) {
        finished = true;
        break;
      }
      const actor =
        current.turn_status.kind === "waiting_for_decision" ? current.turn_status.seat : undefined;
      expect(actor).toBeTruthy();
      const actorPage = views.get(actor!);
      expect(actorPage).toBeDefined();
      const offer = await snapshot(request, game, seats[actor!]);
      const choice = offer.pending_choice?.choice;
      expect(choice).toBeDefined();
      expect(choice?.player).toBe(actor);
      // Every view sees the public fight, only the actor sees the offer.
      for (let i = 0; i < pages.length; i++) {
        const id = ids[i];
        const viewer = await snapshot(request, game, id ? seats[id] : undefined);
        expect(viewer.view.board.combat?.system_id).toBe(current.view.board.combat.system_id);
        if (id !== actor) {
          expect(viewer.pending_choice).toBeFalsy();
          expect(
            viewer.view.players.find((p) => p.id === actor)?.held_action_cards,
          ).toBeUndefined();
          await expect(pages[i].getByTestId("combat-resolution-modal")).toBeVisible();
          await expect(pages[i].getByTestId("spectator-combat-notice")).toBeVisible();
          await expect(
            pages[i]
              .getByTestId("combat-resolution-modal")
              .getByRole("button", { name: /Play (Shields Holding|Direct Hit|Sabotage)/ }),
            `step ${step}, viewer ${id ?? "spectator"}, actor ${actor}`,
          ).toHaveCount(0);
        }
      }
      if (
        choice?.context?.space_battle ||
        choice?.context?.subtype === "announce_retreat" ||
        choice?.context?.subtype === "sustain_damage" ||
        choice?.context?.subtype === "assign_casualty"
      ) {
        await expect(
          actorPage!.getByTestId("combat-resolution-modal"),
          `step ${step} ${choice?.prompt}`,
        ).toBeVisible();
      }
      const options = choice!.options;
      const subtype = choice!.context?.subtype ?? "";
      const card =
        options.find((opt) => opt.kind === "ability" && typeof opt.payload?.card === "string") ??
        options.find((opt) => opt.kind === "action_card") ??
        (actor === sol
          ? options.find(
              (opt) => opt.kind === "ability" && opt.label.startsWith("Choose an action card"),
            )
          : undefined);
      if (card?.label.startsWith("Choose an action card")) sawGroupedCopies = true;
      let selected: ChoiceOptionDto;
      if (card && (actor === sol || String(card.payload?.card_name) === "Sabotage")) {
        selected = card;
      } else if (subtype === "announce_retreat") {
        selected = options.find((opt) => opt.id === "stay") ?? options[0];
      } else if (subtype.startsWith("reaction_")) {
        selected = options.find((opt) => opt.id === "decline") ?? options[0];
      } else if (subtype === "sustain_damage") {
        selected = options.find((opt) => opt.id === "decline") ?? options[0];
      } else {
        selected =
          options.find((opt) => opt.id !== "retreat" && opt.id !== "decline") ?? options[0];
      }
      const selector =
        subtype.startsWith("reaction_") || subtype.startsWith("play_reaction_")
          ? selected.id === "decline"
            ? "pass-combat-reaction-btn"
            : selected.payload?.card_name === "Direct Hit"
              ? "play-direct-hit-btn"
              : `combat-reaction-${selected.id}`
          : subtype === "announce_retreat" || subtype === "retreat_to"
            ? `retreat-opt-${selected.id}`
            : subtype === "sustain_damage"
              ? selected.id === "decline"
                ? "decline-sustain-btn"
                : `sustain-opt-${selected.id}`
              : subtype === "assign_casualty"
                ? `casualty-opt-${selected.id}`
                : `combat-follow-up-${selected.id}`;
      if (!choice?.context && choice?.prompt.startsWith("remove a unit:")) {
        await actorPage!.getByRole("radio", { name: selected.label }).click();
        await actorPage!.getByRole("button", { name: "Confirm choice" }).click();
      } else {
        await actorPage!.getByTestId(selector).first().click();
      }
      await expect
        .poll(async () => (await snapshot(request, game, seats[sol])).game_version)
        .toBeGreaterThan(current.game_version);
      const after = await snapshot(request, game, seats[sol]);
      const facts = (after.events ?? []).filter((event) => event.detail?.includes(" played "));
      for (const fact of facts.slice(played)) {
        for (let i = 0; i < pages.length; i++) {
          const id = ids[i];
          const view = await snapshot(request, game, id ? seats[id] : undefined);
          expect(view.events?.find((event) => event.id === fact.id)?.detail).toBe(fact.detail);
        }
      }
      played = facts.length;
    }
    expect(finished, "battle must end, including its victory window").toBe(true);
    expect(sawGroupedCopies, "multiple copies must open the inner selection").toBe(true);
    expect(played).toBeGreaterThan(0);
    const snapshots = await Promise.all(ids.map((id) => snapshot(request, game, seats[id])));
    snapshots.push(await snapshot(request, game));
    for (const page of pages) await page.reload();
    const watch = pages[3].getByRole("button", { name: "Watch" });
    await expect(watch).toBeVisible();
    await watch.click();
    for (const view of snapshots) {
      expect(view.view.board.combat).toBeFalsy();
      expect(
        (view.events ?? []).filter((event) => event.detail?.includes(" played ")),
      ).toHaveLength(played);
    }
    for (const view of snapshots.slice(1)) {
      expect(view.events?.map((event) => event.detail)).toEqual(
        snapshots[0].events?.map((event) => event.detail),
      );
      expect(view.history?.cursor).toBe(snapshots[0].history?.cursor);
    }
    const publicPlays = (snapshots[0].events ?? []).filter((event) =>
      event.detail?.includes(" played "),
    );
    expect(
      publicPlays.filter((event) => event.detail?.endsWith("played Shields Holding")),
    ).toHaveLength(2);
    const holder = await snapshot(request, game, seats[sol]);
    for (const copy of ["sh1", "sh2"]) {
      expect(
        holder.view.players.find((player) => player.id === sol)?.held_action_cards,
      ).not.toContain(copy);
    }
    for (const id of ids.filter((id) => id !== sol)) {
      expect(
        (await snapshot(request, game, seats[id])).view.players.find((player) => player.id === sol)
          ?.held_action_cards,
      ).toBeUndefined();
    }
    for (const name of ["Shields Holding", "Sabotage", "Salvage"]) {
      expect(
        publicPlays.some((event) => event.detail?.endsWith(`played ${name}`)),
        `${name}: ${publicPlays.map((event) => event.detail).join("; ")}`,
      ).toBe(true);
    }
    for (const [index, page] of pages.entries()) {
      await expect(page.getByTestId("event-log-toggle"), `reloaded view ${index}`).toBeVisible();
      const recap = page.getByTestId("close-combat-modal");
      if (await recap.isVisible()) await recap.click();
      const generic = page.getByTestId("pending-choice-dialog");
      if (await generic.isVisible())
        await generic.getByRole("button", { name: "Minimize decision" }).click();
      await page.getByTestId("event-log-toggle").click();
      for (const play of publicPlays) {
        await expect(page.getByTestId("event-log-container")).toContainText(
          play.detail!.split(" played ")[1],
        );
      }
    }
  } finally {
    await Promise.all(contexts.map((context) => context.close()));
  }
});
