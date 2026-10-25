import React from "react";
import type { BoardView, MovementFacts, RouteFact } from "../protocol/types.ts";
import { systemTag } from "../presentation/systemFacts.ts";
import "./RoutePicker.css";

/** What a route does on the way, as short chips: the rift dice first, they can destroy the ship. */
export interface RouteNote {
  key: string;
  text: string;
  tone: "danger" | "warning" | "info" | "good";
}

/** The anomaly kinds the map names for a tile (`BoardTileView.anomalies`), by system. */
function anomalyNames(board: BoardView | undefined, systemId: string): string[] {
  const tile = board?.map_tiles?.find((t) => t.system_id === systemId);
  return (tile?.anomalies ?? []).map((a) => a.toLowerCase());
}

/**
 * The consequences of a route, from the engine's facts (the rifts it exits, its wormhole hops,
 * the anomalies on it, what it spends, what it can load on the way). Nothing here decides a rule.
 */
export function routeNotes(
  route: RouteFact,
  facts: MovementFacts | null | undefined,
  board: BoardView | undefined,
): RouteNote[] {
  const notes: RouteNote[] = [];
  if (route.rifts.length > 0) {
    notes.push({
      key: "rift",
      text:
        route.rifts.length === 1
          ? "Rift: roll for destruction (1-3)"
          : `${route.rifts.length} rifts: roll for destruction at each (1-3)`,
      tone: "danger",
    });
  } else if (
    route.anomalies.some((id) => anomalyNames(board, id).some((n) => n.includes("rift")))
  ) {
    notes.push({ key: "rift-safe", text: "Rift: no roll", tone: "info" });
  }
  if (route.wormholes > 0) {
    notes.push({
      key: "wormhole",
      text: route.wormholes === 1 ? "Wormhole" : `${route.wormholes} wormholes`,
      tone: "info",
    });
  }
  const others = route.anomalies.filter(
    (id) => !anomalyNames(board, id).some((n) => n.includes("rift")),
  );
  if (others.length > 0) {
    const names = new Set(others.flatMap((id) => anomalyNames(board, id)));
    notes.push({
      key: "anomaly",
      text: names.size > 0 ? [...names].join(", ") : "Anomaly",
      tone: "warning",
    });
  }
  if (route.gravity_drive) {
    notes.push({ key: "gd", text: "Spends Gravity Drive", tone: "warning" });
  }
  if (route.ionian) {
    notes.push({ key: "ion", text: "Exhausts Ionian Fuel Refinery", tone: "warning" });
  }
  const pickups = new Set<string>();
  for (const index of route.loads ?? []) {
    const pool = facts?.cargo[index];
    const between = route.path.slice(1, -1);
    if (pool && between.includes(pool.system)) pickups.add(pool.system);
  }
  if (pickups.size > 0) {
    notes.push({
      key: "pickup",
      text: `Can pick up at ${[...pickups].map((id) => systemTag(id, board)).join(", ")}`,
      tone: "good",
    });
  }
  return notes;
}

export interface RoutePickerProps {
  /** The routes of one ship, the default first (`MoveFact.routes`). */
  routes: RouteFact[];
  /** The place in `routes` that is chosen. */
  selected: number;
  onSelect: (index: number) => void;
  facts?: MovementFacts | null;
  board?: BoardView;
  /** A route is not available for the staged ships, with the reason (e.g. one ship only). */
  disabledReason?: (route: RouteFact, index: number) => string | undefined;
  disabled?: boolean;
  /** Stable part of the test ids: `route-<testKey>-<n>`. */
  testKey: string;
}

/**
 * A choice between the ways one ship can take to the active system. The default route is
 * preselected and marked; every route says what it does on the way. Shown only when there is a
 * choice (more than one route).
 */
export const RoutePicker: React.FC<RoutePickerProps> = ({
  routes,
  selected,
  onSelect,
  facts,
  board,
  disabledReason,
  disabled,
  testKey,
}) => {
  if (routes.length < 2) return null;
  return (
    <div
      className="route-picker"
      role="radiogroup"
      aria-label="Route"
      data-testid={`route-picker-${testKey}`}
    >
      <div className="route-picker__title">
        Route <span className="route-picker__count">({routes.length} ways)</span>
      </div>
      {routes.map((route, index) => {
        const reason = disabledReason?.(route, index);
        const notes = routeNotes(route, facts, board);
        const checked = index === selected;
        return (
          <button
            key={route.path.join(">")}
            type="button"
            role="radio"
            aria-checked={checked}
            data-testid={`route-${testKey}-${index}`}
            data-selected={checked ? "true" : undefined}
            data-default={route.default ? "true" : undefined}
            className="route-picker__option"
            disabled={disabled || Boolean(reason)}
            title={reason}
            onClick={() => onSelect(index)}
          >
            <span className="route-picker__head">
              <span className="route-picker__radio" aria-hidden="true" />
              <span className="route-picker__path">
                {route.path.map((id, step) => (
                  <React.Fragment key={`${id}-${step}`}>
                    {step > 0 && <span className="route-picker__arrow"> › </span>}
                    <span
                      className="route-picker__system"
                      data-rift={route.rifts.includes(id) ? "true" : undefined}
                    >
                      {systemTag(id, board)}
                    </span>
                  </React.Fragment>
                ))}
              </span>
              {route.default && <span className="route-picker__default">Default</span>}
            </span>
            {notes.length > 0 && (
              <span className="route-picker__notes">
                {notes.map((note) => (
                  <span
                    key={note.key}
                    className="route-picker__note"
                    data-tone={note.tone}
                  >
                    {note.text}
                  </span>
                ))}
              </span>
            )}
            {reason && <span className="route-picker__reason">{reason}</span>}
          </button>
        );
      })}
    </div>
  );
};
