/**
 * What a staged tactical move does to the fleet pool of the active system (LRR 37, Fighter II).
 *
 * The pool limits non-fighter ships. A fighter does not count while capacity holds it; a fighter
 * the capacity cannot hold counts against the pool when it is an upgraded one ("Fighter II:
 * fighters in excess of your ships' capacity count against your fleet pool"). That is the engine's
 * arithmetic (crates/ti4-engine/src/fleet.rs `standing_with`); the movement choice reports its
 * inputs for the active system (`details.fleet_supply`, see tactical.rs `preview_moves`) and this
 * module adds what the player staged, so the gauge shows the number the engine will enforce.
 */

/** The engine's `fleet_supply` detail of a movement choice, with the inputs of the sum. */
export interface FleetInputs {
  /** Ships charged against the limit now, uncarried Fighter II included. */
  used: number;
  limit: number;
  /** Non-fighter ships in the active system. */
  ships: number;
  /** Capacity the ships there provide. */
  capacity: number;
  /** Capacity the ground forces in the space area consume. */
  carried: number;
  /** Fighters in the active system. */
  fighters: number;
  /** Fighters that a structure on a planet there supports without capacity. */
  fighterSupport: number;
  /** An upgraded fighter stands there: what capacity cannot hold is charged to the pool. */
  upgraded: boolean;
  /** The pool counts such a fighter as half a ship (Naalu). */
  half: boolean;
}

const count = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;

/** The detail, or `null` when the choice does not carry the inputs (older host, no detail). */
export function readFleetInputs(raw: unknown): FleetInputs | null {
  if (!raw || typeof raw !== "object") return null;
  const v = raw as Record<string, unknown>;
  const used = count(v.used);
  const limit = count(v.limit);
  const ships = count(v.ships);
  const capacity = count(v.capacity);
  const fighters = count(v.fighters);
  if (used === null || limit === null || ships === null || capacity === null || fighters === null) {
    return null;
  }
  return {
    used,
    limit,
    ships,
    capacity,
    carried: count(v.carried) ?? 0,
    fighters,
    fighterSupport: count(v.fighter_support) ?? 0,
    upgraded: v.fighters_upgraded === true,
    half: v.fighters_half === true,
  };
}

/**
 * Whether a fighter of this unit id moves on its own. The base Fighter has move 0, so the engine
 * never offers it a move; every other fighter id offered one (Fighter II, Hybrid Crystal Fighter II,
 * the neutral fighter) has move 2 and needs no carrier.
 */
export function fighterMovesAlone(unitType: string): boolean {
  return /fighter/i.test(unitType) && unitType.toLowerCase() !== "fighter";
}

/** An upgraded fighter: the fleet pool absorbs what capacity cannot hold. */
export const isUpgradedFighter = (unitType: string): boolean =>
  /fighter2$/i.test(unitType);

/** Naalu's Hybrid Crystal Fighter II counts as half a ship against the pool. */
export const isHalfWeightFighter = (unitType: string): boolean =>
  unitType.toLowerCase() === "naalu_fighter2";

/** What the player staged and brings to the active system. */
export interface StagedArrival {
  /** Non-fighter ships. */
  ships: number;
  /** Capacity of those ships. */
  capacity: number;
  /** Fighters, carried or moving alone, and the unit ids among them. */
  fighters: number;
  fighterTypes: string[];
}

export interface FleetProjection {
  /** The number the engine will charge against the limit: ships plus fighters beyond capacity. */
  used: number;
  /** Fighters in the active system after the move that no capacity holds (0 unless upgraded). */
  uncarriedFighters: number;
  /** Of `used`, what the uncarried fighters account for (halved for Naalu). */
  fighterCharge: number;
  /** Fighters in the active system after the move. */
  fighters: number;
}

/** `fleet.rs standing_with` for the active system, with the staged arrival added. */
export function projectFleetSupply(
  now: FleetInputs,
  staged: StagedArrival,
): FleetProjection {
  const fighters = now.fighters + staged.fighters;
  const upgraded = now.upgraded || staged.fighterTypes.some(isUpgradedFighter);
  const half = now.half || staged.fighterTypes.some(isHalfWeightFighter);
  const transport = now.capacity + staged.capacity;
  const excused = Math.min(fighters, now.fighterSupport);
  const consumed = now.carried + fighters - excused;
  const overflow = Math.max(0, consumed - transport);
  const uncarriedFighters = upgraded
    ? Math.min(overflow, Math.max(0, fighters - excused))
    : 0;
  const fighterCharge = half ? Math.ceil(uncarriedFighters / 2) : uncarriedFighters;
  return {
    used: now.ships + staged.ships + fighterCharge,
    uncarriedFighters,
    fighterCharge,
    fighters,
  };
}
