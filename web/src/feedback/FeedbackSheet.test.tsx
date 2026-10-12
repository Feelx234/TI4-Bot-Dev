import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { FeedbackSheet, MAX_FEEDBACK_CHARS, type FeedbackResult } from "./FeedbackSheet.tsx";
import { FeedbackLauncher } from "./FeedbackLauncher.tsx";
import { resetBuildInfoForTests } from "../buildInfo.ts";
import { useRegisterFeedbackSource } from "./source.ts";

function health(isPublic: boolean) {
  globalThis.fetch = vi.fn(async () =>
    new Response(JSON.stringify({ status: "ok", protocol_version: 3, commit: "abc1234", public: isPublic }), {
      status: 200,
    }),
  ) as unknown as typeof fetch;
}

beforeEach(() => {
  resetBuildInfoForTests();
  health(true);
});
afterEach(cleanup);

const GameSource: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  useRegisterFeedbackSource("game", () => ({ gameId: "game_1", viewer: { role: "player", seat: "p1", playerSession: "TOKEN" } }));
  return <>{children}</>;
};

describe("FeedbackSheet", () => {
  it("cannot send an empty report", () => {
    render(<FeedbackSheet open onClose={() => undefined} send={vi.fn()} />);
    expect(screen.getByTestId("feedback-submit")).toBeDisabled();
    fireEvent.change(screen.getByTestId("feedback-text"), { target: { value: "   " } });
    expect(screen.getByTestId("feedback-submit")).toBeDisabled();
  });

  it("offers the four categories and sends category, text and game context", async () => {
    const send = vi.fn(async (): Promise<FeedbackResult> => ({ ok: true, id: "fb_1_abcd" }));
    render(
      <GameSource>
        <FeedbackSheet open onClose={() => undefined} send={send} />
      </GameSource>,
    );
    for (const name of ["bug", "confusing", "idea", "other"]) {
      expect(screen.getByTestId(`feedback-category-${name}`)).toBeInTheDocument();
    }
    fireEvent.click(screen.getByTestId("feedback-category-idea"));
    fireEvent.change(screen.getByTestId("feedback-text"), { target: { value: "  Add a map zoom  " } });
    expect(screen.getByTestId("feedback-include-game")).toBeChecked();
    fireEvent.click(screen.getByTestId("feedback-submit"));
    await waitFor(() => expect(send).toHaveBeenCalledTimes(1));
    const body = (send.mock.calls[0] as unknown as [{ category: string; text: string; context: Record<string, unknown> }])[0];
    expect(body.category).toBe("idea");
    expect(body.text).toBe("Add a map zoom");
    expect(body.context).toMatchObject({ game_id: "game_1", viewer_seat: "p1", build_commit: "abc1234" });
    expect(JSON.stringify(body)).not.toContain("TOKEN");
    expect(await screen.findByTestId("feedback-sent")).toBeInTheDocument();
    expect(screen.getByTestId("feedback-id")).toHaveTextContent("fb_1_abcd");
  });

  it("leaves the game info out when the box is unticked", async () => {
    const send = vi.fn(async (): Promise<FeedbackResult> => ({ ok: true, id: "x" }));
    render(
      <GameSource>
        <FeedbackSheet open onClose={() => undefined} send={send} />
      </GameSource>,
    );
    fireEvent.click(screen.getByTestId("feedback-include-game"));
    fireEvent.change(screen.getByTestId("feedback-text"), { target: { value: "hi" } });
    fireEvent.click(screen.getByTestId("feedback-submit"));
    await waitFor(() => expect(send).toHaveBeenCalled());
    const body = (send.mock.calls[0] as unknown as [{ context: Record<string, unknown> }])[0];
    expect(body.context).not.toHaveProperty("game_id");
    expect(body.context).not.toHaveProperty("viewer_seat");
    expect(body.context).toHaveProperty("build_commit");
  });

  it("keeps the text and shows the problem when sending fails, then allows a retry", async () => {
    const send = vi
      .fn<() => Promise<FeedbackResult>>()
      .mockResolvedValueOnce({ ok: false, message: "Too many reports in a short time. Try again later." })
      .mockResolvedValueOnce({ ok: true, id: "fb_2" });
    render(<FeedbackSheet open onClose={() => undefined} send={send} />);
    fireEvent.change(screen.getByTestId("feedback-text"), { target: { value: "boom" } });
    fireEvent.click(screen.getByTestId("feedback-submit"));
    expect(await screen.findByTestId("feedback-error")).toHaveTextContent("Too many reports");
    expect(screen.getByTestId("feedback-text")).toHaveValue("boom");
    fireEvent.click(screen.getByTestId("feedback-submit"));
    expect(await screen.findByTestId("feedback-sent")).toBeInTheDocument();
  });

  it("is disabled while sending", async () => {
    let finish: (r: FeedbackResult) => void = () => undefined;
    const send = vi.fn(() => new Promise<FeedbackResult>((resolve) => (finish = resolve)));
    render(<FeedbackSheet open onClose={() => undefined} send={send} />);
    fireEvent.change(screen.getByTestId("feedback-text"), { target: { value: "x" } });
    fireEvent.click(screen.getByTestId("feedback-submit"));
    await waitFor(() => expect(screen.getByTestId("feedback-submit")).toBeDisabled());
    fireEvent.click(screen.getByTestId("feedback-submit"));
    expect(send).toHaveBeenCalledTimes(1);
    finish({ ok: true, id: "z" });
    await screen.findByTestId("feedback-sent");
  });

  it("limits the text to 4000 characters", () => {
    render(<FeedbackSheet open onClose={() => undefined} send={vi.fn()} />);
    expect(screen.getByTestId("feedback-text")).toHaveAttribute("maxlength", String(MAX_FEEDBACK_CHARS));
    expect(MAX_FEEDBACK_CHARS).toBe(4000);
  });
});

describe("FeedbackLauncher", () => {
  it("shows the floating button on the public server and opens the sheet", async () => {
    render(<FeedbackLauncher />);
    const button = await screen.findByTestId("feedback-button");
    fireEvent.click(button);
    expect(await screen.findByTestId("feedback-sheet")).toBeInTheDocument();
  });

  it("is absent on a local server", async () => {
    health(false);
    render(<FeedbackLauncher />);
    await waitFor(() => expect(globalThis.fetch).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.queryByTestId("feedback-button")).toBeNull();
  });
});
