import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { AgendaPeekCard } from "./AgendaPeekCard.tsx";

const peek = (key: string) => ({
  key,
  entries: [
    { place: 1, name: "Mutiny", tags: ["Law", "For/Against"], text: ["For: x."] },
    { place: 2, name: "Secret", tags: ["Directive"], text: [] },
  ],
});

describe("AgendaPeekCard", () => {
  it("lists the cards in deck order and says it is private", () => {
    render(<AgendaPeekCard peek={peek("a,b")} />);
    expect(screen.getAllByTestId("agenda-peek-entry")).toHaveLength(2);
    expect(screen.getByText("Mutiny")).toBeInTheDocument();
    expect(screen.getByLabelText("Next to be revealed")).toHaveTextContent("1");
    expect(screen.getByText(/only you can see this/)).toBeInTheDocument();
  });

  it("closes, stays closed for the same cards and returns for a new peek", () => {
    const { rerender } = render(<AgendaPeekCard peek={peek("a,b")} />);
    fireEvent.click(screen.getByTestId("agenda-peek-close"));
    expect(screen.queryByTestId("agenda-peek")).toBeNull();
    rerender(<AgendaPeekCard peek={peek("a,b")} />);
    expect(screen.queryByTestId("agenda-peek")).toBeNull();
    rerender(<AgendaPeekCard peek={peek("c,d")} />);
    expect(screen.getByTestId("agenda-peek")).toBeInTheDocument();
  });

  it("renders nothing without a peek", () => {
    render(<AgendaPeekCard peek={null} />);
    expect(screen.queryByTestId("agenda-peek")).toBeNull();
  });
});
