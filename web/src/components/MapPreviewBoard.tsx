import React from "react";
import type { MapPreviewDto } from "../protocol/types.ts";
import { deriveHexGeometry } from "../presentation/boardPresentation.ts";
import { homeIdSet, mainTiles, tileKind } from "../presentation/mapPicker.ts";
import { seatStyle } from "../presentation/playerDisplay.ts";
import { TILE_FILL } from "./MapThumbnail.tsx";
import { HyperlaneLinks, HyperlaneNetwork, connectionId } from "./hyperlane/HyperlaneArt.tsx";
import { connectionsAt, getHyperlaneNetwork, linkSentence, tileKey } from "./hyperlane/hyperlaneNetwork.ts";
import { hyperlaneLabel, parseHyperlaneId } from "./hyperlane/hyperlaneGeometry.ts";
import { PlanetValueGlyph, pairLabel } from "./PlanetValueIcons.tsx";

const planetLabel = (tile: MapPreviewDto["tiles"][number]) =>
  (tile.planets ?? []).map((p) => pairLabel(p.resources, p.influence)).join("; ");

/**
 * The large picture of a map: the same hex geometry as the game board, with the place of each
 * seat badged by its number. Who sits where, and with which faction, is drawn when the game
 * starts, so a place names no player and no faction. Read-only.
 */
export const MapPreviewBoard: React.FC<{
  preview: MapPreviewDto;
}> = ({ preview }) => {
  const homes = homeIdSet(preview.seats);
  const seatOf = new Map(preview.seats.map((seat) => [seat.home_system_id, seat]));
  const placed = mainTiles(preview.tiles).map((tile) => ({
    tile,
    geometry: deriveHexGeometry(tile.q, tile.r),
  }));
  const model = getHyperlaneNetwork(placed.map(({ tile }) => ({ systemId: tile.system_id, q: tile.q, r: tile.r })));
  const netTiles = placed.map(({ tile, geometry }) => ({
    systemId: tile.system_id,
    q: tile.q,
    r: tile.r,
    cx: geometry.center.x,
    cy: geometry.center.y,
  }));
  // Hover, focus or tap on a linked system (or on a lane tile) lights its routes.
  const [hot, setHot] = React.useState<string | null>(null);
  const [hotPair, setHotPair] = React.useState<string | null>(null);
  const hotList = hotPair
    ? model.connections.filter((c) => connectionId(c) === hotPair)
    : connectionsAt(model, hot);
  const partnerKeys = new Set(hotList.flatMap((c) => [c.fromKey, c.toKey]));
  const xs = placed.map((p) => p.geometry.center.x);
  const ys = placed.map((p) => p.geometry.center.y);
  const pad = 90;
  const left = Math.min(...xs, 0) - pad;
  const top = Math.min(...ys, 0) - pad;
  const width = Math.max(...xs, 0) + pad - left;
  const height = Math.max(...ys, 0) + pad - top;
  return (
    <>
    <svg
      className="map-preview-board"
      viewBox={`${left} ${top} ${width} ${height}`}
      role="img"
      aria-label={`Map preview with ${placed.length} systems${model.connections.length ? `, ${model.connections.length} hyperlane connections` : ""}`}
      data-testid="map-preview-board"
    >
      {placed.map(({ tile, geometry }, index) => {
        const kind = tileKind(tile, homes);
        const seat = seatOf.get(tile.system_id);
        const key = tileKey(tile.q, tile.r);
        const link = kind === "hyperlane" ? null : linkSentence(model, key);
        const interactive = model.connections.length > 0 && (link !== null || kind === "hyperlane");
        return (
          <g
            key={`${tile.system_id}-${index}`}
            data-testid={`map-tile-${tile.system_id}`}
            data-kind={kind}
            data-linked={link ? "true" : undefined}
            tabIndex={link ? 0 : undefined}
            onMouseEnter={interactive ? () => setHot(key) : undefined}
            onMouseLeave={interactive ? () => setHot(null) : undefined}
            onFocus={link ? () => setHot(key) : undefined}
            onBlur={link ? () => setHot(null) : undefined}
            onClick={interactive ? () => setHot((h) => (h === key ? null : key)) : undefined}
          >
            <title>
              {seat
                ? `Seat ${seat.seat}${link ? `. ${link}` : ""}`
                : kind === "hyperlane"
                  ? (() => {
                      const lane = parseHyperlaneId(tile.system_id);
                      return lane ? hyperlaneLabel(lane) : tile.label;
                    })()
                  : `${tile.label}${tile.planets?.length ? ` (${planetLabel(tile)})` : ""}${link ? `. ${link}` : ""}`}
            </title>
            <polygon
              points={geometry.points}
              fill={TILE_FILL[kind]}
              stroke={partnerKeys.has(key) ? "#f59e0b" : seat ? seatStyle(seat.seat).color : "#0b1220"}
              strokeWidth={partnerKeys.has(key) ? 9 : seat ? 8 : 3}
            />
            {kind !== "hyperlane" && (
            <text
              x={geometry.center.x}
              y={geometry.center.y + (seat ? 12 : -2)}
              textAnchor="middle"
              fontSize={seat ? 40 : 24}
              fontWeight={seat ? 700 : 400}
              fill={seat ? "#0b1220" : "#cbd5e1"}
            >
              {seat ? seat.seat : tile.system_id}
            </text>
            )}
            {!seat && tile.planets && tile.planets.length > 0 && (
              <g aria-hidden="true">
                {tile.planets.map((planet, i) => {
                  const y = geometry.center.y + 12 + i * 18;
                  const x = geometry.center.x - 26;
                  return (
                    <g key={i} data-testid={`map-planet-values-${tile.system_id}-${i}`}>
                      <PlanetValueGlyph kind="resources" x={x} y={y} size={14} />
                      <text x={x + 17} y={y + 12} fontSize="14" fontWeight="bold" fill="#fef08a">
                        {planet.resources}
                      </text>
                      <PlanetValueGlyph kind="influence" x={x + 33} y={y} size={14} />
                      <text x={x + 50} y={y + 12} fontSize="14" fontWeight="bold" fill="#bae6fd">
                        {planet.influence}
                      </text>
                    </g>
                  );
                })}
              </g>
            )}
          </g>
        );
      })}
      <HyperlaneNetwork
        tiles={netTiles}
        model={model}
        weight={1.2}
        bed={TILE_FILL.hyperlane}
        hotConnections={hotList}
      />
      <HyperlaneLinks
        tiles={netTiles}
        model={model}
        weight={1.2}
        hot={hotList}
      />
    </svg>
    {model.connections.length > 0 && (
      <div className="map-preview-links" data-testid="hyperlane-link-legend" style={{ fontSize: 12, color: "var(--color-text-muted, #94a3b8)" }}>
        <p style={{ margin: "0 0 4px", display: "flex", gap: 12, flexWrap: "wrap", alignItems: "center" }}>
          <span aria-hidden="true">
            <svg width="26" height="10" viewBox="0 0 26 10" style={{ verticalAlign: "middle" }}>
              <line x1="1" y1="5" x2="25" y2="5" stroke="#c4b5fd" strokeWidth="7" strokeLinecap="round" />
              <line x1="1" y1="5" x2="25" y2="5" stroke="#1e1b4b" strokeWidth="3" strokeLinecap="round" />
              <line x1="3" y1="5" x2="23" y2="5" stroke="#fde68a" strokeWidth="1.5" strokeDasharray="3 3" />
            </svg>{" "}
            Lane route
          </span>
          <span aria-hidden="true">
            <svg width="26" height="10" viewBox="0 0 26 10" style={{ verticalAlign: "middle" }}>
              <path d="M3 8Q13 -2 23 8" stroke="#fde68a" strokeWidth="1.5" fill="none" />
              <circle cx="3" cy="8" r="2.5" fill="#fde68a" />
              <circle cx="23" cy="8" r="2.5" fill="#fde68a" />
            </svg>{" "}
            Linked pair
          </span>
          <span>Point at a system, or a pair below, to light its route.</span>
        </p>
        <details>
          <summary>
            {model.connections.length} linked pairs
          </summary>
          <ul style={{ listStyle: "none", margin: "6px 0 0", padding: 0, display: "flex", flexWrap: "wrap", gap: 4 }}>
            {model.connections.map((c) => {
              const id = connectionId(c);
              return (
                <li key={id}>
                  <button
                    type="button"
                    data-testid="hyperlane-pair"
                    data-pair={id}
                    aria-pressed={hotPair === id}
                    aria-label={`Systems ${c.fromId} and ${c.toId}, linked through ${c.tiles.length} lane tile${c.tiles.length === 1 ? "" : "s"}`}
                    onMouseEnter={() => setHotPair(id)}
                    onMouseLeave={() => setHotPair(null)}
                    onFocus={() => setHotPair(id)}
                    onBlur={() => setHotPair(null)}
                    onClick={() => setHotPair((h) => (h === id ? null : id))}
                    style={{
                      font: "inherit",
                      padding: "6px 8px",
                      minHeight: 32,
                      borderRadius: 6,
                      border: `1px solid ${hotPair === id ? "#f59e0b" : "var(--color-border-muted, #334155)"}`,
                      background: "transparent",
                      color: "inherit",
                      cursor: "pointer",
                    }}
                  >
                    {c.fromId} – {c.toId}
                  </button>
                </li>
              );
            })}
          </ul>
        </details>
      </div>
    )}
    </>
  );
};
