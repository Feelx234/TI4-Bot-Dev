import { describe, it, expect } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { TradeItemDisplay } from "./TradeItemDisplay.tsx";

// Run 01-2322: the trade desk showed "Promissory Note: support_for_throne" instead of the card name.
describe("TradeItemDisplay card names", () => {
  const name = () => screen.getByTestId("item").querySelector(".trade-item__name")?.textContent;

  it("shows Support for the Throne by name, not by its id", () => {
    render(<TradeItemDisplay testId="item" label="Promissory Note: support_for_throne" type="offer" isCard />);
    expect(name()).toBe("Support for the Throne");
  });

  it("resolves engine note aliases (cf:generic) to the printed name and card text", () => {
    render(<TradeItemDisplay testId="item" label="Promissory Note: cf:generic" type="receive" isCard />);
    expect(name()).toBe("Ceasefire");
    expect(screen.getByTestId("item")).toHaveTextContent("cannot move units to the active system");
  });

  it("keeps the owning faction of a faction-bound note", () => {
    render(<TradeItemDisplay testId="item" label="Promissory Note: ceasefire:sol" type="receive" isCard />);
    expect(name()).toBe("Sol's Ceasefire");
  });

  it("names action cards from the catalog", () => {
    render(<TradeItemDisplay testId="item" label="Action Card: arch_expedition" type="offer" isCard />);
    expect(name()).toBe("Archaeological Expedition");
  });

  it("names faction notes", () => {
    render(<TradeItemDisplay testId="item" label="Promissory Note: ra:jolnar" type="offer" isCard />);
    expect(name()).toBe("Jol-Nar's Research Agreement");
  });

  it("gives unknown cards a readable name and keeps the id in the title", () => {
    render(<TradeItemDisplay testId="item" label="Action Card: nope_card" type="offer" isCard />);
    expect(name()).toBe("Nope Card");
    expect(screen.getByTestId("item").querySelector(".trade-item__name")).toHaveAttribute(
      "title",
      "Action Card: nope_card",
    );
  });

  it("leaves non-card items as they are", () => {
    render(<TradeItemDisplay testId="item" label="2 trade goods" type="offer" />);
    expect(name()).toBe("2 trade goods");
  });

  it("says whose Support for the Throne it is, with the card text and the holder", () => {
    render(
      <TradeItemDisplay testId="item" label="Promissory Note: support_for_throne" issuerFaction="hacan" holder="Red" type="receive" isCard />,
    );
    expect(name()).toBe("Hacan's Support for the Throne");
    const item = screen.getByTestId("item");
    expect(item).toHaveTextContent("gain 1 victory point");
    expect(item).toHaveTextContent("Hacan player");
    expect(screen.getByTestId("trade-item-holder")).toHaveTextContent("Red");
    expect(item).not.toHaveTextContent("not available");
  });

  it("shows the text of a note id keyed support:<faction> (it was bare, with no text)", () => {
    render(<TradeItemDisplay testId="item" label="Promissory Note: support:jolnar" type="offer" isCard />);
    expect(name()).toBe("Jol-Nar's Support for the Throne");
    expect(screen.getByTestId("item")).not.toHaveTextContent("not available");
  });

  it("shows the Antivirus with its text, clamped with More / Less", () => {
    render(<TradeItemDisplay testId="item" label="Promissory Note: antivirus:nekro" type="offer" isCard />);
    expect(name()).toMatch(/'s Antivirus$/);
    expect(screen.getByTestId("item")).toHaveTextContent("Technological Singularity");
    const more = screen.getByRole("button", { name: "More" });
    expect(more).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(more);
    expect(screen.getByRole("button", { name: "Less" })).toHaveAttribute("aria-expanded", "true");
  });

  it("calls a concealed note a promissory note", () => {
    render(<TradeItemDisplay testId="item" label="Promissory Note: ?3" type="offer" isCard />);
    expect(name()).toBe("a promissory note");
  });
});
