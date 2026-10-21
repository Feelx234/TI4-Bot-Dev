import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import type { BoardView, PendingChoiceDto } from "../protocol/types.ts";
import { RetreatPickBar } from "../components/RetreatPickBar.tsx";
import { SystemPickConfirmBar } from "../components/SystemPickParts.tsx";
import { isRetreatPickChoice } from "../presentation/retreatPick.ts";
import { DECISION_CONTAINERS, classifyControl } from "../../e2e/smokePolicy.ts";

describe("DECISION_CONTAINERS (smoke harness candidate scan)", () => {
  it("covers the retreat destination bar, so its chips and Confirm are clickable", () => {
    // Run o1-01-2331: "retreat to which system" had #24 picked on the map and Confirm enabled, but
    // the bar was not a scanned container; the harness only re-clicked the hex 80 times.
    const choice = {
      prompt: "retreat to which system",
      actor: "me",
      nonce: "n",
      options: [
        { id: "24", kind: "retreat_to", label: "retreat to 24", payload: { system: "24" } },
        { id: "17", kind: "retreat_to", label: "retreat to 17", payload: { system: "17" } },
      ],
      context: { subtype: "retreat_to", target: { System: "30" } },
    } as unknown as PendingChoiceDto;
    const { container } = render(
      <RetreatPickBar choice={choice} selectedOptionId="24" onSubmit={async () => undefined} />,
    );
    const root = container.querySelector("[data-testid]")!;
    expect(DECISION_CONTAINERS).toContain(root.getAttribute("data-testid"));
    expect(root.querySelector('[data-testid="confirm-retreat-btn"]')).not.toBeNull();
    expect(classifyControl("confirm-retreat-btn | Confirm").commit).toBe(true);
  });

  it("scans the retreat bar for every question it stands in for", () => {
    // Runs o3-01-2331, o2-04-2336, o3-03-2339 (retreat_to) and o4-06-2338 (Skilled Retreat):
    // all four subtypes render RetreatPickBar, whose Confirm the driver never saw.
    for (const subtype of ["retreat_to", "skilled_retreat_choose_system", "foresight_destination", "ralnel_retreat_destination"]) {
      const choice = { options: [], context: { subtype } } as unknown as PendingChoiceDto;
      expect(isRetreatPickChoice(choice), subtype).toBe(true);
    }
    expect(DECISION_CONTAINERS).toContain("retreat-pick-bar");
  });

  it("covers the system-pick confirm bar inside the minimized choice banner", () => {
    // Nightly 2026-10-20: a minimized system pick showed SystemPickConfirmBar, whose Confirm the
    // driver never scanned.
    const choice = {
      prompt: "activate which system",
      actor: "me",
      nonce: "n",
      options: [{ id: "24", kind: "activate_system", label: "activate 24", payload: { system: "24" } }],
      context: { subtype: "diplomacy_pick_system" },
    } as unknown as PendingChoiceDto;
    const { container } = render(
      <SystemPickConfirmBar choice={choice} board={{ systems: { "24": {} } } as unknown as BoardView} selectedOptionId="24" onSubmit={async () => undefined} />,
    );
    const root = container.querySelector('[data-testid="system-pick-bar"]');
    expect(root, "bar rendered").not.toBeNull();
    expect(DECISION_CONTAINERS).toContain("system-pick-bar");
    expect(root!.querySelector('[data-testid="confirm-activation-btn"]')).not.toBeNull();
  });
});
