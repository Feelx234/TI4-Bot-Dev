import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { InvasionLandingTray } from "./InvasionLandingTray.tsx";
import { galleryBoard } from "../dev/galleryBoard.ts";
import { actor } from "../dev/decisionGalleryCases.ts";
import { partialContext } from "../test/wire.ts";
import { landingText, sameDraft } from "../presentation/landingPlan.ts";

// G'hom Sek'kus (Sardakk commander): the engine offers ground forces standing on other planets,
// carrying `from_system` / `from_planet`. They must stay separate offers in the tray.
const commanderChoice = {
  actor,
  nonce: "landing-ghom",
  prompt: "commit ground forces in 18",
  context: partialContext({ subtype: "commit_ground_forces", target: { System: "18" } }),
  options: [
    {
      id: "commit|0|jord",
      label: "land infantry on jord",
      kind: "commit",
      payload: { planet: "jord", unit: "infantry", damaged: false },
    },
    {
      id: "commit|1|jord",
      label: "land infantry on jord",
      kind: "commit",
      payload: { planet: "jord", unit: "infantry", damaged: false, from_system: "26", from_planet: "lodor" },
    },
    {
      id: "commit|2|jord",
      label: "land mech on jord",
      kind: "commit",
      payload: { planet: "jord", unit: "mech", damaged: false, from_system: "26", from_planet: "lodor" },
    },
    { id: "done_committing", label: "commit no more ground forces", kind: "decline", payload: {} },
  ],
};

const renderTray = (onSubmit = vi.fn().mockResolvedValue(undefined)) => {
  render(
    <InvasionLandingTray
      choice={commanderChoice}
      viewerSeat={actor}
      board={galleryBoard}
      onSubmit={onSubmit}
      onClose={vi.fn()}
    />,
  );
  return onSubmit;
};

it("says where a commander force comes from and keeps it out of the space stock", () => {
  renderTray();
  const origins = screen.getAllByTestId("invasion-landing-origin");
  expect(origins).toHaveLength(2);
  expect(origins[0]).toHaveTextContent(/from Lodor/);
  expect(origins[0]).toHaveTextContent(/adjacent/);
  expect(origins[0]).not.toHaveTextContent(/in space/);
  const plusLodor = screen.getByRole("button", { name: "land infantry on jord from Lodor" });
  expect(plusLodor).toBeEnabled();
  fireEvent.click(plusLodor);
  // "up to 1 ground force from each planet": the mech on the same planet can no longer be added.
  expect(screen.getByRole("button", { name: "land mech on jord from Lodor" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "land infantry on jord from Lodor" })).toBeDisabled();
  expect(screen.getByTestId("invasion-landing-tray")).toHaveTextContent(/infantry from Lodor → jord/);
});

it("submits the commander option, not the space option of the same unit and planet", async () => {
  const onSubmit = renderTray();
  // Unstage the default plan (space forces), then stage only the Lodor infantry.
  const spaceRemove = screen.getAllByRole("button", { name: "Remove infantry from jord" })[0]!;
  while (!(spaceRemove as HTMLButtonElement).disabled) fireEvent.click(spaceRemove);
  fireEvent.click(screen.getByRole("button", { name: "land infantry on jord from Lodor" }));
  fireEvent.click(screen.getByRole("button", { name: "Confirm landings" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith("commit|1|jord"));
});

it("tells landings with different starting places apart", () => {
  const space = { planet: "jord", unit: "infantry", damaged: false };
  const lodor = { ...space, fromSystem: "26", fromPlanet: "lodor" };
  expect(sameDraft([space], [lodor])).toBe(false);
  expect(landingText(lodor)).toBe("infantry from Lodor → jord");
  expect(landingText(space)).toBe("infantry → jord");
});
