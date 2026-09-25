import React, { useMemo, useState, useEffect } from "react";
import {
  PendingChoiceDto,
  PlayerView,
  BoardView,
  PlacedUnitView,
  CombatDieRoll,
} from "../protocol/types.ts";
import { getCombatPayload, ChoiceRendererModel } from "../presentation/choiceModel.ts";
import { Dialog } from "../primitives/index.ts";
import { WorkflowShell } from "./WorkflowShell.tsx";
import { usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";
import { DecisionHeader } from "./DecisionHeader.tsx";
import { UnitIcon, getUnitBaseType, getUnitDisplayName } from "./UnitIcon.tsx";
import { BattleOddsResponse } from "../protocol/advisorTypes.ts";
import { fetchBattleOdds, buildBattleRequest } from "../services/advisorService.ts";

export interface SpaceCombatOverlayProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
  viewerSeat?: string | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose: () => void;
  lastError?: string | null;
  recentDiceRolls?: CombatDieRoll[];
  board?: BoardView | null;
  players?: Record<string, PlayerView> | PlayerView[] | null;
  activeSystemId?: string | null;
  isMinimized?: boolean;
  onMinimize?: (minimized: boolean) => void;
  advisorUrl?: string;
}

/** Standard base transport capacity per ship type in TI4 */
export function getBaseCapacity(unitType: string): number {
  const base = getUnitBaseType(unitType);
  switch (base) {
    case "carrier":
      return 4;
    case "warsun":
      return 6;
    case "dreadnought":
      return 1;
    case "flagship":
      return 3;
    default:
      return 0;
  }
}

/** Returns true if unit is a non-fighter ship that consumes fleet supply */
export function isNonFighterShip(unitType: string): boolean {
  const base = getUnitBaseType(unitType);
  return (
    base === "warsun" ||
    base === "flagship" ||
    base === "dreadnought" ||
    base === "carrier" ||
    base === "cruiser" ||
    base === "destroyer"
  );
}

/** Returns true if unit is transported / requires capacity in space */
export function requiresCapacity(unitType: string): boolean {
  const base = getUnitBaseType(unitType);
  return base === "fighter" || base === "infantry" || base === "mech";
}

/** Heuristic combat expected hits per unit in live space combat */
function getExpectedHits(unitType: string): number {
  const base = getUnitBaseType(unitType);
  switch (base) {
    case "warsun":
      return 2.4; // 3 dice @ 3+ (3 * 0.8)
    case "flagship":
      return 1.2; // 2 dice @ 5+ (2 * 0.6)
    case "dreadnought":
      return 0.6; // 1 die @ 5+
    case "cruiser":
      return 0.4; // 1 die @ 7+
    case "destroyer":
      return 0.2; // 1 die @ 9+
    case "fighter":
      return 0.2; // 1 die @ 9+
    default:
      return 0;
  }
}

export interface FleetStats {
  nonFighterCount: number;
  fleetSupply: number;
  isOverFleetSupply: boolean;
  totalCapacity: number;
  usedCapacity: number;
  isOverCapacity: boolean;
  units: PlacedUnitView[];
  groupedUnits: {
    unitType: string;
    count: number;
    damagedCount: number;
    undamagedCount: number;
  }[];
}

export function computeFleetStats(
  units: PlacedUnitView[],
  fleetSupply = 3,
): FleetStats {
  const spaceUnits = units.filter((u) => !u.planet);
  let nonFighterCount = 0;
  let totalCapacity = 0;
  let usedCapacity = 0;

  const groupMap = new Map<
    string,
    { unitType: string; count: number; damagedCount: number; undamagedCount: number }
  >();

  for (const u of spaceUnits) {
    const isShip = isNonFighterShip(u.unit_type);
    if (isShip) {
      nonFighterCount += 1;
    }
    totalCapacity += getBaseCapacity(u.unit_type);
    if (requiresCapacity(u.unit_type)) {
      usedCapacity += 1;
    }

    const normType = u.unit_type.toLowerCase();
    const existing = groupMap.get(normType);
    if (existing) {
      existing.count += 1;
      if (u.damaged) {
        existing.damagedCount += 1;
      } else {
        existing.undamagedCount += 1;
      }
    } else {
      groupMap.set(normType, {
        unitType: u.unit_type,
        count: 1,
        damagedCount: u.damaged ? 1 : 0,
        undamagedCount: u.damaged ? 0 : 1,
      });
    }
  }

  return {
    nonFighterCount,
    fleetSupply,
    isOverFleetSupply: nonFighterCount > fleetSupply,
    totalCapacity,
    usedCapacity,
    isOverCapacity: usedCapacity > totalCapacity,
    units: spaceUnits,
    groupedUnits: Array.from(groupMap.values()),
  };
}

export const SpaceCombatOverlay: React.FC<SpaceCombatOverlayProps> = ({
  choice,
  model,
  viewerSeat,
  onSubmit,
  isOpen,
  onClose,
  lastError,
  recentDiceRolls,
  board,
  players,
  activeSystemId,
  isMinimized = false,
  onMinimize,
  advisorUrl,
}) => {
  const display = usePlayerIdentity();

  // Normalize players map
  const playersMap = useMemo<Record<string, PlayerView>>(() => {
    if (!players) return {};
    if (Array.isArray(players)) {
      return Object.fromEntries(players.map((p) => [p.id, p]));
    }
    return players;
  }, [players]);

  const subtype = choice?.context?.subtype ?? "";
  const constraints = model?.outstanding?.[0] ?? choice?.context?.outstanding?.[0];
  const hitsOwed =
    model?.selectionMode.mode === "casualty"
      ? model.selectionMode.hitsToAssign
      : model?.selectionMode.mode === "sustain"
        ? model.selectionMode.hitsRemaining
        : constraints?.amount ?? board?.combat?.hits_to_assign;

  const isSustainStage = subtype === "sustain_damage" || model?.workflow === "combat_sustain";
  const isCasualtyStage =
    subtype === "assign_casualty" ||
    model?.workflow === "combat_casualty" ||
    choice?.options.some((o) => o.kind === "casualty");
  const isRetreatStage =
    subtype === "announce_retreat" ||
    subtype === "retreat_to" ||
    model?.workflow === "combat_retreat";

  // Identify system where combat is taking place
  const combatSystemId =
    board?.combat?.system_id ??
    (choice?.context?.target && "System" in choice.context.target
      ? choice.context.target.System
      : null) ??
    activeSystemId ??
    board?.active_system ??
    "Combat";

  // Identify units in combat system
  const systemUnits = useMemo(() => {
    if (!board?.systems || !combatSystemId) return [];
    return board.systems[combatSystemId]?.units ?? [];
  }, [board, combatSystemId]);

  // Discover sides
  const { attackerSeat, defenderSeat } = useMemo(() => {
    if (board?.combat) {
      return {
        attackerSeat: board.combat.attacker,
        defenderSeat: board.combat.defender,
      };
    }
    const spaceUnits = systemUnits.filter((u) => !u.planet);
    const owners = Array.from(new Set(spaceUnits.map((u) => u.owner)));
    const active = choice?.actor ?? owners[0] ?? "Attacker";
    const opponent = owners.find((o) => o !== active) ?? owners[1] ?? "Defender";
    return { attackerSeat: active, defenderSeat: opponent };
  }, [board?.combat, systemUnits, choice]);

  // Compute fleet stats for both sides
  const attackerStats = useMemo(() => {
    const units = systemUnits.filter((u) => u.owner === attackerSeat);
    const supply = playersMap[attackerSeat]?.fleet_tokens ?? 3;
    return computeFleetStats(units, supply);
  }, [systemUnits, attackerSeat, playersMap]);

  const defenderStats = useMemo(() => {
    const units = systemUnits.filter((u) => u.owner === defenderSeat);
    const supply = playersMap[defenderSeat]?.fleet_tokens ?? 3;
    return computeFleetStats(units, supply);
  }, [systemUnits, defenderSeat, playersMap]);

  // Fallback heuristic combat odds
  const combatOdds = useMemo(() => {
    const attExp = attackerStats.units.reduce((acc, u) => acc + getExpectedHits(u.unit_type), 0);
    const defExp = defenderStats.units.reduce((acc, u) => acc + getExpectedHits(u.unit_type), 0);
    const attHp = attackerStats.units.length;
    const defHp = defenderStats.units.length;

    const attPower = (attExp + 0.1) * (attHp + 0.1);
    const defPower = (defExp + 0.1) * (defHp + 0.1);
    const totalPower = attPower + defPower;

    const attWinPct = totalPower > 0 ? Math.round((attPower / totalPower) * 100) : 50;
    const defWinPct = 100 - attWinPct;

    return {
      attExp: attExp.toFixed(1),
      defExp: defExp.toFixed(1),
      attWinPct,
      defWinPct,
    };
  }, [attackerStats, defenderStats]);

  // Live simulation odds from ti4-advisor
  const [advisorOdds, setAdvisorOdds] = useState<BattleOddsResponse | null>(null);
  const [isAdvisorLoading, setIsAdvisorLoading] = useState(false);
  const [advisorFailed, setAdvisorFailed] = useState(false);

  const battleParams = useMemo(() => {
    const attackerUnits: Record<string, number> = {};
    const attackerDamaged: Record<string, number> = {};
    for (const g of attackerStats.groupedUnits) {
      const base = getUnitBaseType(g.unitType);
      if (base === "spacedock" || base === "pds") continue;
      attackerUnits[base] = (attackerUnits[base] ?? 0) + g.count;
      if (g.damagedCount > 0) {
        attackerDamaged[base] = (attackerDamaged[base] ?? 0) + g.damagedCount;
      }
    }

    const defenderUnits: Record<string, number> = {};
    const defenderDamaged: Record<string, number> = {};
    for (const g of defenderStats.groupedUnits) {
      const base = getUnitBaseType(g.unitType);
      if (base === "spacedock" || base === "pds") continue;
      defenderUnits[base] = (defenderUnits[base] ?? 0) + g.count;
      if (g.damagedCount > 0) {
        defenderDamaged[base] = (defenderDamaged[base] ?? 0) + g.damagedCount;
      }
    }

    return {
      attackerFaction: playersMap[attackerSeat]?.faction,
      attackerUnits,
      attackerDamaged,
      defenderFaction: playersMap[defenderSeat]?.faction,
      defenderUnits,
      defenderDamaged,
    };
  }, [attackerStats, defenderStats, playersMap, attackerSeat, defenderSeat]);

  const battleKey = useMemo(() => JSON.stringify(battleParams), [battleParams]);

  useEffect(() => {
    if (!isOpen && !choice) return;

    const hasAttackerUnits = Object.keys(battleParams.attackerUnits).length > 0;
    const hasDefenderUnits = Object.keys(battleParams.defenderUnits).length > 0;

    if (!hasAttackerUnits || !hasDefenderUnits) {
      setAdvisorOdds(null);
      setIsAdvisorLoading(false);
      setAdvisorFailed(false);
      return;
    }

    let active = true;
    const controller = new AbortController();
    setIsAdvisorLoading(true);
    setAdvisorFailed(false);

    const request = buildBattleRequest({
      ...battleParams,
      simulations: 2000,
    });

    fetchBattleOdds(request, { signal: controller.signal, baseUrl: advisorUrl })
      .then((res) => {
        if (!active) return;
        setAdvisorOdds(res);
        setIsAdvisorLoading(false);
      })
      .catch((_err: unknown) => {
        if (!active || controller.signal.aborted) return;
        setIsAdvisorLoading(false);
        setAdvisorFailed(true);
      });

    return () => {
      active = false;
      controller.abort();
    };
  }, [isOpen, Boolean(choice), battleKey, advisorUrl]);

  // Merged odds for rendering
  const displayedOdds = useMemo(() => {
    if (advisorOdds) {
      const attWinPct = Math.round(advisorOdds.attacker_win_rate * 100);
      const defWinPct = Math.round(advisorOdds.defender_win_rate * 100);
      const mutWinPct = Math.round(advisorOdds.mutual_destruction_rate * 100);
      const attExpSurvivors = Object.values(advisorOdds.attacker_expected_survivors)
        .reduce((a, b) => a + b, 0)
        .toFixed(1);
      const defExpSurvivors = Object.values(advisorOdds.defender_expected_survivors)
        .reduce((a, b) => a + b, 0)
        .toFixed(1);

      return {
        attWinPct,
        defWinPct,
        mutWinPct,
        attSub: `~${attExpSurvivors} survivors`,
        defSub: `~${defExpSurvivors} survivors`,
        avgRounds: advisorOdds.average_rounds.toFixed(1),
        tag: `Simulated (${advisorOdds.simulations.toLocaleString()} rollouts)`,
        tagClass: "combat-odds-card__tag--simulated",
      };
    }

    return {
      attWinPct: combatOdds.attWinPct,
      defWinPct: combatOdds.defWinPct,
      mutWinPct: 0,
      attSub: `~${combatOdds.attExp} exp hits`,
      defSub: `~${combatOdds.defExp} exp hits`,
      avgRounds: null,
      tag: isAdvisorLoading
        ? "Simulating..."
        : advisorFailed
          ? "Estimated (Advisor Offline)"
          : "Preview Heuristic",
      tagClass: isAdvisorLoading
        ? "combat-odds-card__tag--loading"
        : advisorFailed
          ? "combat-odds-card__tag--offline"
          : "",
    };
  }, [advisorOdds, combatOdds, isAdvisorLoading, advisorFailed]);

  // Gather dice feed
  const activeDiceFeed: CombatDieRoll[] = useMemo(() => {
    if (recentDiceRolls && recentDiceRolls.length > 0) return recentDiceRolls;
    if (board?.combat?.dice_rolls && board.combat.dice_rolls.length > 0) {
      return board.combat.dice_rolls;
    }
    return [];
  }, [recentDiceRolls, board?.combat?.dice_rolls]);

  const casualtyOptions =
    choice?.options.filter((o) => o.id !== "decline" && o.kind !== "decline") ?? [];

  const attackerHitsDealt = useMemo(() => {
    if (typeof board?.combat?.attacker_hits === "number") {
      return board.combat.attacker_hits;
    }
    return activeDiceFeed.filter((d) => d.hit && (d.player ? d.player === attackerSeat : true)).length;
  }, [board?.combat?.attacker_hits, activeDiceFeed, attackerSeat]);

  const defenderHitsDealt = useMemo(() => {
    if (typeof board?.combat?.defender_hits === "number") {
      return board.combat.defender_hits;
    }
    return activeDiceFeed.filter((d) => d.hit && (d.player ? d.player === defenderSeat : false)).length;
  }, [board?.combat?.defender_hits, activeDiceFeed, defenderSeat]);

  const attackerPlayer = playersMap[attackerSeat];
  const defenderPlayer = playersMap[defenderSeat];
  const attackerHasDirectHit = Boolean(attackerPlayer?.held_action_cards?.includes("direct_hit"));
  const defenderHasDirectHit = Boolean(defenderPlayer?.held_action_cards?.includes("direct_hit"));
  const isAttackerDeciding = choice?.actor === attackerSeat;
  const opponentHasDirectHit = isAttackerDeciding ? defenderHasDirectHit : attackerHasDirectHit;
  const activeHasDirectHit = isAttackerDeciding ? attackerHasDirectHit : defenderHasDirectHit;

  function matchesUnitType(groupType: string, optUnit: string | undefined, optLabel: string): boolean {
    const target = (optUnit ?? optLabel).toLowerCase();
    const groupNorm = groupType.toLowerCase();
    const baseGroup = getUnitBaseType(groupType).toLowerCase();
    const baseTarget = getUnitBaseType(target).toLowerCase();
    return (
      baseGroup === baseTarget ||
      groupNorm.includes(baseTarget) ||
      target.includes(baseGroup) ||
      groupNorm === target
    );
  }

  const renderFleetUnits = (
    stats: FleetStats,
    seat: string,
    isActor: boolean,
    isDirectSubmitting: boolean,
    submitDirect: (optionId: string) => Promise<void>,
  ) => {
    if (stats.groupedUnits.length === 0) {
      return <div className="combat-empty-fleet">No ships remaining</div>;
    }

    const isCurrentDecider = isActor && choice?.actor === seat;

    const matchedSustainIds = new Set<string>();

    return stats.groupedUnits.map((group) => {
      // Casualty stage options for this group
      const matchingCasualties =
        isCurrentDecider && isCasualtyStage
          ? casualtyOptions.filter((opt) => {
              const payload = getCombatPayload(opt);
              return matchesUnitType(group.unitType, payload.unit, opt.label);
            })
          : [];

      // Sustain stage options for this group
      const matchingSustains =
        isCurrentDecider && isSustainStage
          ? (choice?.options ?? [])
              .filter((o) => o.id !== "decline" && o.kind !== "decline")
              .filter((opt) => {
                const payload = getCombatPayload(opt);
                return matchesUnitType(group.unitType, payload.unit, opt.label);
              })
          : [];

      for (const opt of matchingSustains) {
        matchedSustainIds.add(opt.id);
      }

      const isCasualtyInteractive = matchingCasualties.length > 0;
      const isSustainInteractive = matchingSustains.length > 0;

      const handleRowClick = () => {
        if (isDirectSubmitting) return;
        if (matchingCasualties.length === 1) {
          void submitDirect(matchingCasualties[0].id);
        } else if (matchingCasualties.length > 1) {
          const damagedOpt = matchingCasualties.find(
            (o) => getCombatPayload(o).damaged || o.label.toLowerCase().includes("damaged"),
          );
          void submitDirect((damagedOpt ?? matchingCasualties[0]).id);
        }
      };

      return (
        <div
          key={group.unitType}
          className={`combat-unit-row ${isCasualtyInteractive ? "combat-unit-row--interactive combat-unit-row--casualty" : ""}`}
          data-testid={`unit-row-${group.unitType}`}
          role={isCasualtyInteractive ? "button" : undefined}
          tabIndex={isCasualtyInteractive ? 0 : undefined}
          onClick={isCasualtyInteractive ? handleRowClick : undefined}
          onKeyDown={
            isCasualtyInteractive
              ? (e) => {
                  if (e.key === "Enter" || e.key === " ") handleRowClick();
                }
              : undefined
          }
        >
          <UnitIcon type={group.unitType} size={20} />
          <span className="combat-unit-row__name">
            {getUnitDisplayName(group.unitType, group.count)}
          </span>
          <span className="combat-unit-row__count">×{group.count}</span>
          {group.damagedCount > 0 && (
            <span className="combat-unit-row__damaged-badge" title="Sustained Damage">
              ⚠️ {group.damagedCount} damaged
            </span>
          )}

          {/* Integrated Casualty Actions */}
          {isCasualtyInteractive && (
            <div className="combat-unit-row__actions">
              {matchingCasualties.length === 1 ? (
                <span className="combat-unit-row__click-hint">💥 Click to assign</span>
              ) : (
                matchingCasualties.map((opt) => {
                  const payload = getCombatPayload(opt);
                  const isDamaged = payload.damaged || opt.label.toLowerCase().includes("damaged");
                  return (
                    <button
                      key={opt.id}
                      type="button"
                      disabled={isDirectSubmitting}
                      className={`combat-unit-row__action-btn ${isDamaged ? "combat-unit-row__action-btn--damaged" : ""}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        void submitDirect(opt.id);
                      }}
                    >
                      💥 {isDamaged ? "Destroy Damaged" : "Destroy Fresh"}
                    </button>
                  );
                })
              )}
            </div>
          )}

          {/* Integrated Sustain Actions */}
          {isSustainInteractive && (
            <div className="combat-unit-row__actions">
              {matchingSustains.map((opt) => (
                <button
                  key={opt.id}
                  type="button"
                  data-testid={`sustain-opt-${opt.id}`}
                  disabled={isDirectSubmitting}
                  className="combat-unit-row__sustain-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    void submitDirect(opt.id);
                  }}
                >
                  🛡️ Sustain
                </button>
              ))}
            </div>
          )}
        </div>
      );
    });
  };

  // When docked / minimized, render a non-intrusive sticky pill
  if (isMinimized) {
    const isActor = choice && (!viewerSeat || choice.actor === viewerSeat);
    return (
      <div
        className="combat-arena-dock choice-banner choice-minimized-pill"
        data-testid="combat-docked-pill"
      >
        <div className="combat-arena-dock__info">
          <span className="combat-arena-dock__badge">⚔️ SPACE COMBAT</span>
          <span className="combat-arena-dock__system">
            System {combatSystemId} — {display(attackerSeat).label} vs {display(defenderSeat).label}
          </span>
          {hitsOwed != null && (
            <span className="combat-arena-dock__hits">({hitsOwed} hits to resolve)</span>
          )}
          {isActor && (
            <span className="combat-arena-dock__alert-pill">Your Decision Required</span>
          )}
        </div>
        <button
          type="button"
          data-testid="resume-combat-btn"
          onClick={() => onMinimize?.(false)}
          className="button button--primary button--sm"
        >
          {isActor ? "Resume Decision" : "View Combat"}
        </button>
      </div>
    );
  }

  if (!isOpen && !choice) return null;

  return (
    <Dialog.Root
      open={isOpen}
      onOpenChange={(open) => {
        if (!open) {
          onMinimize ? onMinimize(true) : onClose();
        }
      }}
    >
      <Dialog.Content
        data-testid="combat-resolution-modal"
        className="combat-dialog combat-arena-dialog choice-workflow-dialog"
      >
        <div className="panel choice-workflow-modal combat-arena-panel">
          {/* Header */}
          <Dialog.Title as="h2" className="visually-hidden">
            {choice?.prompt || `Space Combat in System ${combatSystemId}`}
          </Dialog.Title>
          <DecisionHeader
            actor={choice?.actor || attackerSeat}
            title={
              isSustainStage
                ? "Sustain damage"
                : isCasualtyStage
                  ? "Assign a casualty"
                  : isRetreatStage
                    ? subtype === "retreat_to"
                      ? "Choose a retreat destination"
                      : "Announce retreat"
                    : choice?.prompt || `Space Combat — System ${combatSystemId}`
            }
            instruction={choice?.prompt || "Space combat in progress"}
            onMinimize={() => (onMinimize ? onMinimize(true) : onClose())}
            titleTestId="combat-stage-title"
            minimizeTestId="close-combat-modal"
          />

          {choice ? (
            <WorkflowShell
              choice={choice}
              model={model}
              viewerSeat={viewerSeat}
              onSubmit={onSubmit}
              lastError={lastError}
              spectatorNotice={`Observing combat resolution in progress for ${display(choice.actor).label}...`}
              spectatorNoticeTestId="spectator-combat-notice"
              errorTestId="combat-error-banner"
            >
              {({ isActor, isDirectSubmitting, declineOption, submitDirect }) => (
                <>
                  {/* Both Sides Arena Grid */}
                  <div className="combat-arena-sides-grid" data-testid="combat-arena-sides">
                    {/* Attacker Side */}
                    <div
                      className={`combat-fleet-card ${choice?.actor === attackerSeat ? "combat-fleet-card--active" : ""}`}
                      data-testid="attacker-fleet-card"
                    >
                      <div className="combat-fleet-card__header">
                        <span className="combat-fleet-card__role">ATTACKER</span>
                        <span className="combat-fleet-card__name">{display(attackerSeat).label}</span>
                        {playersMap[attackerSeat] && (
                          <span className="combat-fleet-card__faction">
                            ({playersMap[attackerSeat].faction})
                          </span>
                        )}
                        <span className="combat-fleet-card__hits" data-testid="attacker-hits-dealt">
                          💥 {attackerHitsDealt} hit{attackerHitsDealt === 1 ? "" : "s"}
                        </span>
                      </div>

                      {/* Gauges: Fleet Supply and Capacity */}
                      <div className="combat-fleet-card__gauges">
                        <div
                          className="combat-gauge-stat"
                          data-alert={attackerStats.isOverFleetSupply}
                          title="Non-fighter ships vs Fleet Supply pool"
                          data-testid="attacker-fleet-supply-gauge"
                        >
                          <span className="combat-gauge-stat__label">Fleet Supply:</span>
                          <span className="combat-gauge-stat__value">
                            {attackerStats.nonFighterCount} / {attackerStats.fleetSupply}
                          </span>
                        </div>
                        <div
                          className="combat-gauge-stat"
                          data-alert={attackerStats.isOverCapacity}
                          title="Capacity consumed by fighters/infantry vs total transport capacity"
                          data-testid="attacker-capacity-gauge"
                        >
                          <span className="combat-gauge-stat__label">Capacity:</span>
                          <span className="combat-gauge-stat__value">
                            {attackerStats.usedCapacity} / {attackerStats.totalCapacity}
                          </span>
                        </div>
                      </div>

                      {/* Units in fight */}
                      <div className="combat-fleet-card__units" data-testid="attacker-units-list">
                        {renderFleetUnits(attackerStats, attackerSeat, isActor, isDirectSubmitting, submitDirect)}
                      </div>
                    </div>

                    {/* Center Stage: Scorecard, Odds & Hits */}
                    <div className="combat-arena-center">
                      {/* Round Hits Scorecard */}
                      <div className="combat-round-hits-card" data-testid="combat-round-hits">
                        <div className="combat-round-hits__header">
                          Round {board?.combat?.round ?? 1} Hits Produced
                        </div>
                        <div className="combat-round-hits__grid">
                          <div className="combat-round-hits__player" data-testid="attacker-round-hits">
                            <span className="combat-round-hits__count combat-round-hits__count--attacker">
                              {attackerHitsDealt}
                            </span>
                            <span className="combat-round-hits__label">{display(attackerSeat).label}</span>
                          </div>
                          <div className="combat-round-hits__divider">vs</div>
                          <div className="combat-round-hits__player" data-testid="defender-round-hits">
                            <span className="combat-round-hits__count combat-round-hits__count--defender">
                              {defenderHitsDealt}
                            </span>
                            <span className="combat-round-hits__label">{display(defenderSeat).label}</span>
                          </div>
                        </div>
                      </div>

                      {/* Combat Odds Analysis */}
                      <div className="combat-odds-card" data-testid="combat-odds-card">
                        <div className="combat-odds-card__title">
                          <span>Combat Odds Analysis</span>
                          <span
                            className={`combat-odds-card__tag ${displayedOdds.tagClass}`}
                            data-testid="combat-odds-tag"
                          >
                            {displayedOdds.tag}
                          </span>
                        </div>
                        <div className="combat-odds-card__bars">
                          <div className="combat-odds-col">
                            <span className="combat-odds-col__pct">{displayedOdds.attWinPct}%</span>
                            <span className="combat-odds-col__exp">{displayedOdds.attSub}</span>
                          </div>
                          <div className="combat-odds-bar">
                            <div
                              className="combat-odds-bar__att"
                              style={{ width: `${displayedOdds.attWinPct}%` }}
                              title={`Attacker Win: ${displayedOdds.attWinPct}%`}
                            />
                            {displayedOdds.mutWinPct > 0 && (
                              <div
                                className="combat-odds-bar__mutual"
                                style={{ width: `${displayedOdds.mutWinPct}%` }}
                                title={`Mutual Destruction: ${displayedOdds.mutWinPct}%`}
                              />
                            )}
                            <div
                              className="combat-odds-bar__def"
                              style={{ width: `${displayedOdds.defWinPct}%` }}
                              title={`Defender Win: ${displayedOdds.defWinPct}%`}
                            />
                          </div>
                          <div className="combat-odds-col combat-odds-col--right">
                            <span className="combat-odds-col__pct">{displayedOdds.defWinPct}%</span>
                            <span className="combat-odds-col__exp">{displayedOdds.defSub}</span>
                          </div>
                        </div>
                        {displayedOdds.avgRounds && (
                          <div className="combat-odds-card__sub-detail" data-testid="combat-odds-sub-detail">
                            Avg {displayedOdds.avgRounds} rounds
                            {displayedOdds.mutWinPct > 0 ? ` • ${displayedOdds.mutWinPct}% mutual wipe` : ""}
                          </div>
                        )}
                      </div>

                      {/* Hits Remaining Banner */}
                      {hitsOwed != null && hitsOwed > 0 && (
                        <div className="combat-hits-callout" data-testid="combat-hits-callout">
                          <span className="combat-hits-callout__count">{hitsOwed}</span>
                          <span className="combat-hits-callout__label">
                            Hit{hitsOwed > 1 ? "s" : ""} to Resolve
                          </span>
                        </div>
                      )}
                    </div>

                    {/* Defender Side */}
                    <div
                      className={`combat-fleet-card ${choice?.actor === defenderSeat ? "combat-fleet-card--active" : ""}`}
                      data-testid="defender-fleet-card"
                    >
                      <div className="combat-fleet-card__header">
                        <span className="combat-fleet-card__role">DEFENDER</span>
                        <span className="combat-fleet-card__name">{display(defenderSeat).label}</span>
                        {playersMap[defenderSeat] && (
                          <span className="combat-fleet-card__faction">
                            ({playersMap[defenderSeat].faction})
                          </span>
                        )}
                        <span className="combat-fleet-card__hits" data-testid="defender-hits-dealt">
                          💥 {defenderHitsDealt} hit{defenderHitsDealt === 1 ? "" : "s"}
                        </span>
                      </div>

                      {/* Gauges: Fleet Supply and Capacity */}
                      <div className="combat-fleet-card__gauges">
                        <div
                          className="combat-gauge-stat"
                          data-alert={defenderStats.isOverFleetSupply}
                          title="Non-fighter ships vs Fleet Supply pool"
                          data-testid="defender-fleet-supply-gauge"
                        >
                          <span className="combat-gauge-stat__label">Fleet Supply:</span>
                          <span className="combat-gauge-stat__value">
                            {defenderStats.nonFighterCount} / {defenderStats.fleetSupply}
                          </span>
                        </div>
                        <div
                          className="combat-gauge-stat"
                          data-alert={defenderStats.isOverCapacity}
                          title="Capacity consumed by fighters/infantry vs total transport capacity"
                          data-testid="defender-capacity-gauge"
                        >
                          <span className="combat-gauge-stat__label">Capacity:</span>
                          <span className="combat-gauge-stat__value">
                            {defenderStats.usedCapacity} / {defenderStats.totalCapacity}
                          </span>
                        </div>
                      </div>

                      {/* Units in fight */}
                      <div className="combat-fleet-card__units" data-testid="defender-units-list">
                        {renderFleetUnits(defenderStats, defenderSeat, isActor, isDirectSubmitting, submitDirect)}
                      </div>
                    </div>
                  </div>

                  {/* Dice Results Feed */}
                  {activeDiceFeed.length > 0 && (
                    <div data-testid="combat-dice-feed" className="combat-dialog__feed">
                      <div className="combat-dialog__feed-title">Combat Rolls</div>
                      <div className="combat-dialog__dice">
                        {activeDiceFeed.map((d, i) => {
                          const isAttackerDie = d.player ? d.player === attackerSeat : undefined;
                          const isDefenderDie = d.player ? d.player === defenderSeat : undefined;
                          const sideTag = isAttackerDie ? "Attacker" : isDefenderDie ? "Defender" : undefined;
                          return (
                            <span
                              key={i}
                              data-testid="dice-roll-badge"
                              className="combat-dialog__die"
                              data-hit={d.hit}
                            >
                              {sideTag && <span className="combat-dialog__die-side">[{sideTag}] </span>}
                              {d.unit} ({d.target}+): [{d.roll}] {d.hit ? "★ HIT" : "MISS"}
                            </span>
                          );
                        })}
                      </div>
                    </div>
                  )}

                  {/* Workflow Action Controls */}
                  <div className="combat-action-area">
                    {/* Stage 1: Sustain Damage */}
                    {isActor && isSustainStage && (
                      <div className="workflow-inline">
                        <div className="combat-dialog__warning">
                          ⚠️ Caution: Opponents holding "Direct Hit" action cards may react to destroy
                          sustained ships!
                        </div>

                        {opponentHasDirectHit && (
                          <div className="combat-direct-hit-banner" data-testid="direct-hit-threat-banner">
                            ⚠️ Threat Alert: Opponent holds a "Direct Hit" action card! Any sustained ship can be immediately destroyed.
                          </div>
                        )}
                        {activeHasDirectHit && (
                          <div className="combat-direct-hit-banner" data-testid="direct-hit-held-banner">
                            🎯 Tactical Advantage: You hold "Direct Hit" in hand! You can target opponent ships if they sustain damage.
                          </div>
                        )}

                        <div className="combat-action-prompt">
                          Click 🛡️ Sustain on an eligible ship row above, or decline to take hits directly:
                        </div>

                        {/* Fallback sustain buttons when no unit rows exist on board */}
                        {(() => {
                          const activeStats = choice.actor === attackerSeat ? attackerStats : defenderStats;
                          const hasUnitRows = activeStats.groupedUnits.length > 0;
                          if (!hasUnitRows) {
                            return (
                              <div className="combat-action-buttons">
                                {choice.options
                                  .filter((o) => o.id !== "decline" && o.kind !== "decline")
                                  .map((opt) => (
                                    <button
                                      key={opt.id}
                                      type="button"
                                      data-testid={`sustain-opt-${opt.id}`}
                                      onClick={() => void submitDirect(opt.id)}
                                      disabled={isDirectSubmitting}
                                      className="button button--secondary combat-sustain-btn"
                                    >
                                      <span className="combat-btn-icon">🛡️</span>
                                      <span className="combat-btn-label">{opt.label}</span>
                                      <span className="combat-btn-sub">Sustain Hit</span>
                                    </button>
                                  ))}
                              </div>
                            );
                          }
                          return null;
                        })()}

                        {declineOption && (
                          <div className="combat-decline-row">
                            <button
                              type="button"
                              data-testid="decline-sustain-btn"
                              onClick={() => void submitDirect(declineOption.id)}
                              disabled={isDirectSubmitting}
                              className="button button--secondary"
                            >
                              {declineOption.label || "Do Not Sustain Damage"}
                            </button>
                          </div>
                        )}
                      </div>
                    )}

                    {/* Stage 2: Assign Casualty */}
                    {isActor && isCasualtyStage && (
                      <div className="workflow-inline">
                        <p className="combat-casualty-instruction">
                          {hitsOwed ? `${hitsOwed} hits remaining. ` : ""}Click a ship row above to assign a hit, or select below:
                        </p>
                        {choice.context?.target && (
                          <p className="combat-location-label">
                            Location:{" "}
                            {"System" in choice.context.target
                              ? `System ${choice.context.target.System}`
                              : "Planet" in choice.context.target
                                ? choice.context.target.Planet.planet
                                : "Combat"}
                          </p>
                        )}
                        <div className="combat-casualty-options-grid">
                          {casualtyOptions.map((opt) => (
                            <button
                              key={opt.id}
                              type="button"
                              data-testid={`casualty-opt-${opt.id}`}
                              className="button button--secondary combat-casualty-btn"
                              disabled={isDirectSubmitting}
                              onClick={() => void submitDirect(opt.id)}
                            >
                              Assign this hit: {opt.label}
                              {getCombatPayload(opt).damaged ? " (damaged)" : ""}
                            </button>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* Stage 3: Retreat */}
                    {isActor && isRetreatStage && (
                      <div className="workflow-inline">
                        <div className="combat-action-prompt">
                          {subtype === "announce_retreat"
                            ? "Choose whether to announce a retreat before combat rounds commence:"
                            : "Select an adjacent system to retreat your surviving fleet to:"}
                        </div>

                        <div className="combat-action-buttons">
                          {choice.options.map((opt) => (
                            <button
                              key={opt.id}
                              type="button"
                              data-testid={`retreat-opt-${opt.id}`}
                              onClick={() => void submitDirect(opt.id)}
                              disabled={isDirectSubmitting}
                              className="button button--secondary"
                            >
                              {opt.label}
                            </button>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                </>
              )}
            </WorkflowShell>
          ) : (
            <>
              {/* Both Sides Read-only Arena Grid */}
              <div className="combat-arena-sides-grid" data-testid="combat-arena-sides">
                <div className="combat-fleet-card" data-testid="attacker-fleet-card">
                  <div className="combat-fleet-card__header">
                    <span className="combat-fleet-card__role">ATTACKER</span>
                    <span className="combat-fleet-card__name">{display(attackerSeat).label}</span>
                    <span className="combat-fleet-card__hits" data-testid="attacker-hits-dealt">
                      💥 {attackerHitsDealt} hit{attackerHitsDealt === 1 ? "" : "s"}
                    </span>
                  </div>
                  <div className="combat-fleet-card__units" data-testid="attacker-units-list">
                    {renderFleetUnits(attackerStats, attackerSeat, false, false, async () => {})}
                  </div>
                </div>

                <div className="combat-arena-center">
                  <div className="combat-round-hits-card" data-testid="combat-round-hits">
                    <div className="combat-round-hits__header">
                      Round {board?.combat?.round ?? 1} Hits Produced
                    </div>
                    <div className="combat-round-hits__grid">
                      <div className="combat-round-hits__player" data-testid="attacker-round-hits">
                        <span className="combat-round-hits__count combat-round-hits__count--attacker">
                          {attackerHitsDealt}
                        </span>
                        <span className="combat-round-hits__label">{display(attackerSeat).label}</span>
                      </div>
                      <div className="combat-round-hits__divider">vs</div>
                      <div className="combat-round-hits__player" data-testid="defender-round-hits">
                        <span className="combat-round-hits__count combat-round-hits__count--defender">
                          {defenderHitsDealt}
                        </span>
                        <span className="combat-round-hits__label">{display(defenderSeat).label}</span>
                      </div>
                    </div>
                  </div>
                </div>

                <div className="combat-fleet-card" data-testid="defender-fleet-card">
                  <div className="combat-fleet-card__header">
                    <span className="combat-fleet-card__role">DEFENDER</span>
                    <span className="combat-fleet-card__name">{display(defenderSeat).label}</span>
                    <span className="combat-fleet-card__hits" data-testid="defender-hits-dealt">
                      💥 {defenderHitsDealt} hit{defenderHitsDealt === 1 ? "" : "s"}
                    </span>
                  </div>
                  <div className="combat-fleet-card__units" data-testid="defender-units-list">
                    {renderFleetUnits(defenderStats, defenderSeat, false, false, async () => {})}
                  </div>
                </div>
              </div>

              <div
                className="combat-spectator-waiting"
                data-testid="spectator-combat-notice"
              >
                Observing space combat in System {combatSystemId}...
              </div>
            </>
          )}
        </div>
      </Dialog.Content>
    </Dialog.Root>
  );
};
