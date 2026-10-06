import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { AutoResolveToast } from "./AutoResolveToast.tsx";

describe("AutoResolveToast", () => {
  it("names what was chosen and why there was only one choice", () => {
    render(
      <AutoResolveToast
        notification={{
          id: "a",
          decisionType: "pay 1 more resources",
          selectedValue: "trade goods",
          reason: "it was the only way left to pay",
        }}
        onDismiss={() => {}}
      />,
    );
    const toast = screen.getByTestId("corner-toast");
    expect(toast.textContent).toContain("pay 1 more resources");
    expect(toast.textContent).toContain("trade goods");
    expect(toast.textContent).toContain("(it was the only way left to pay)");
  });

  it("falls back to a generic reason and dismisses on click", () => {
    vi.useFakeTimers();
    const onDismiss = vi.fn();
    render(
      <AutoResolveToast
        notification={{ id: "b", decisionType: "Strategy Card", selectedValue: "Technology" }}
        onDismiss={onDismiss}
      />,
    );
    expect(screen.getByTestId("corner-toast").textContent).toContain("(the only legal option)");
    fireEvent.click(screen.getByTestId("corner-toast"));
    vi.advanceTimersByTime(400);
    expect(onDismiss).toHaveBeenCalledWith("b");
    vi.useRealTimers();
  });
});
