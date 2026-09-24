import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { InvasionLandingTray } from "./InvasionLandingTray.tsx";
import { galleryBoard } from "../dev/galleryBoard.ts";
import { actor } from "../dev/decisionGalleryCases.ts";

const choice = {
  actor,
  nonce: "landing-1",
  prompt: "Land on Jord",
  context: { subtype: "commit_ground_forces", target: { System: "18" } },
  options: [
    {
      id: "land|jord|infantry",
      label: "Land infantry on Jord",
      kind: "land",
      payload: { planet: "jord", unit: "infantry" },
    },
    { id: "done_landing", label: "Done landing", kind: "decline" },
  ],
};

it("stages a single offered landing, submits once and retains the exact option id", async () => {
  const onSubmit = vi.fn().mockResolvedValue(undefined);
  render(
    <InvasionLandingTray
      choice={choice}
      viewerSeat={actor}
      board={galleryBoard}
      onSubmit={onSubmit}
      onClose={vi.fn()}
    />,
  );
  expect(screen.getByRole("button", { name: "Land forces" })).toBeDisabled();
  fireEvent.click(screen.getByRole("radio"));
  expect(screen.getByRole("button", { name: "Land forces" })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "Land forces" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledExactlyOnceWith("land|jord|infantry"));
});

it("does not expose the actor landing options to another seat", () => {
  render(
    <InvasionLandingTray
      choice={choice}
      viewerSeat="other_seat"
      board={galleryBoard}
      onSubmit={vi.fn()}
      onClose={vi.fn()}
    />,
  );
  expect(screen.queryByRole("radio")).not.toBeInTheDocument();
});
