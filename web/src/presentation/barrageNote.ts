/**
 * What the anti-fighter barrage step did, in words, for the round-1 note. Read from data the
 * client already has: the barrage's start-of-step units, each side's hits and the factions.
 * Only a side that rolled appears in `barrage_hits`, so a side without a barrage is skipped.
 */

export interface BarrageUnit {
  owner: string;
  unit_type: string;
  planet?: string | null;
}

export interface BarrageSide {
  seat: string;
  /** Hits this side's barrage produced (after rerolls). */
  hits: number;
  /** Fighters the opponent had in the space area when the barrage was rolled. */
  enemyFighters: number;
  /** Argent Flight: Raid Formation turns hits beyond the enemy's fighters into damage. */
  raidFormation: boolean;
}

export interface BarrageLine {
  seat: string;
  tone: "info" | "none";
  text: string;
}

export function isFighterType(unitType: string): boolean {
  return unitType.toLowerCase().replace(/[-_\s]/g, "").includes("fighter");
}

export function barrageSides(
  barrageHits: Record<string, number> | undefined,
  start: BarrageUnit[] | undefined,
  seats: { attacker: string; defender: string },
  factionOf: (seat: string) => string | undefined,
): BarrageSide[] {
  const sides: BarrageSide[] = [];
  for (const [seat, other] of [
    [seats.attacker, seats.defender],
    [seats.defender, seats.attacker],
  ] as const) {
    const hits = barrageHits?.[seat];
    if (hits === undefined) continue;
    const enemyFighters = (start ?? []).filter(
      (unit) => unit.owner === other && !unit.planet && isFighterType(unit.unit_type),
    ).length;
    sides.push({
      seat,
      hits,
      enemyFighters,
      raidFormation: (factionOf(seat) ?? "").toLowerCase().includes("argent"),
    });
  }
  return sides;
}

/** One explanation per side that rolled. `name` turns a seat into its player name. */
export function barrageLines(sides: BarrageSide[], name: (seat: string) => string): BarrageLine[] {
  return sides.map((side) => {
    const who = name(side.seat);
    const excess = Math.max(0, side.hits - side.enemyFighters);
    if (side.hits === 0) {
      return { seat: side.seat, tone: "none", text: `${who}: no hits.` };
    }
    if (side.raidFormation && excess > 0) {
      return {
        seat: side.seat,
        tone: "info",
        text:
          side.enemyFighters === 0
            ? `${who}: ${side.hits} hit${side.hits === 1 ? "" : "s"}, no enemy fighters. Raid Formation: each hit damages a ship of theirs with Sustain Damage.`
            : `${who}: ${side.hits} hit${side.hits === 1 ? "" : "s"}, ${side.enemyFighters} enemy fighter${side.enemyFighters === 1 ? "" : "s"}. Raid Formation: the ${excess} excess hit${excess === 1 ? "" : "s"} damage${excess === 1 ? "s" : ""} a ship of theirs with Sustain Damage.`,
      };
    }
    if (side.enemyFighters === 0) {
      return {
        seat: side.seat,
        tone: "none",
        text: `${who}: ${side.hits} hit${side.hits === 1 ? "" : "s"}. No enemy fighters: the hits have no effect.`,
      };
    }
    if (excess > 0) {
      return {
        seat: side.seat,
        tone: "info",
        text: `${who}: ${side.hits} hits destroy ${side.enemyFighters} fighter${side.enemyFighters === 1 ? "" : "s"}; the ${excess} excess have no effect.`,
      };
    }
    return {
      seat: side.seat,
      tone: "info",
      text: `${who}: ${side.hits} hit${side.hits === 1 ? "" : "s"} destroy${side.hits === 1 ? "s" : ""} ${side.hits} enemy fighter${side.hits === 1 ? "" : "s"}.`,
    };
  });
}
