import React, { useMemo, useState } from "react";
import type { BoardView, PlayerView } from "../protocol/types.ts";
import { findStrategyCardMeta } from "../protocol/contentCatalog.ts";
import { usePlayerIdentity } from "../presentation/PlayerIdentity.tsx";
import type { StrategicAction } from "../presentation/strategicAction.ts";
import {
  STRUCTURE_UNITS,
  TRADE_WARNING,
  describePlan,
  hasTradeWarning,
  ownPlanets,
  planetName,
  researchableEstimate,
  structureName,
  techName,
  type SecondaryPlan,
} from "../presentation/secondaryPlan.ts";
import { TOKEN_POOLS, POOL_LABEL, type TokenPool } from "../presentation/commandTokens.ts";
import "./SecondaryPrep.css";

export interface SecondaryPrepPanelProps {
  action: StrategicAction;
  viewer: PlayerView | undefined;
  board: BoardView | undefined;
  plan: SecondaryPlan | null;
  onChange: (plan: SecondaryPlan) => void;
  onClear: () => void;
  /** Start expanded (screenshots and tests); by default only the chip shows. */
  defaultOpen?: boolean;
}

const MAX_LEADERSHIP_TOKENS = 3;

/**
 * "Prepare your secondary": a private, revocable suggestion for a strategy card another player is
 * resolving. Every click is saved at once; nothing is spent or sent until the engine really asks.
 */
export const SecondaryPrepPanel: React.FC<SecondaryPrepPanelProps> = ({
  action,
  viewer,
  board,
  plan,
  onChange,
  onClear,
  defaultOpen = false,
}) => {
  const display = usePlayerIdentity();
  const [open, setOpen] = useState(defaultOpen);
  const family = action.family;
  const techs = useMemo(() => (family === "technology" ? researchableEstimate(viewer) : []), [family, viewer]);
  const planets = useMemo(() => ownPlanets(board, viewer?.id), [board, viewer?.id]);
  const exhausted = planets.filter((planet) => planet.exhausted);
  const meta = findStrategyCardMeta(action.card);
  const base: SecondaryPlan = plan ?? { card: action.card, follow: true };
  const prepared = plan !== null;

  if (!open) {
    return (
      <button
        type="button"
        className="secondary-prep__chip"
        data-testid="secondary-prep-chip"
        onClick={() => setOpen(true)}
      >
        {prepared ? "Prepared" : "Prepare your secondary"}
        <span className="secondary-prep__note" style={{ margin: 0 }}>
          {prepared ? describePlan(plan) : `${action.cardName}, played by ${display(action.primary).label}`}
        </span>
      </button>
    );
  }

  const toggleTech = (tech: string) =>
    onChange({ ...base, follow: true, tech: base.tech === tech ? undefined : tech });
  const togglePlanet = (planet: string) => {
    const current = base.planets ?? [];
    const next = current.includes(planet)
      ? current.filter((entry) => entry !== planet)
      : [...current, planet].slice(-2);
    onChange({ ...base, follow: true, planets: next });
  };

  return (
    <section className="secondary-prep__card" data-testid="secondary-prep-panel" aria-label="Prepare your secondary">
      <div className="secondary-prep__head">
        <div>
          <span className="secondary-prep__eyebrow">Prepare your secondary</span>
          <span className="secondary-prep__title">
            {meta?.initiative ? `${meta.initiative}. ` : ""}
            {action.cardName}
            <span className="secondary-prep__note" style={{ display: "inline", marginLeft: 8 }}>
              played by {display(action.primary).label}
            </span>
          </span>
        </div>
        {prepared && (
          <span className="secondary-prep__badge" data-testid="secondary-prepared-badge">
            Prepared
          </span>
        )}
      </div>
      {meta?.secondaryText && <p className="secondary-prep__text">{meta.secondaryText}</p>}

      <div className="secondary-prep__row" role="group" aria-label="Follow or skip">
        <span className="secondary-prep__label">Plan</span>
        <button
          type="button"
          className="secondary-prep__choice"
          data-testid="prep-follow"
          aria-pressed={plan?.follow === true}
          onClick={() => onChange({ ...base, follow: true })}
        >
          Follow
        </button>
        <button
          type="button"
          className="secondary-prep__choice"
          data-testid="prep-skip"
          aria-pressed={plan?.follow === false}
          onClick={() => onChange({ card: action.card, follow: false })}
        >
          Skip
        </button>
      </div>

      {family === "technology" && plan?.follow && (
        <div>
          <div className="secondary-prep__label">Technology to research</div>
          <div className="secondary-prep__list" data-testid="prep-tech-list">
            {techs.length === 0 && <span className="secondary-prep__note">No technology looks researchable.</span>}
            {techs.map((tech) => (
              <button
                key={tech}
                type="button"
                className="secondary-prep__choice"
                data-testid={`prep-tech-${tech}`}
                aria-pressed={plan.tech === tech}
                onClick={() => toggleTech(tech)}
              >
                {techName(tech)}
              </button>
            ))}
          </div>
          <p className="secondary-prep__note">
            The engine chooses how the 4 resources are paid; this only names the technology.
          </p>
        </div>
      )}

      {family === "diplomacy" && plan?.follow && (
        <div>
          <div className="secondary-prep__label">Planets to ready (up to two)</div>
          <div className="secondary-prep__list" data-testid="prep-planet-list">
            {exhausted.length === 0 && <span className="secondary-prep__note">None of your planets is exhausted now.</span>}
            {exhausted.map((entry) => (
              <button
                key={entry.planet}
                type="button"
                className="secondary-prep__choice"
                data-testid={`prep-planet-${entry.planet}`}
                aria-pressed={plan.planets?.includes(entry.planet) ?? false}
                onClick={() => togglePlanet(entry.planet)}
              >
                {planetName(entry.planet)}
              </button>
            ))}
          </div>
        </div>
      )}

      {family === "construction" && plan?.follow && (
        <div>
          <div className="secondary-prep__row">
            <span className="secondary-prep__label">Structure</span>
            {STRUCTURE_UNITS.map((unit) => (
              <button
                key={unit}
                type="button"
                className="secondary-prep__choice"
                data-testid={`prep-unit-${unit}`}
                aria-pressed={plan.structure?.unit === unit}
                onClick={() => {
                  const planet = plan.structure?.planet ?? planets[0]?.planet;
                  if (planet) onChange({ ...base, follow: true, structure: { unit, planet } });
                }}
              >
                {structureName(unit)}
              </button>
            ))}
          </div>
          <div className="secondary-prep__label">On planet</div>
          <div className="secondary-prep__list" data-testid="prep-site-list">
            {planets.map((entry) => (
              <button
                key={entry.planet}
                type="button"
                className="secondary-prep__choice"
                data-testid={`prep-site-${entry.planet}`}
                aria-pressed={plan.structure?.planet === entry.planet}
                onClick={() =>
                  onChange({
                    ...base,
                    follow: true,
                    structure: { unit: plan.structure?.unit ?? "pds", planet: entry.planet },
                  })
                }
              >
                {planetName(entry.planet)}
              </button>
            ))}
          </div>
        </div>
      )}

      {family === "leadership" && plan?.follow && (
        <div className="secondary-prep__row" data-testid="prep-leadership">
          <span className="secondary-prep__label">Tokens to buy</span>
          {Array.from({ length: MAX_LEADERSHIP_TOKENS }, (_, index) => index + 1).map((count) => (
            <button
              key={count}
              type="button"
              className="secondary-prep__choice"
              data-testid={`prep-tokens-${count}`}
              aria-pressed={plan.leadership?.tokens === count}
              onClick={() =>
                onChange({
                  ...base,
                  follow: true,
                  leadership: { tokens: count, pool: plan.leadership?.pool ?? "tactic" },
                })
              }
            >
              {count} (3 influence each)
            </button>
          ))}
          <span className="secondary-prep__label">Pool</span>
          {TOKEN_POOLS.map((pool: TokenPool) => (
            <button
              key={pool}
              type="button"
              className="secondary-prep__choice"
              data-testid={`prep-pool-${pool}`}
              aria-pressed={plan.leadership?.pool === pool}
              disabled={!plan.leadership}
              onClick={() =>
                onChange({
                  ...base,
                  follow: true,
                  leadership: { tokens: plan.leadership?.tokens ?? 1, pool },
                })
              }
            >
              {POOL_LABEL[pool]}
            </button>
          ))}
        </div>
      )}

      {hasTradeWarning(action.card) && (
        <p className="secondary-prep__warning" data-testid="prep-trade-warning" role="note">
          Trade: {TRADE_WARNING}
        </p>
      )}

      <div className="secondary-prep__foot">
        <p className="secondary-prep__note" data-testid="prep-private-note">
          Private, kept only on this device. Nothing is spent or exhausted until the secondary is really
          asked, and you can still change your mind then.
        </p>
        <div className="secondary-prep__row">
          {prepared && (
            <button type="button" className="button button--secondary button--sm" data-testid="prep-clear" onClick={onClear}>
              Clear prepared
            </button>
          )}
          <button type="button" className="button button--secondary button--sm" data-testid="prep-close" onClick={() => setOpen(false)}>
            Close
          </button>
        </div>
      </div>
    </section>
  );
};
