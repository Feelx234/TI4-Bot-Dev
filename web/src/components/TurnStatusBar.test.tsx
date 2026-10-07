import { expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { TurnStatusBar, stageLabel } from "./TurnStatusBar.tsx";

it("turns engine stage ids into human labels", () => {
  expect(stageLabel("activate_system")).toBe("Activate system");
  expect(stageLabel("vote_exhaust_planet")).toBe("Vote exhaust planet");
  expect(stageLabel("")).toBe("");
});

it("never shows the raw stage id in the banner", () => {
  render(
    <TurnStatusBar
      status={{
        kind: "waiting_for_decision",
        seat: "player_a",
        phase: "action",
        round: 1,
        stage: "activate_system",
      }}
      view={null}
      gameVersion={1}
      connectionStatus="connected"
      userSeat="player_a"
    />,
  );
  const banner = screen.getByTestId("turn-status-banner");
  expect(banner).toHaveTextContent("Awaiting your choice (Activate system)");
  expect(banner.textContent).not.toContain("activate_system");
});
