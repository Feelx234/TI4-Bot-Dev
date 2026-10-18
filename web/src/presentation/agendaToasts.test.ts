import { describe, expect, it } from "vitest";
import {
  agendaCastText,
  agendaCastToasts,
  agendaResultText,
  agendaToasts,
  readAgendaCasts,
  readAgendaLog,
  type AgendaCastDto,
  type AgendaRecordDto,
} from "./agendaToasts.ts";

const names: Record<string, string> = { p1: "Alice", p2: "Bob" };
const seatName = (id: string) => names[id] ?? null;
const text = (record: AgendaRecordDto) => agendaResultText(record, seatName);

describe("agenda result toasts", () => {
  it("For wins a law: the law enters play, with the vote totals", () => {
    expect(
      text({ agenda: "anti_intellectual_revolution", name: "Anti-Intellectual Revolution", outcome: "for", votes: { for: 7, against: 3 }, law: true, enacted: true }),
    ).toBe("Anti-Intellectual Revolution: For wins\nVotes: For 7 · Against 3\nThe law enters play.");
  });

  it("Against wins: a law is discarded, a directive is resolved", () => {
    expect(text({ agenda: "mutiny", name: "Mutiny", outcome: "against", votes: { for: 2, against: 4 } })).toBe(
      "Mutiny: Against wins\nVotes: Against 4 · For 2\nDirective resolved.",
    );
    expect(text({ agenda: "fleet_regulations", outcome: "against", votes: { against: 1 }, law: true })).toContain(
      "The law is discarded.",
    );
  });

  it("an election names the elected player or planet by name", () => {
    expect(
      text({ agenda: "minister_peace", name: "Minister of Peace", outcome: "p2", votes: { p2: 6, p1: 3 }, law: true, enacted: true }),
    ).toBe("Minister of Peace: Bob elected\nVotes: Bob 6 · Alice 3\nThe law enters play.");
    expect(text({ agenda: "core_mining", name: "Core Mining", outcome: "mecatol_rex", votes: { mecatol_rex: 2 } })).toMatch(
      /^Core Mining: Mecatol Rex elected/,
    );
  });

  it("a tie and a silent table say the speaker decided", () => {
    expect(text({ agenda: "mutiny", name: "Mutiny", outcome: "for", votes: { for: 3, against: 3 }, speaker_decided: true })).toBe(
      "Mutiny: For wins\nTied (For 3 · Against 3): the speaker decided.\nDirective resolved.",
    );
    expect(text({ agenda: "mutiny", name: "Mutiny", outcome: "against", votes: {}, speaker_decided: true })).toContain(
      "No votes cast: the speaker decided.",
    );
  });

  it("Deadly Plot, a redirect, and an agenda with no outcome", () => {
    expect(text({ agenda: "mutiny", name: "Mutiny", outcome: "for", votes: { for: 1 }, discarded: true })).toContain(
      "Discarded by Deadly Plot: no effect.",
    );
    expect(text({ agenda: "minister_war", name: "Minister of War", outcome: "p1", votes: { p1: 4 }, redirected_to: "p2" })).toContain(
      "Its effect goes to Bob instead.",
    );
    expect(text({ agenda: "mutiny", name: "Mutiny", outcome: null })).toBe("Mutiny\nResolved with no outcome.");
  });

  it("a card discarded without a vote says why", () => {
    expect(
      text({ agenda: "classified", name: "Classified Document Leaks", outcome: null, discarded: true, discard_reason: "no_choice" }),
    ).toBe("Classified Document Leaks\nDiscarded: nothing to elect. The next agenda is revealed.");
    expect(
      text({ agenda: "standardization", name: "Standardization", outcome: null, discarded: true, discard_reason: "replaced" }),
    ).toBe("Standardization\nDiscarded and replaced by the next agenda.");
  });

  it("toasts only records past the cursor, once each", () => {
    const log: AgendaRecordDto[] = [
      { round: 2, agenda: "mutiny", outcome: "for" },
      { round: 2, agenda: "minister_peace", outcome: "p1" },
    ];
    const first = agendaToasts(log, 1, seatName);
    expect(first.toasts.map((t) => t.id)).toEqual(["agenda:1:2:minister_peace"]);
    expect(first.next).toBe(2);
    expect(agendaToasts(log, first.next, seatName).toasts).toEqual([]);
  });

  it("reads the log off the state and ignores junk", () => {
    expect(readAgendaLog({ agenda_log: [{ agenda: "mutiny", outcome: "for" }, null, { outcome: "x" }] })).toEqual([
      { agenda: "mutiny", outcome: "for" },
    ]);
    expect(readAgendaLog(null)).toEqual([]);
  });

  describe("vote count toasts", () => {
    const cast = (player: string, outcome: string | null, votes: number): AgendaCastDto => ({ player, outcome, votes });

    it("names the seat, the outcome and the votes cast", () => {
      expect(agendaCastText(cast("p2", "for", 7), [], "p1", seatName)).toBe("Bob voted For: 7 votes");
      expect(agendaCastText(cast("p2", "against", 1), [], "p1", seatName)).toBe("Bob voted Against: 1 vote");
    });

    it("says You for the viewer, and names an elected seat or planet", () => {
      expect(agendaCastText(cast("p1", "for", 4), [], "p1", seatName)).toBe("You voted For: 4 votes");
      expect(agendaCastText(cast("p2", "p1", 12), [], "p1", seatName)).toBe("Bob voted for Alice: 12 votes");
      expect(agendaCastText(cast("p1", "p2", 3), [], "p1", seatName)).toBe("You voted for Bob: 3 votes");
      expect(agendaCastText(cast("p2", "mecatol_rex", 5), [], null, seatName)).toBe("Bob voted for Mecatol Rex: 5 votes");
    });

    it("an abstention (or nothing cast) says so, without a count", () => {
      expect(agendaCastText(cast("p2", null, 0), [], "p1", seatName)).toBe("Bob abstained");
      expect(agendaCastText(cast("p1", null, 0), [], "p1", seatName)).toBe("You abstained");
      expect(agendaCastText(cast("p1", "for", 0), [], "p1", seatName)).toBe("You abstained");
    });

    it("adds the running total once a second cast makes it news", () => {
      const before = [cast("p2", "for", 7)];
      expect(agendaCastText(cast("p1", "against", 3), before, "p1", seatName)).toBe(
        "You voted Against: 3 votes\nRunning total: For 7 \u00b7 Against 3",
      );
      expect(agendaCastText(cast("p3", "for", 5), before, "p1", seatName)).toContain("Running total: For 12");
      expect(agendaCastText(cast("p3", "for", 5), [cast("p2", null, 0)], "p1", seatName)).not.toContain("Running total");
    });

    it("a large cast from several sources is one total", () => {
      expect(agendaCastText(cast("p1", "for", 21), [], "p1", seatName)).toBe("You voted For: 21 votes");
    });

    it("toasts only casts past the cursor, with ids unique per vote", () => {
      const casts = [cast("p2", "for", 7), cast("p1", null, 0)];
      const first = agendaCastToasts(casts, 1, 3, "p1", seatName);
      expect(first.toasts.map((t) => t.id)).toEqual(["agenda-cast:3:1:p1"]);
      expect(first.toasts[0]).toMatchObject({ tag: "Vote", text: "You abstained" });
      expect(first.next).toBe(2);
      expect(agendaCastToasts(casts, first.next, 3, "p1", seatName).toasts).toEqual([]);
      expect(agendaCastToasts(casts, 0, 4, "p1", seatName).toasts[0].id).toBe("agenda-cast:4:0:p2");
    });

    it("reads the casts off the state and ignores junk", () => {
      expect(readAgendaCasts({ agenda_casts: [{ player: "p1", outcome: "for", votes: 2 }, null, { votes: 1 }] })).toEqual([
        { player: "p1", outcome: "for", votes: 2 },
      ]);
      expect(readAgendaCasts(undefined)).toEqual([]);
    });
  });
});
