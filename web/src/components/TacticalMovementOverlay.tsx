import React, { useState, useEffect, useMemo, useRef } from "react";
import {
  PendingChoiceDto,
  PlayerView,
  BoardView,
} from "../protocol/types.ts";
import {
  getMovementPayload,
  ChoiceRendererModel,
} from "../presentation/choiceModel.ts";
import { DecisionHeader } from "./DecisionHeader.tsx";
import {
  UnitIcon,
  getUnitDisplayName,
  getUnitBaseType,
} from "./UnitIcon.tsx";

export interface TacticalMovementOverlayProps {
  choice: PendingChoiceDto | null;
  model?: ChoiceRendererModel | null;
  board?: BoardView;
  viewerSeat?: string | null;
  activeSystemId?: string | null;
  player?: PlayerView | null;
  onSubmit: (optionId: string) => Promise<void>;
  isOpen: boolean;
  onClose: () => void;
  lastError?: string | null;
}

interface OriginShipGroup {
  originSystemId: string;
  unitType: string;
  damaged: boolean;
  totalAvailable: number;
  capacityPerUnit: number;
  isFighter: boolean;
  isGroundForce: boolean;
  options: { id: string; gravityDrive?: boolean }[];
}

interface OriginCargoGroup {
  originSystemId: string;
  unitType: string;
  source: string | null;
  damaged: boolean;
  totalAvailable: number;
}

interface ExecutionPlan {
  active: boolean;
  remainingShips: { origin: string; unitType: string; capacity: number }[];
  remainingCargo: { origin: string; unitType: string; source: string | null }[];
  currentShipOrigin: string | null;
  currentShipCapacity: number;
  currentShipLoadedCount: number;
}

export const TacticalMovementOverlay: React.FC<TacticalMovementOverlayProps> = ({
  choice,
  model,
  board,
  activeSystemId,
  player,
  onSubmit,
  isOpen,
  onClose,
  lastError,
}) => {
  // Map of key -> count to move
  const [stagedMoves, setStagedMoves] = useState<Record<string, number>>({});
  const [isDirectSubmitting, setIsDirectSubmitting] = useState(false);
  const [isExecuting, setIsExecuting] = useState(false);
  const [executionStep, setExecutionStep] = useState(0);
  const [localError, setLocalError] = useState<string | null>(null);

  const planRef = useRef<ExecutionPlan>({
    active: false,
    remainingShips: [],
    remainingCargo: [],
    currentShipOrigin: null,
    currentShipCapacity: 0,
    currentShipLoadedCount: 0,
  });

  const isSubmittingRef = useRef(false);
  const lastSubmittedNonceRef = useRef<string | null>(null);
  const choiceRef = useRef(choice);
  choiceRef.current = choice;

  const isCargoStep = choice?.context?.subtype === "load_cargo";

  const doneMovingOption = useMemo(() => {
    return (
      model?.declineOption ??
      choice?.options.find(
        (o) =>
          o.id === "done_moving" ||
          o.id === "done_loading" ||
          o.kind === "decline",
      ) ??
      null
    );
  }, [choice, model]);

  // Extract and group move options for movement_step
  const shipGroups = useMemo(() => {
    if (!choice || isCargoStep) return [] as OriginShipGroup[];

    const groupMap = new Map<string, OriginShipGroup>();

    for (const opt of choice.options) {
      if (opt.id === "done_moving" || opt.kind === "decline") continue;

      const p = getMovementPayload(opt);
      const origin = p.origin ?? "unknown";
      const unit = p.unit ?? opt.label.toLowerCase();
      const damaged = Boolean(p.damaged);
      const key = `${origin}:${unit}${damaged ? ":damaged" : ""}`;

      const baseType = getUnitBaseType(unit);
      const isFighter = baseType === "fighter";
      const isGroundForce = baseType === "infantry" || baseType === "mech";
      const capacityPerUnit = p.capacity ?? 0;

      // Determine accurate available count from boardView if present
      let availableCount = 1;
      if (board?.systems?.[origin]?.units) {
        const matchingUnits = board.systems[origin].units.filter((u) => {
          if (u.owner !== choice.actor) return false;
          if (u.planet) return false; // ships are in space
          if (Boolean(u.damaged) !== damaged) return false;
          return (
            u.unit_type.toLowerCase() === unit.toLowerCase() ||
            getUnitBaseType(u.unit_type) === baseType
          );
        });
        if (matchingUnits.length > 0) {
          availableCount = matchingUnits.length;
        }
      }

      const existing = groupMap.get(key);
      if (existing) {
        if (!board?.systems?.[origin]?.units) {
          existing.totalAvailable += 1;
        }
        existing.options.push({ id: opt.id, gravityDrive: p.gravity_drive });
      } else {
        groupMap.set(key, {
          originSystemId: origin,
          unitType: unit,
          damaged,
          totalAvailable: availableCount,
          capacityPerUnit,
          isFighter,
          isGroundForce,
          options: [{ id: opt.id, gravityDrive: p.gravity_drive }],
        });
      }
    }

    return Array.from(groupMap.values());
  }, [choice, board, isCargoStep]);

  // Discover cargo (ground forces + fighters) in origin systems where ships can move from
  const originCargoGroups = useMemo(() => {
    if (!choice || isCargoStep || !board?.systems) return [] as OriginCargoGroup[];

    const originsWithMovable = new Set<string>();
    for (const g of shipGroups) {
      if (!g.isFighter && !g.isGroundForce) {
        originsWithMovable.add(g.originSystemId);
      }
    }

    const cargoMap = new Map<string, OriginCargoGroup>();

    for (const origin of originsWithMovable) {
      const units = board.systems[origin]?.units ?? [];
      for (const u of units) {
        if (u.owner !== choice.actor) continue;
        const base = getUnitBaseType(u.unit_type);
        const isGround = base === "infantry" || base === "mech";
        const isFighter = base === "fighter";

        if (!isGround && !isFighter) continue;

        const source = u.planet ?? null;
        const damaged = Boolean(u.damaged);
        const key = `${origin}:${u.unit_type}:${source ?? "space"}${damaged ? ":damaged" : ""}`;

        const existing = cargoMap.get(key);
        if (existing) {
          existing.totalAvailable += 1;
        } else {
          cargoMap.set(key, {
            originSystemId: origin,
            unitType: u.unit_type,
            source,
            damaged,
            totalAvailable: 1,
          });
        }
      }
    }

    return Array.from(cargoMap.values());
  }, [choice, board, shipGroups, isCargoStep]);

  // Standalone load_cargo options
  const standaloneCargoGroups = useMemo(() => {
    if (!choice || !isCargoStep) return [];
    return choice.options
      .filter((o) => o.kind !== "decline" && o.id !== "done_loading")
      .map((option) => {
        const unit = String(option.payload?.unit ?? option.label);
        const source = typeof option.payload?.source === "string" ? option.payload.source : null;
        const damaged = option.payload?.damaged === true;
        const key = JSON.stringify([unit, source, damaged, option.id]);
        return { key, unit, source, damaged, option };
      });
  }, [choice, isCargoStep]);

  // Reset staging on choice nonce change only when NOT actively executing
  useEffect(() => {
    if (!planRef.current.active) {
      setStagedMoves({});
      setIsDirectSubmitting(false);
      setLocalError(null);
    }
  }, [choice?.nonce]);

  const fleetTokens = player?.fleet_tokens;

  const destinationSystemId =
    activeSystemId ??
    (model?.selectionMode.mode === "tactical_move"
      ? model.selectionMode.activeSystem
      : null) ??
    (choice?.context?.target && "System" in choice.context.target
      ? choice.context.target.System
      : null);

  // Existing non-fighter ships already in active destination system
  const existingNonFightersInDestination = useMemo(() => {
    if (!destinationSystemId || !board?.systems?.[destinationSystemId]?.units || !choice) {
      return 0;
    }
    return board.systems[destinationSystemId].units.filter((u) => {
      if (u.owner !== choice.actor || u.planet) return false;
      const base = getUnitBaseType(u.unit_type);
      return (
        base !== "fighter" &&
        base !== "infantry" &&
        base !== "mech" &&
        base !== "pds" &&
        base !== "spacedock"
      );
    }).length;
  }, [board, destinationSystemId, choice]);

  // Capacity & Fleet calculations
  const {
    totalNonFightersMoving,
    totalCapacityProvided,
    totalCargoMoving,
    capacityByOrigin,
    cargoByOrigin,
  } = useMemo(() => {
    let nonFighters = 0;
    let capacity = 0;
    let cargo = 0;
    const capByOrig: Record<string, number> = {};
    const cargoByOrig: Record<string, number> = {};

    // 1. Moving ships
    for (const g of shipGroups) {
      const key = `${g.originSystemId}:${g.unitType}${g.damaged ? ":damaged" : ""}`;
      const count = stagedMoves[key] ?? 0;
      if (count === 0) continue;

      if (!g.isFighter && !g.isGroundForce) {
        nonFighters += count;
        const shipCap = count * g.capacityPerUnit;
        capacity += shipCap;
        capByOrig[g.originSystemId] = (capByOrig[g.originSystemId] ?? 0) + shipCap;
      } else {
        cargo += count;
        cargoByOrig[g.originSystemId] = (cargoByOrig[g.originSystemId] ?? 0) + count;
      }
    }

    // 2. Staged pooled cargo from origin
    for (const c of originCargoGroups) {
      const key = `cargo:${c.originSystemId}:${c.unitType}:${c.source ?? "space"}${c.damaged ? ":damaged" : ""}`;
      const count = stagedMoves[key] ?? 0;
      if (count === 0) continue;
      cargo += count;
      cargoByOrig[c.originSystemId] = (cargoByOrig[c.originSystemId] ?? 0) + count;
    }

    // 3. Standalone cargo step
    if (isCargoStep) {
      for (const g of standaloneCargoGroups) {
        cargo += stagedMoves[g.key] ?? 0;
      }
    }

    return {
      totalNonFightersMoving: nonFighters,
      totalCapacityProvided: capacity,
      totalCargoMoving: cargo,
      capacityByOrigin: capByOrig,
      cargoByOrigin: cargoByOrig,
    };
  }, [shipGroups, originCargoGroups, standaloneCargoGroups, stagedMoves, isCargoStep]);

  const totalProjectedFleet = existingNonFightersInDestination + totalNonFightersMoving;
  const isOverFleetSupply =
    fleetTokens !== undefined && totalProjectedFleet > fleetTokens;

  const totalUnitsStaged = Object.values(stagedMoves).reduce((a, b) => a + b, 0);

  const handleUpdateCount = (key: string, delta: number, max: number) => {
    setStagedMoves((prev) => {
      const current = prev[key] ?? 0;
      const next = Math.max(0, Math.min(max, current + delta));
      return { ...prev, [key]: next };
    });
  };

  const submitFinish = async () => {
    setLocalError(null);
    if (!doneMovingOption) {
      setLocalError("Cannot finish movement: no finish option was offered.");
      return;
    }
    setIsDirectSubmitting(true);
    try {
      await onSubmit(doneMovingOption.id);
    } catch (error) {
      setLocalError(`Could not finish movement: ${String(error)}`);
    } finally {
      setIsDirectSubmitting(false);
    }
  };

  // State machine step execution
  const stepExecution = async (currentChoice: PendingChoiceDto) => {
    if (!planRef.current.active || isSubmittingRef.current) return;
    if (currentChoice.nonce === lastSubmittedNonceRef.current) return;

    const subtype = currentChoice.context?.subtype;
    if (subtype !== "movement_step" && subtype !== "load_cargo") {
      planRef.current.active = false;
      setIsExecuting(false);
      return;
    }

    if (subtype === "movement_step") {
      // Find the first staged ship that matches any offered move option
      let matchedShipIdx = -1;
      let matchedShipOpt: (typeof currentChoice.options)[0] | null = null;

      for (let i = 0; i < planRef.current.remainingShips.length; i++) {
        const ship = planRef.current.remainingShips[i];
        const opt = currentChoice.options.find((o) => {
          if (o.kind !== "move") return false;
          const p = getMovementPayload(o);
          return (
            p.origin === ship.origin &&
            (p.unit?.toLowerCase() === ship.unitType.toLowerCase() ||
              getUnitBaseType(p.unit ?? "") === getUnitBaseType(ship.unitType))
          );
        });

        if (opt) {
          matchedShipIdx = i;
          matchedShipOpt = opt;
          break;
        }
      }

      if (matchedShipOpt && matchedShipIdx !== -1) {
        const ship = planRef.current.remainingShips.splice(matchedShipIdx, 1)[0];
        planRef.current.currentShipOrigin = ship.origin;
        planRef.current.currentShipCapacity = ship.capacity;
        planRef.current.currentShipLoadedCount = 0;

        isSubmittingRef.current = true;
        lastSubmittedNonceRef.current = currentChoice.nonce;
        try {
          await onSubmit(matchedShipOpt.id);
        } catch (err) {
          console.error("[TMO catch err move]", err);
          planRef.current.active = false;
          setIsExecuting(false);
          setLocalError(err instanceof Error ? err.message : String(err));
        } finally {
          isSubmittingRef.current = false;
          setExecutionStep((s) => s + 1);
        }
        return;
      }

      // No ships left to move: conclude movement
      planRef.current.active = false;
      setIsExecuting(false);
      const doneOpt = currentChoice.options.find(
        (o) => o.id === "done_moving" || o.kind === "decline",
      );
      if (doneOpt) {
        isSubmittingRef.current = true;
        lastSubmittedNonceRef.current = currentChoice.nonce;
        try {
          await onSubmit(doneOpt.id);
        } catch (err) {
          const msg = err instanceof Error ? err.message : String(err);
          if (!msg.includes("no_pending_choice") && !msg.includes("stale_nonce")) {
            setLocalError(msg);
          }
        } finally {
          isSubmittingRef.current = false;
          setExecutionStep((s) => s + 1);
        }
      }
      return;
    }

    if (subtype === "load_cargo") {
      const origin = planRef.current.currentShipOrigin;
      const capacity = planRef.current.currentShipCapacity;
      const loaded = planRef.current.currentShipLoadedCount;

      if (loaded < capacity && origin) {
        let matchedCargoIdx = -1;
        let matchedCargoOpt: (typeof currentChoice.options)[0] | null = null;

        for (let i = 0; i < planRef.current.remainingCargo.length; i++) {
          const item = planRef.current.remainingCargo[i];
          if (item.origin !== origin) continue;

          const opt = currentChoice.options.find((o) => {
            if (o.kind !== "load") return false;
            const pUnit = String(o.payload?.unit ?? o.label);
            const pSource = typeof o.payload?.source === "string" ? o.payload.source : null;
            const matchesUnit =
              pUnit.toLowerCase() === item.unitType.toLowerCase() ||
              getUnitBaseType(pUnit) === getUnitBaseType(item.unitType);
            const matchesSource = item.source === null || pSource === item.source;
            return matchesUnit && matchesSource;
          });

          if (opt) {
            matchedCargoIdx = i;
            matchedCargoOpt = opt;
            break;
          }
        }

        if (matchedCargoOpt && matchedCargoIdx !== -1) {
          planRef.current.remainingCargo.splice(matchedCargoIdx, 1);
          planRef.current.currentShipLoadedCount += 1;
          isSubmittingRef.current = true;
          lastSubmittedNonceRef.current = currentChoice.nonce;
          try {
            await onSubmit(matchedCargoOpt.id);
          } catch (err) {
            console.error("[TMO catch err load_cargo]", err);
            planRef.current.active = false;
            setIsExecuting(false);
            setLocalError(err instanceof Error ? err.message : String(err));
          } finally {
            isSubmittingRef.current = false;
            setExecutionStep((s) => s + 1);
          }
          return;
        }
      }

      // Done loading this hold
      const doneOpt = currentChoice.options.find(
        (o) => o.id === "done_loading" || o.kind === "decline",
      );
      if (doneOpt) {
        isSubmittingRef.current = true;
        lastSubmittedNonceRef.current = currentChoice.nonce;
        try {
          await onSubmit(doneOpt.id);
        } catch (err) {
          console.error("[TMO catch err done_loading]", err);
          planRef.current.active = false;
          setIsExecuting(false);
          setLocalError(err instanceof Error ? err.message : String(err));
        } finally {
          isSubmittingRef.current = false;
          setExecutionStep((s) => s + 1);
        }
      }
      return;
    }
  };

  useEffect(() => {
    if (choice && planRef.current.active && !isSubmittingRef.current) {
      void stepExecution(choice);
    }
  }, [choice?.nonce, isExecuting, executionStep]);

  const handleCommitMoves = async () => {
    if (totalUnitsStaged === 0) {
      await submitFinish();
      return;
    }
    setLocalError(null);

    // Standalone cargo step
    if (isCargoStep) {
      const doneOpt = choice?.options.find((o) => o.id === "done_loading" || o.kind === "decline");
      if (doneOpt) {
        await onSubmit(doneOpt.id);
      }
      return;
    }

    // Collect capital ships to move
    const ships: { origin: string; unitType: string; capacity: number }[] = [];
    for (const g of shipGroups) {
      if (g.isFighter || g.isGroundForce) continue;
      const key = `${g.originSystemId}:${g.unitType}${g.damaged ? ":damaged" : ""}`;
      const count = stagedMoves[key] ?? 0;
      for (let i = 0; i < count; i++) {
        ships.push({
          origin: g.originSystemId,
          unitType: g.unitType,
          capacity: g.capacityPerUnit,
        });
      }
    }

    // Collect cargo to load
    const cargo: { origin: string; unitType: string; source: string | null }[] = [];
    for (const c of originCargoGroups) {
      const key = `cargo:${c.originSystemId}:${c.unitType}:${c.source ?? "space"}${c.damaged ? ":damaged" : ""}`;
      const count = stagedMoves[key] ?? 0;
      for (let i = 0; i < count; i++) {
        cargo.push({
          origin: c.originSystemId,
          unitType: c.unitType,
          source: c.source,
        });
      }
    }
    for (const g of shipGroups) {
      if (!g.isFighter && !g.isGroundForce) continue;
      const key = `${g.originSystemId}:${g.unitType}${g.damaged ? ":damaged" : ""}`;
      const count = stagedMoves[key] ?? 0;
      for (let i = 0; i < count; i++) {
        cargo.push({
          origin: g.originSystemId,
          unitType: g.unitType,
          source: null,
        });
      }
    }

    planRef.current = {
      active: true,
      remainingShips: ships,
      remainingCargo: cargo,
      currentShipOrigin: null,
      currentShipCapacity: 0,
      currentShipLoadedCount: 0,
    };
    setIsExecuting(true);
    if (choice) {
      stepExecution(choice);
    }
  };

  if (!isOpen || !choice) return null;

  const freeCapacityStandalone = Number(
    choice.options.find((o) => typeof o.payload?.capacity_remaining === "number")
      ?.payload?.capacity_remaining ?? 0,
  );

  return (
    <aside
      role="region"
      aria-label="Tactical Fleet Rally Tray"
      data-testid="tactical-movement-tray"
      className="fleet-rally-tray panel"
    >
      <DecisionHeader
        actor={choice.actor}
        title={isCargoStep ? "Load Cargo" : "Move Units"}
        instruction={
          isCargoStep
            ? "Choose units to carry in this ship's hold"
            : choice.prompt && choice.prompt !== "movement"
              ? choice.prompt
              : "Select ships and cargo to rally into the active system"
        }
        progress={
          destinationSystemId && !isCargoStep
            ? `Destination: system ${destinationSystemId}`
            : undefined
        }
        onMinimize={onClose}
        minimizeTestId="close-movement-tray"
      />

      {/* Gauges */}
      <div className="fleet-rally-tray__gauges">
        {!isCargoStep && (
          <div
            data-testid="fleet-supply-gauge"
            className="workflow-card"
            data-warning={isOverFleetSupply ? "true" : undefined}
          >
            <div className="workflow-card--row">
              <span className="text-muted">Fleet Supply:</span>
              <span
                className="fleet-rally-tray__status"
                data-warning={isOverFleetSupply}
                data-alert={false}
              >
                {totalProjectedFleet} / {fleetTokens ?? "unknown"} Ships
              </span>
            </div>
            {isOverFleetSupply && (
              <div className="fleet-rally-tray__advisory">
                Exceeds fleet limit ({totalProjectedFleet}/{fleetTokens}) — excess ships must be lost in combat or destroyed after movement.
              </div>
            )}
          </div>
        )}

        <div data-testid="cargo-capacity-gauge" className="workflow-card">
          <div className="workflow-card--row">
            <span className="text-muted">
              {isCargoStep ? "Free Hold Slots:" : "Cargo Capacity:"}
            </span>
            <span
              className="fleet-rally-tray__status"
              data-alert={
                isCargoStep
                  ? totalCargoMoving > freeCapacityStandalone
                  : totalCargoMoving > totalCapacityProvided
              }
            >
              {isCargoStep
                ? `${freeCapacityStandalone - totalCargoMoving} / ${freeCapacityStandalone} Free`
                : `${totalCargoMoving} / ${totalCapacityProvided} Loaded`}
            </span>
          </div>
        </div>
      </div>

      {/* Standalone cargo loading view */}
      {isCargoStep ? (
        <div className="fleet-rally-tray__list">
          {standaloneCargoGroups.length === 0 ? (
            <div className="text-muted">No units available to load into this hold.</div>
          ) : (
            standaloneCargoGroups.map((g) => {
              const count = stagedMoves[g.key] ?? 0;
              return (
                <div
                  key={g.key}
                  className="workflow-card workflow-card--row"
                  style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: "0.6rem" }}>
                    <UnitIcon type={g.unit} size={20} />
                    <div>
                      <div className="workflow-unit-name">
                        {getUnitDisplayName(g.unit)}
                        {g.damaged ? " (Damaged)" : ""}
                      </div>
                      <div className="text-muted">From {g.source ?? "space"}</div>
                    </div>
                  </div>
                  <div className="workflow-row">
                    <button
                      type="button"
                      onClick={() => handleUpdateCount(g.key, -1, freeCapacityStandalone)}
                      disabled={count <= 0 || isExecuting || isDirectSubmitting}
                      className="button button--secondary button--icon workflow-button--stepper"
                    >
                      -
                    </button>
                    <span className="workflow-count">{count}</span>
                    <button
                      type="button"
                      onClick={() => handleUpdateCount(g.key, 1, freeCapacityStandalone)}
                      disabled={
                        totalCargoMoving >= freeCapacityStandalone ||
                        isExecuting ||
                        isDirectSubmitting
                      }
                      className="button button--secondary button--icon workflow-button--stepper"
                    >
                      +
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      ) : (
        /* Standard Movement & Pooled Cargo List */
        <div className="fleet-rally-tray__list">
          {shipGroups.length === 0 ? (
            <div className="text-muted">No ships eligible to move into the active system.</div>
          ) : (
            shipGroups.map((g) => {
              const key = `${g.originSystemId}:${g.unitType}${g.damaged ? ":damaged" : ""}`;
              const count = stagedMoves[key] ?? 0;

              return (
                <div
                  key={key}
                  data-testid={`rally-row-${g.originSystemId}-${g.unitType}`}
                  className="workflow-card workflow-card--row"
                  style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: "0.6rem" }}>
                    <UnitIcon type={g.unitType} size={22} />
                    <div>
                      <div className="workflow-unit-name">
                        {getUnitDisplayName(g.unitType)}
                        {g.damaged ? " (Damaged)" : ""}
                      </div>
                      <div className="text-muted">
                        Origin: #{g.originSystemId} • Available: {g.totalAvailable}
                        {g.capacityPerUnit > 0 && ` • Capacity: ${g.capacityPerUnit}`}
                      </div>
                    </div>
                  </div>

                  <div className="workflow-row">
                    <button
                      type="button"
                      data-testid={`rally-dec-${g.originSystemId}-${g.unitType}`}
                      onClick={() => handleUpdateCount(key, -1, g.totalAvailable)}
                      disabled={count <= 0 || isExecuting || isDirectSubmitting}
                      className="button button--secondary button--icon workflow-button--stepper"
                    >
                      -
                    </button>
                    <span
                      data-testid={`rally-count-${g.originSystemId}-${g.unitType}`}
                      className="workflow-count"
                    >
                      {count}
                    </span>
                    <button
                      type="button"
                      data-testid={`rally-inc-${g.originSystemId}-${g.unitType}`}
                      onClick={() => handleUpdateCount(key, 1, g.totalAvailable)}
                      disabled={count >= g.totalAvailable || isExecuting || isDirectSubmitting}
                      className="button button--secondary button--icon workflow-button--stepper"
                    >
                      +
                    </button>
                  </div>
                </div>
              );
            })
          )}

          {/* Pooled Cargo for origins with capacity */}
          {originCargoGroups.length > 0 && (
            <div style={{ marginTop: "8px" }}>
              <div
                className="text-muted"
                style={{
                  fontSize: "0.85rem",
                  fontWeight: 600,
                  marginBottom: "4px",
                  textTransform: "uppercase",
                  letterSpacing: "0.05em",
                }}
              >
                Carryable Cargo in Origin Systems
              </div>
              {originCargoGroups.map((c) => {
                const key = `cargo:${c.originSystemId}:${c.unitType}:${c.source ?? "space"}${c.damaged ? ":damaged" : ""}`;
                const count = stagedMoves[key] ?? 0;
                const originCap = capacityByOrigin[c.originSystemId] ?? 0;
                const originCargoStaged = cargoByOrigin[c.originSystemId] ?? 0;
                const maxForThisCargo = Math.min(
                  c.totalAvailable,
                  count + Math.max(0, originCap - originCargoStaged),
                );

                return (
                  <div
                    key={key}
                    data-testid={`rally-row-${c.originSystemId}-${c.unitType}`}
                    className="workflow-card workflow-card--row"
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      opacity: originCap > 0 ? 1 : 0.6,
                    }}
                  >
                    <div style={{ display: "flex", alignItems: "center", gap: "0.6rem" }}>
                      <UnitIcon type={c.unitType} size={18} />
                      <div>
                        <div className="workflow-unit-name">
                          {getUnitDisplayName(c.unitType)}
                          {c.damaged ? " (Damaged)" : ""}
                        </div>
                        <div className="text-muted">
                          Origin: #{c.originSystemId} ({c.source ?? "Space"}) • Available: {c.totalAvailable}
                        </div>
                      </div>
                    </div>

                    <div className="workflow-row">
                      <button
                        type="button"
                        data-testid={`rally-dec-${c.originSystemId}-${c.unitType}`}
                        onClick={() => handleUpdateCount(key, -1, maxForThisCargo)}
                        disabled={count <= 0 || isExecuting || isDirectSubmitting}
                        className="button button--secondary button--icon workflow-button--stepper"
                      >
                        -
                      </button>
                      <span
                        data-testid={`rally-count-${c.originSystemId}-${c.unitType}`}
                        className="workflow-count"
                      >
                        {count}
                      </span>
                      <button
                        type="button"
                        data-testid={`rally-inc-${c.originSystemId}-${c.unitType}`}
                        onClick={() => handleUpdateCount(key, 1, maxForThisCargo)}
                        disabled={
                          count >= maxForThisCargo ||
                          originCap === 0 ||
                          isExecuting ||
                          isDirectSubmitting
                        }
                        className="button button--secondary button--icon workflow-button--stepper"
                      >
                        +
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

      {(localError || lastError) && (
        <div data-testid="movement-error-banner" role="alert" className="workflow-error">
          {localError || lastError}
        </div>
      )}

      {/* Action Footer */}
      <div className="workflow-actions">
        {totalUnitsStaged > 0 && !isExecuting && (
          <button
            type="button"
            className="button button--secondary"
            onClick={() => setStagedMoves({})}
          >
            Reset selection
          </button>
        )}
        {totalUnitsStaged > 1 && (
          <p>
            Moves and cargo loading are committed sequentially.
          </p>
        )}
        {doneMovingOption && (
          <button
            type="button"
            data-testid="finish-movement-btn"
            onClick={submitFinish}
            disabled={isExecuting || isDirectSubmitting}
            className="button button--secondary workflow-button--wide"
          >
            {doneMovingOption.label || (isCargoStep ? "Done Loading" : "Finish Movement")}
          </button>
        )}

        <button
          type="button"
          data-testid="commit-moves-btn"
          onClick={handleCommitMoves}
          disabled={isExecuting || isDirectSubmitting}
          className="button button--primary workflow-button--wide"
        >
          {isExecuting || isDirectSubmitting
            ? isCargoStep
              ? "Loading Cargo..."
              : "Moving Fleet..."
            : totalUnitsStaged > 0
              ? isCargoStep
                ? `Confirm Loads (${totalUnitsStaged})`
                : `Commit Moves (${totalUnitsStaged})`
              : isCargoStep
                ? "Done Loading"
                : "Done Moving"}
        </button>
      </div>
    </aside>
  );
};
