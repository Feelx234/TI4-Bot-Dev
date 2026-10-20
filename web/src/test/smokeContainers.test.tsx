import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import type { PendingChoiceDto } from "../protocol/types.ts";
import { RetreatPickBar } from "../components/RetreatPickBar.tsx";
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
});
