import React from "react";
import type { MapPreviewDto } from "../protocol/types.ts";
import { deriveHexGeometry } from "../presentation/boardPresentation.ts";
import { homeIdSet, mainTiles, tileKind, viewerSeat } from "../presentation/mapPicker.ts";
import { seatStyle } from "../presentation/playerDisplay.ts";
import { TILE_FILL } from "./MapThumbnail.tsx";
import { PlanetValueGlyph, pairLabel } from "./PlanetValueIcons.tsx";

const planetLabel = (tile: MapPreviewDto["tiles"][number]) =>
  (tile.planets ?? []).map((p) => pairLabel(p.resources, p.influence)).join("; ");

/**
 * The large picture of a map: the same hex geometry as the game board, with each seat's home
 * badged by its position and faction, and the viewer's own seat ringed. Read-only.
 */
export const MapPreviewBoard: React.FC<{
  preview: MapPreviewDto;
  /** The viewer's lobby position (1-based), if they hold a seat. */
  viewerPosition?: number | null;
}> = ({ preview, viewerPosition }) => {
  const homes = homeIdSet(preview.seats);
  const seatOf = new Map(preview.seats.map((seat) => [seat.home_system_id, seat]));
  const mine = viewerSeat(preview.seats, viewerPosition);
  const placed = mainTiles(preview.tiles).map((tile) => ({
    tile,
    geometry: deriveHexGeometry(tile.q, tile.r),
  }));
  const xs = placed.map((p) => p.geometry.center.x);
  const ys = placed.map((p) => p.geometry.center.y);
  const pad = 90;
  const left = Math.min(...xs, 0) - pad;
  const top = Math.min(...ys, 0) - pad;
  const width = Math.max(...xs, 0) + pad - left;
  const height = Math.max(...ys, 0) + pad - top;
  return (
    <svg
      className="map-preview-board"
      viewBox={`${left} ${top} ${width} ${height}`}
      role="img"
      aria-label={`Map preview with ${placed.length} systems`}
      data-testid="map-preview-board"
    >
      {placed.map(({ tile, geometry }, index) => {
        const kind = tileKind(tile, homes);
        const seat = seatOf.get(tile.system_id);
        const isMine = mine !== undefined && seat?.seat === mine.seat;
        return (
          <g
            key={`${tile.system_id}-${index}`}
            data-testid={`map-tile-${tile.system_id}`}
            data-kind={kind}
            data-mine={isMine ? "true" : undefined}
          >
            <title>
              {seat
                ? `Seat ${seat.seat}: ${seat.faction_name}`
                : `${tile.label}${tile.planets?.length ? ` (${planetLabel(tile)})` : ""}`}
            </title>
            <polygon
              points={geometry.points}
              fill={TILE_FILL[kind]}
              stroke={seat ? seatStyle(seat.seat).color : "#0b1220"}
              strokeWidth={isMine ? 16 : seat ? 8 : 3}
            />
            {isMine && (
              <polygon
                points={geometry.innerPoints}
                fill="none"
                stroke="#f8fafc"
                strokeWidth={4}
                strokeDasharray="10 8"
              />
            )}
            <text
              x={geometry.center.x}
              y={geometry.center.y - (seat ? 8 : 2)}
              textAnchor="middle"
              fontSize={seat ? 40 : 24}
              fontWeight={seat ? 700 : 400}
              fill={seat ? "#0b1220" : "#cbd5e1"}
            >
              {seat ? seat.seat : tile.system_id}
            </text>
            {seat && (
              <text
                x={geometry.center.x}
                y={geometry.center.y + 26}
                textAnchor="middle"
                fontSize="20"
                fontWeight={600}
                fill="#0b1220"
              >
                {seat.faction.toUpperCase()}
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
    </svg>
  );
};
