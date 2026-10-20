import { findPublicObjectiveMeta, findSecretObjectiveMeta, humanizeId } from "../protocol/contentCatalog.ts";
import type { PlayerView, PointSourceKind, ScoreFacts, TableView } from "../protocol/types.ts";

/** The points that win when the server sends no `score` facts (the engine's `VICTORY_TARGET`). */
export const DEFAULT_TARGET = 10;

export type ScoreGroup = "public" | "secret" | "custodians" | "imperial" | "support" | "agenda" | "relic" | "other";

/** One line of a seat's breakdown: where some points came from. */
export interface ScoreRow {
  key: string;
  group: ScoreGroup;
  label: string;
  /** Second line: the stage of an objective, whose notes Support for the Throne came from, ... */
  detail?: string;
  points: number;
}

/** A public objective the seat has not scored (yet): read-only, no eligibility check. */
export interface OpenObjective {
  id: string;
  name: string;
  points: number;
  /** Seats that already scored it (labels are the caller's job). */
  scoredBy: string[];
}

export interface SeatScore {
  seat: string;
  total: number;
  rows: ScoreRow[];
  /** Points no row explains (an adjustment the engine does not name); also in `rows`. */
  unexplained: number;
  isLeader: boolean;
  /** Points behind the best seat; 0 for a leader. */
  behind: number;
  /** Points still missing to the target; 0 when reached. */
  toWin: number;
  open: OpenObjective[];
  /** Secret objectives still in the seat's hand (a count; which ones is private). */
  secretsHeld: number;
}

export interface ScoreModel {
  target: number;
  seats: SeatScore[];
}

const GROUP_OF: Record<PointSourceKind, ScoreGroup> = {
  secrets: "secret",
  custodians: "custodians",
  imperial: "imperial",
  support: "support",
  agenda: "agenda",
  relic: "relic",
  action_card: "other",
  planet: "other",
  other: "other",
};

const GROUP_ORDER: ScoreGroup[] = ["public", "secret", "custodians", "imperial", "support", "agenda", "relic", "other"];

export const GROUP_TITLES: Record<ScoreGroup, string> = {
  public: "Public objectives",
  secret: "Secret objectives",
  custodians: "Custodians token",
  imperial: "Imperial",
  support: "Support for the Throne",
  agenda: "Agendas",
  relic: "Relics",
  other: "Other",
};

const stageOf = (points: number) => (points >= 2 ? "Stage II" : "Stage I");

/**
 * The score facts the server did not send, rebuilt from what the view carries: the revealed and
 * scored public objectives and each seat's scored secrets. Whatever is left of a seat's total is
 * "unexplained", so the totals still match.
 */
export function fallbackScoreFacts(players: readonly PlayerView[], table: TableView | undefined): ScoreFacts {
  const scored = table?.scored_objectives ?? {};
  const ids = new Set<string>(table?.revealed_objectives ?? []);
  for (const list of Object.values(scored)) {
    for (const id of list ?? []) if (findPublicObjectiveMeta(id)) ids.add(id);
  }
  const secrets: Record<string, number> = {};
  for (const p of players) {
    const n = p.scored_secret_objectives?.length ?? 0;
    if (n) secrets[p.id] = n;
  }
  const explained = new Map<string, number>();
  const bump = (seat: string, n: number) => explained.set(seat, (explained.get(seat) ?? 0) + n);
  const pub = [...ids].map((id) => {
    const meta = findPublicObjectiveMeta(id);
    const by = players.filter((p) => (scored[p.id] ?? []).includes(id)).map((p) => p.id);
    for (const seat of by) bump(seat, meta?.points ?? 1);
    return { id, name: meta?.name ?? humanizeId(id), points: meta?.points ?? 1, text: "", scored: by };
  });
  for (const [seat, n] of Object.entries(secrets)) bump(seat, n);
  const unexplained: Record<string, number> = {};
  for (const p of players) {
    const rest = p.victory_points - (explained.get(p.id) ?? 0);
    if (rest !== 0) unexplained[p.id] = rest;
  }
  return {
    target: DEFAULT_TARGET,
    public: pub,
    sources: [{ kind: "secrets", name: "", points: secrets }],
    unexplained,
  };
}

/**
 * Per-seat breakdown of the victory points. Totals are the engine's `victory_points`; the rows are
 * the server's score facts (public objectives, secrets, custodians, Imperial, Support for the
 * Throne, agendas, relics, anything else), so the rows add up to the total.
 */
export function buildScoreModel(input: {
  players: readonly PlayerView[];
  table?: TableView;
  score?: ScoreFacts | null;
  label: (seat: string) => string;
}): ScoreModel {
  const { players, table, label } = input;
  const facts = input.score ?? fallbackScoreFacts(players, table);
  const best = Math.max(0, ...players.map((p) => p.victory_points));
  const seats = players.map((player): SeatScore => {
    const seat = player.id;
    const rows: ScoreRow[] = [];
    for (const objective of facts.public) {
      if (!objective.scored.includes(seat)) continue;
      rows.push({
        key: `public:${objective.id}`,
        group: "public",
        label: objective.name,
        detail: stageOf(objective.points),
        points: objective.points,
      });
    }
    for (const source of facts.sources) {
      const points = source.points[seat] ?? 0;
      if (points === 0) continue;
      const group = GROUP_OF[source.kind] ?? "other";
      if (source.kind === "secrets") {
        // Scored secrets are public; list them by name, then any the view cannot name (plots).
        const named = player.scored_secret_objectives ?? [];
        for (const id of named) {
          rows.push({
            key: `secret:${id}`,
            group,
            label: findSecretObjectiveMeta(id)?.name ?? humanizeId(id),
            detail: "Scored secret",
            points: 1,
          });
        }
        const rest = points - named.length;
        if (rest !== 0)
          rows.push({ key: "secret:more", group, label: rest > 0 ? "More secret objectives" : "Secret adjustment", points: rest });
        continue;
      }
      const from = source.from?.[seat];
      rows.push({
        key: `${source.kind}:${source.name}`,
        group,
        label: source.name || (source.kind === "imperial" ? "Imperial strategy card" : GROUP_TITLES[group]),
        detail:
          source.kind === "support" && from?.length
            ? `Note from ${from.map(label).join(", ")}`
            : source.kind === "imperial"
              ? "Primary ability, Mecatol Rex controlled"
              : source.kind === "custodians"
                ? "Took the custodians token"
                : undefined,
        points,
      });
    }
    const unexplained = facts.unexplained?.[seat] ?? 0;
    if (unexplained !== 0)
      rows.push({ key: "unexplained", group: "other", label: "Other adjustments", detail: "Not named by the game", points: unexplained });
    rows.sort((a, b) => GROUP_ORDER.indexOf(a.group) - GROUP_ORDER.indexOf(b.group));
    const total = Math.max(0, player.victory_points);
    return {
      seat,
      total,
      rows,
      unexplained,
      isLeader: total > 0 && total === best,
      behind: best - total,
      toWin: Math.max(0, facts.target - total),
      open: facts.public
        .filter((o) => !o.scored.includes(seat))
        .map((o) => ({ id: o.id, name: o.name, points: o.points, scoredBy: o.scored })),
      secretsHeld: player.secret_objectives_count ?? 0,
    };
  });
  return { target: facts.target, seats };
}
