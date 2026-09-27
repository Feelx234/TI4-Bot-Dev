import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { galleryBoard } from "../dev/galleryBoard.ts";
import { InvasionOverlay } from "./InvasionOverlay.tsx";

it("shows the same planet-local resolved ground round without offering spectator controls", () => {
  const board = {
    ...galleryBoard,
    invasion: {
      system_id: "18",
      invasion_seq: 4,
      invader: "attacker",
      phase: "ground_battle",
      planets: ["jord"],
      current_planet: "jord",
      defender: "defender",
      ground_round: 2,
      last_step: {
        planet: "jord",
        kind: "ground_round",
        round: 1,
        harrow_hits: 0,
        before: [{ owner: "attacker", unit_type: "infantry", planet: "jord", damaged: false }],
        after: [],
        hits: { attacker: 1, defender: 1 },
        dice: [
          {
            planet: "jord",
            player: "attacker",
            group: "combat value 8",
            face: 9,
            target: 8,
            hit: true,
          },
        ],
      },
    },
  };
  render(
    <InvasionOverlay
      board={board}
      choice={null}
      viewerSeat={null}
      onSubmit={vi.fn()}
      onClose={vi.fn()}
    />,
  );
  expect(screen.getByTestId("invasion-step")).toHaveTextContent("jord · ground round · round 1");
  expect(screen.getByTestId("invasion-step")).toHaveTextContent(
    "attacker · combat value 8: 9 / 8 hit",
  );
  expect(screen.getByTestId("invasion-step")).toHaveTextContent("Before: attacker infantry");
  expect(screen.getByTestId("invasion-step")).toHaveTextContent("After: None");
  expect(screen.queryByRole("button", { name: "Fight next round" })).not.toBeInTheDocument();
});
