// The Objectives sheet: every source of a victory point against every seat. The engine says what
// each seat has from each source (`score` of the update); this only puts it in rows and words.
import type { ScoreCellView, ScoreRowView, ScoreView } from "../../model";
import type { PointSource, SessionUpdate } from "../wire";
import { plain } from "./components";
import { seated } from "./players";

/** The name of a source that the game has once. */
const FIXED: Partial<Record<PointSource["kind"], string>> = {
  secrets: "Secret objectives",
  custodians: "Custodians of Mecatol Rex",
  imperial: "Imperial · Mecatol Rex",
  support: "Support for the Throne",
};
/** What a source with a printed name is. */
const KIND: Partial<Record<PointSource["kind"], string>> = {
  agenda: "Agenda",
  relic: "Relic",
  action_card: "Action card",
  planet: "Planet",
};

export function selectScore(update: SessionUpdate): ScoreView | null {
  const facts = update.score;
  if (!facts) {
    return null;
  }
  const viewer = update.viewer;
  const me = viewer.role === "player" ? viewer.seat : null;
  // The viewer is first, then the seats by their points: the sheet cuts the last ones off.
  const players = seated(update).sort(
    (a, b) => Number(b.id === me) - Number(a.id === me) || b.victory_points - a.victory_points,
  );
  const seats = players.map((player) => player.id);
  const objective = (item: (typeof facts.public)[number]): ScoreRowView => ({
    name: item.name,
    text: plain(item.text),
    worth: item.points,
    cells: seats.map((seat) => ({ kind: item.scored.includes(seat) ? "scored" : "none" })),
  });
  const points = (
    name: string,
    of: Record<string, number>,
    from: Record<string, string[]> = {},
  ): ScoreRowView => ({
    name,
    text: "",
    worth: null,
    cells: seats.map((seat): ScoreCellView => {
      const value = of[seat] ?? 0;
      return value === 0 ? { kind: "none" } : { kind: "points", value, from: from[seat] ?? [] };
    }),
  });
  const other = facts.sources.map((source) =>
    points(
      FIXED[source.kind] ??
        (KIND[source.kind] ? `${KIND[source.kind]} · ${source.name}` : source.name),
      source.points,
      source.from,
    ),
  );
  if (facts.unexplained && Object.keys(facts.unexplained).length > 0) {
    other.push(points("Other", facts.unexplained));
  }
  return {
    target: facts.target,
    seats: players.map((player) => ({
      seat: player.id,
      points: player.victory_points,
      isMe: player.id === me,
    })),
    groups: [
      {
        title: "Public objectives · stage I",
        rows: facts.public.filter((item) => item.points < 2).map(objective),
      },
      {
        title: "Public objectives · stage II",
        rows: facts.public.filter((item) => item.points >= 2).map(objective),
      },
      { title: "Other points", rows: other },
    ],
  };
}
