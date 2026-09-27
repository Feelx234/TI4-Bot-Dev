import { test, expect, type APIRequestContext } from "@playwright/test";
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
  expect(response.ok(), `snapshot: ${response.status()} ${response.ok() ? "" : await response.text()}`)
    .toBe(true);
  return response.json();
}

test("a complete human battle stays public in four independent views", async ({
  browser,
  request,
}) => {
  test.setTimeout(180_000);
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
    let sawGroupedOpportunity = false;
    let shieldHits: number | undefined;
    let shieldRound: number | undefined;
    let shieldWasSabotaged = false;
    let checkedShieldHits = false;
    let checkedShieldCallout = false;
    let sawSustain = false;
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
      const docked = actorPage!.getByTestId("resume-combat-btn");
      if (await docked.isVisible()) await docked.click();
      const wrappedDock = actorPage!.getByTestId("resume-decision-btn");
      if (await wrappedDock.isVisible()) await wrappedDock.click();
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
      if (shieldHits !== undefined && current.view.board.combat?.round !== shieldRound) {
        if (!shieldWasSabotaged && shieldHits <= 2) checkedShieldHits = true;
        shieldHits = undefined;
      }
      if (shieldHits !== undefined && (subtype === "sustain_damage" || subtype === "assign_casualty")) {
        if (!shieldWasSabotaged) {
          const remaining = Number(
            await actorPage!.getByTestId("combat-hits-callout").locator(".combat-hits-callout__count").textContent(),
          );
          expect.soft(remaining, "Shields Holding must cancel two hits before assignment").toBe(
            Math.max(0, shieldHits - 2),
          );
          checkedShieldHits = true;
        }
        shieldHits = undefined;
      }
      if (shieldHits !== undefined && shieldHits <= 2 && !shieldWasSabotaged &&
          !subtype.startsWith("reaction_") && !subtype.startsWith("play_reaction_")) {
        checkedShieldHits = true;
        shieldHits = undefined;
      }
      const card =
        options.find((opt) => opt.kind === "ability" && typeof opt.payload?.card === "string") ??
        options.find((opt) => opt.kind === "action_card") ??
        (actor === sol
          ? options.find(
              (opt) => opt.kind === "ability" && opt.label.startsWith("Choose an action card"),
            )
          : undefined);
      const hand = offer.view.players.find((player) => player.id === actor)?.held_action_cards ?? [];
      if (hand.includes("sh1") && hand.includes("sh2") && subtype.startsWith("reaction_") && subtype.includes("HITS_TO_ASSIGN")) {
        sawGroupedOpportunity = true;
        expect(options.filter((option) => option.kind === "ability")).toHaveLength(1);
        expect.soft(card?.label, "two copies of the only applicable card need one Play button")
          .toBe("Play Shields Holding");
        await expect.soft(actorPage!.getByRole("button", { name: "Play Shields Holding" }))
          .toBeVisible();
      }
      let selected: ChoiceOptionDto;
      if (card && (actor === sol || String(card.payload?.card_name) === "Sabotage")) {
        selected = card;
      } else if (subtype === "announce_retreat") {
        selected = options.find((opt) => opt.id === "stay") ?? options[0];
      } else if (subtype.startsWith("reaction_")) {
        selected = options.find((opt) => opt.id === "decline") ?? options[0];
      } else if (subtype === "sustain_damage") {
        selected = !sawSustain
          ? options.find((opt) => opt.kind === "sustain") ?? options[0]
          : options.find((opt) => opt.id === "decline") ?? options[0];
      } else {
        selected =
          options.find((opt) => opt.id !== "retreat" && opt.id !== "decline") ?? options[0];
      }
      if (selected.payload?.card_name === "Shields Holding") {
        shieldHits = current.view.board.combat?.hits_to_assign ?? undefined;
        shieldRound = current.view.board.combat?.round;
        shieldWasSabotaged = false;
      }
      if (selected.payload?.card_name === "Sabotage" && shieldHits !== undefined) {
        shieldWasSabotaged = true;
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
      if (selected.payload?.card_name === "Shields Holding" && shieldHits !== undefined &&
          after.pending_choice?.choice.context?.subtype?.includes("HITS_TO_ASSIGN") &&
          after.view.board.combat?.round === shieldRound) {
        const expectedHits = Math.max(0, shieldHits - 2);
        for (let i = 0; i < pages.length; i++) {
          const id = ids[i];
          const view = await snapshot(request, game, id ? seats[id] : undefined);
          expect(view.view.board.combat?.hits_to_assign,
            `Shields Holding must update incoming hits for ${id ?? "spectator"}`)
            .toBe(expectedHits);
          const callout = pages[i].getByTestId("combat-hits-callout");
          if (expectedHits === 0) {
            await expect(callout, `cancelled hits in view ${id ?? "spectator"}`).toBeHidden();
          } else {
            await expect(callout.locator(".combat-hits-callout__count"),
              `remaining hits in view ${id ?? "spectator"}`).toHaveText(String(expectedHits));
          }
        }
        checkedShieldCallout = true;
      }
      if (sawGroupedOpportunity && subtype.startsWith("reaction_") &&
          subtype.includes("HITS_TO_ASSIGN") && selected.label === "Play Shields Holding") {
        const next = await snapshot(request, game, seats[sol]);
        expect.soft(next.pending_choice?.choice.context?.subtype?.startsWith("play_reaction_") ?? false,
          "one Shields Holding click must not ask for a physical copy").toBe(false);
        expect.soft(next.view.players.find((player) => player.id === sol)?.held_action_cards?.length,
          "one click consumes exactly one Shields Holding copy").toBe(hand.length - 1);
      }
      const expectedDetail =
        selected.payload?.card_name && !selected.label.startsWith("Choose an action card")
          ? `played ${selected.payload.card_name}`
          : subtype === "sustain_damage" && selected.id !== "decline"
            ? "sustained damage"
            : undefined;
      if (expectedDetail) {
        if (expectedDetail === "sustained damage") sawSustain = true;
        // Check before anyone answers the next reaction, not just at battle completion.
        const newlyLogged = (after.events ?? []).filter(
          (event) => event.detail?.includes(expectedDetail) &&
            !(current.events ?? []).some((old) => old.id === event.id),
        );
        expect.soft(newlyLogged, `${expectedDetail} must be logged at this decision boundary`)
          .toHaveLength(1);
        if (newlyLogged.length === 1) for (let i = 0; i < pages.length; i++) {
          const id = ids[i];
          const view = await snapshot(request, game, id ? seats[id] : undefined);
          expect.soft(view.events?.some((event) => event.id === newlyLogged[0]?.id),
            `${expectedDetail} visible to ${id ?? "spectator"} before the next response`).toBe(true);
          // The modal intercepts clicks outside it; dock it to inspect the live log.
          const modal = pages[i].getByTestId("combat-resolution-modal");
          if (await modal.isVisible()) await modal.getByTestId("close-combat-modal").click();
          const generic = pages[i].getByTestId("pending-choice-dialog");
          if (await generic.isVisible())
            await generic.getByRole("button", { name: "Minimize decision" }).click();
          const toggle = pages[i].getByTestId("event-log-toggle");
          if (await toggle.getAttribute("aria-expanded") === "false") {
            // A new decision can open the actor's modal while the other views are checked.
            await expect(async () => {
              if (await modal.isVisible()) await modal.getByTestId("close-combat-modal").click();
              if (await generic.isVisible())
                await generic.getByRole("button", { name: "Minimize decision" }).click();
              await toggle.click({ timeout: 1_000 });
            }).toPass();
          }
          await expect.soft(pages[i].getByTestId("event-log-list"),
            `live event log for ${id ?? "spectator"}`).toContainText(expectedDetail, { timeout: 1_000 });
          const resume = pages[i].getByTestId("resume-combat-btn");
          if (await resume.isVisible()) await resume.click();
          const wrappedResume = pages[i].getByTestId("resume-decision-btn");
          if (await wrappedResume.isVisible()) await wrappedResume.click();
        }
      }
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
    expect(sawGroupedOpportunity, "the scenario must offer both copies together").toBe(true);
    expect(sawSustain, "the battle must exercise sustain damage").toBe(true);
    expect(checkedShieldHits, "Shields Holding must cancel hits before assignment").toBe(true);
    expect(checkedShieldCallout, "Shields Holding must update the live hits callout").toBe(true);
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

test("undo during a Shields Holding reaction preserves a healthy, playable game", async ({
  browser,
  request,
}) => {
  test.setTimeout(90_000);
  const launch = await request.post(`${backend}/api/dev/scenarios/launch`, {
    data: { scenario_id: "ongoing_combat_four_views", seed: 42 },
  });
  expect(launch.ok(), await launch.text()).toBe(true);
  const { game_id: game, player_id: host, test_seats: seats } = (await launch.json()) as {
    game_id: string;
    player_id: string;
    test_seats: Record<string, string>;
  };
  const contexts = await Promise.all(Object.keys(seats).map(() => browser.newContext()));
  const pages = await Promise.all(contexts.map((context) => context.newPage()));
  const views = new Map(Object.keys(seats).map((id, index) => [id, pages[index]]));
  try {
    await Promise.all(Object.keys(seats).map((id) => openPlayerGame(views.get(id)!, game, seats[id])));
    const page = views.get(host)!;

  let atShields = false;
  for (let step = 0; step < 40; step++) {
    const state = await snapshot(request, game, seats[host]);
    const actor = state.turn_status.kind === "waiting_for_decision" ? state.turn_status.seat : undefined;
    expect(actor).toBeTruthy();
    const offer = await snapshot(request, game, seats[actor!]);
    const choice = offer.pending_choice?.choice;
    expect(choice).toBeDefined();
    const subtype = choice!.context?.subtype ?? "";
    const shields = choice!.options.find((option) =>
      subtype.includes("HITS_TO_ASSIGN") &&
      (option.label === "Play Shields Holding" || option.label.startsWith("Choose an action card")),
    );
    if (shields && actor === host) {
      atShields = true;
      // Resolve the grouped card offer before rewinding the host timeline.
      await page.getByTestId(`combat-reaction-${shields.id}`).click();
      await expect.poll(async () => (await snapshot(request, game, seats[host])).game_version)
        .toBeGreaterThan(state.game_version);
      expect((await snapshot(request, game, seats[host])).pending_choice?.choice.context?.subtype ?? "")
        .not.toContain("play_reaction_");
      break;
    }
    const selected = choice!.options.find((option) => option.payload?.card_name === "Direct Hit") ??
      choice!.options.find((option) => option.id === "stay" || option.id === "decline") ??
      choice!.options[0];
    const actorPage = views.get(actor!)!;
    const selector = subtype === "announce_retreat"
      ? `retreat-opt-${selected.id}`
      : subtype.startsWith("reaction_")
        ? selected.id === "decline" ? "pass-combat-reaction-btn" :
          selected.payload?.card_name === "Direct Hit" ? "play-direct-hit-btn" : `combat-reaction-${selected.id}`
        : subtype === "sustain_damage"
          ? selected.id === "decline" ? "decline-sustain-btn" : `sustain-opt-${selected.id}`
          : subtype === "assign_casualty"
            ? `casualty-opt-${selected.id}`
          : `combat-follow-up-${selected.id}`;
    await actorPage.getByTestId(selector).first().click();
    await expect.poll(async () => (await snapshot(request, game, seats[host])).game_version)
      .toBeGreaterThan(state.game_version);
  }
  expect(atShields, "the seeded combat must open the Shields Holding reaction").toBe(true);
  const before = await snapshot(request, game, seats[host]);
  const response = await request.post(`${backend}/api/games/${game}/history`, {
    headers: { "x-ti4-player-session": seats[host] },
    data: { action: "undo", expected_version: before.game_version },
  });
  // Even a failed history request must not terminate the server.
  const health = await request.get(`${backend}/api/games/${game}/snapshot`, {
    headers: { "x-ti4-player-session": seats[host] },
  });
  expect(health.ok(), `snapshot after undo: ${health.status()}`).toBe(true);
  expect(response.ok(), `undo during Shields Holding: ${response.status()} ${await response.text()}`)
    .toBe(true);
  const restored = (await health.json()) as InitialSnapshotMsg;
  expect(restored.history?.cursor).toBeLessThan(before.history!.cursor);
  const restoredActor = restored.turn_status.kind === "waiting_for_decision"
    ? restored.turn_status.seat : undefined;
  expect(restoredActor).toBeTruthy();
  expect((await snapshot(request, game, seats[restoredActor!])).pending_choice?.choice)
    .toBeDefined();
  await expect(page.getByTestId("combat-resolution-modal")).toBeVisible();
  await expect(page.getByTestId("combat-error-banner")).toBeHidden();
  } finally {
    await Promise.all(contexts.map((context) => context.close()));
  }
});

test("replaying Direct Hit, Sabotage and Shields Holding after undo keeps the game playable", async ({
  browser,
  request,
}) => {
  test.setTimeout(110_000);
  const launch = await request.post(`${backend}/api/dev/scenarios/launch`, {
    data: { scenario_id: "ongoing_combat_four_views", seed: 42 },
  });
  expect(launch.ok(), await launch.text()).toBe(true);
  const { game_id: game, player_id: sol, test_seats: seats } = (await launch.json()) as {
    game_id: string;
    player_id: string;
    test_seats: Record<string, string>;
  };
  const ids = Object.keys(seats);
  const contexts = await Promise.all(ids.map(() => browser.newContext()));
  const pages = await Promise.all(contexts.map((context) => context.newPage()));
  const views = new Map(ids.map((id, index) => [id, pages[index]]));
  try {
    await Promise.all(ids.map((id) => openPlayerGame(views.get(id)!, game, seats[id])));
    const letnev = (await snapshot(request, game, seats[sol])).view.players
      .find((player) => player.faction === "letnev")?.id;
    expect(letnev).toBeTruthy();

    async function advanceChain(replayed: boolean) {
      const seen = { sustain: replayed, directHit: false, sabotage: false, shields: false };
      for (let step = 0; step < 55; step++) {
        const before = await snapshot(request, game, seats[sol]);
        const actor = before.turn_status.kind === "waiting_for_decision"
          ? before.turn_status.seat : undefined;
        expect(actor, `chain ${replayed ? "replayed" : "original"}, step ${step}`)
          .toBeTruthy();
        const offer = await snapshot(request, game, seats[actor!]);
        const choice = offer.pending_choice?.choice;
        expect(choice).toBeDefined();
        const subtype = choice!.context?.subtype ?? "";
        const options = choice!.options;
        const actorPage = views.get(actor!)!;
        const resume = actorPage.getByTestId("resume-combat-btn");
        if (await resume.isVisible()) await resume.click();
        const wrappedResume = actorPage.getByTestId("resume-decision-btn");
        if (await wrappedResume.isVisible()) await wrappedResume.click();

        const named = (name: string) => options.find((option) =>
          option.payload?.card_name === name || option.label === `Play ${name}`,
        );
        let selected: ChoiceOptionDto;
        if (!seen.sustain && actor === letnev && subtype === "sustain_damage") {
          selected = options.find((option) => option.kind === "sustain")!;
          expect(selected).toBeDefined();
          seen.sustain = true;
        } else if (!seen.directHit && actor === sol && named("Direct Hit")) {
          selected = named("Direct Hit")!;
          seen.directHit = true;
        } else if (!seen.sabotage && actor === letnev && named("Sabotage")) {
          selected = named("Sabotage")!;
          seen.sabotage = true;
        } else if (!seen.shields && actor === sol && subtype.includes("HITS_TO_ASSIGN")) {
          selected = named("Shields Holding") ?? options.find((option) =>
            option.label.startsWith("Choose an action card"),
          )!;
          expect(selected).toBeDefined();
          seen.shields = true;
        } else if (subtype.startsWith("play_reaction_") && actor === sol && seen.shields) {
          selected = options.find((option) => option.payload?.card_name === "Shields Holding")!;
          expect(selected).toBeDefined();
        } else if (subtype === "announce_retreat") {
          selected = options.find((option) => option.id === "stay")!;
        } else {
          selected = options.find((option) => option.id === "decline") ?? options[0];
        }

        const selector = subtype.startsWith("reaction_") || subtype.startsWith("play_reaction_")
          ? selected.id === "decline" ? "pass-combat-reaction-btn" :
            selected.payload?.card_name === "Direct Hit" ? "play-direct-hit-btn" :
              `combat-reaction-${selected.id}`
          : subtype === "announce_retreat" || subtype === "retreat_to"
            ? `retreat-opt-${selected.id}`
            : subtype === "sustain_damage"
              ? selected.id === "decline" ? "decline-sustain-btn" : `sustain-opt-${selected.id}`
              : subtype === "assign_casualty"
                ? `casualty-opt-${selected.id}`
                : `combat-follow-up-${selected.id}`;
        await actorPage.getByTestId(selector).first().click();
        await expect.poll(async () => {
          const response = await request.get(`${backend}/api/games/${game}/snapshot`, {
            headers: { "x-ti4-player-session": seats[sol] },
          });
          expect(response.ok(),
            `${replayed ? "replay" : "original"} step ${step} after ${actor} ${selected.label}: ` +
              `${response.status()} ${response.ok() ? "" : await response.text()}`,
          ).toBe(true);
          return ((await response.json()) as InitialSnapshotMsg).game_version;
        }).toBeGreaterThan(before.game_version);
        const after = await snapshot(request, game, seats[sol]);
        if (seen.sustain && seen.directHit && seen.sabotage && seen.shields &&
            (after.events ?? []).some((event) => event.detail?.endsWith("played Shields Holding")) &&
            !after.pending_choice?.choice.context?.subtype?.startsWith("play_reaction_")) {
          return after;
        }
      }
      throw new Error(`Combat never completed reaction chain: ${JSON.stringify(seen)}`);
    }

    const first = await advanceChain(false);
    const directHit = first.events?.find((event) => event.detail?.endsWith("played Direct Hit"));
    expect(directHit?.decision_count, "Direct Hit must have a rewindable decision cursor")
      .toBeGreaterThan(0);
    const cursor = directHit!.decision_count! - 1;
    const rewind = await request.post(`${backend}/api/games/${game}/history`, {
      headers: { "x-ti4-player-session": seats[sol] },
      data: { action: "restore_cursor", cursor, expected_version: first.game_version },
    });
    expect(rewind.ok(), `undo to Direct Hit: ${rewind.status()} ${await rewind.text()}`).toBe(true);
    const restored = await snapshot(request, game, seats[sol]);
    expect(restored.history?.cursor).toBe(cursor);
    expect(restored.view.players.find((player) => player.id === sol)?.held_action_cards)
      .toContain("dh1");
    const second = await advanceChain(true);
    expect(second.view.board.combat).toBeTruthy();
    expect(second.events?.some((event) => event.detail?.endsWith("played Sabotage"))).toBe(true);
    // A replay failure may close the game session or the HTTP server entirely.
    const health = await request.get(`${backend}/api/games/${game}/snapshot`, {
      headers: { "x-ti4-player-session": seats[sol] },
    });
    expect(health.ok(), `snapshot after replay: ${health.status()} ${await health.text()}`)
      .toBe(true);
  } finally {
    await Promise.all(contexts.map((context) => context.close()));
  }
});
