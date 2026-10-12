import React, { useEffect, useId, useRef, useState } from "react";
import {
  FACTION_HUE,
  factionMonogram,
  lobbyFactionName,
} from "../presentation/lobbyFactions.ts";

export const FactionSigil: React.FC<{ faction: string | null }> = ({ faction }) => (
  <span
    className="faction-sigil"
    aria-hidden="true"
    style={faction ? { background: FACTION_HUE[faction] ?? "#94a3b8" } : undefined}
  >
    {faction ? factionMonogram(faction) : "?"}
  </span>
);

interface FactionPickerProps {
  /** Factions the server offers, in its order. */
  pool: readonly string[];
  /** The chosen faction, or `null` for Random. */
  value: string | null;
  /** Faction id -> who holds it; those options are disabled and say who. */
  taken?: ReadonlyMap<string, { nickname: string | null }>;
  onChange: (faction: string | null) => void;
  disabled?: boolean;
  label: string;
  testId: string;
  compact?: boolean;
}

/** A closed button showing the choice; opens a list of Random and every faction in the pool. */
export const FactionPicker: React.FC<FactionPickerProps> = ({
  pool,
  value,
  taken,
  onChange,
  disabled,
  label,
  testId,
  compact,
}) => {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const listId = useId();
  useEffect(() => {
    if (!open) return;
    const away = (event: Event) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);
  const choose = (faction: string | null) => {
    setOpen(false);
    if (faction !== value) onChange(faction);
  };
  return (
    <div
      className={`faction-picker${compact ? " faction-picker--compact" : ""}`}
      ref={root}
      data-testid={testId}
    >
      <button
        type="button"
        className="button button--outline faction-picker__button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-label={`${label}: ${value ? lobbyFactionName(value) : "Random"}`}
        disabled={disabled}
        data-testid={`${testId}-button`}
        onClick={() => setOpen((was) => !was)}
      >
        <FactionSigil faction={value} />
        <span className="faction-picker__name">{value ? lobbyFactionName(value) : "Random"}</span>
        <span aria-hidden="true">▾</span>
      </button>
      {open && (
        <ul className="faction-picker__list" role="listbox" id={listId} aria-label={label}>
          {[null, ...pool].map((faction) => {
            const holder = faction ? taken?.get(faction) : undefined;
            return (
              <li
                key={faction ?? "random"}
                role="option"
                aria-selected={faction === value}
                aria-disabled={!!holder}
                tabIndex={holder ? -1 : 0}
                className={`faction-picker__option${faction === value ? " is-selected" : ""}${holder ? " is-taken" : ""}`}
                data-testid={`${testId}-option-${faction ?? "random"}`}
                onClick={() => !holder && choose(faction)}
                onKeyDown={(event) => {
                  if (!holder && (event.key === "Enter" || event.key === " ")) {
                    event.preventDefault();
                    choose(faction);
                  }
                }}
              >
                <FactionSigil faction={faction} />
                <span>{faction ? lobbyFactionName(faction) : "Random"}</span>
                {holder && (
                  <span className="faction-picker__taken">
                    taken by {holder.nickname ?? "another seat"}
                  </span>
                )}
                {!faction && <span className="faction-picker__taken">dealt by the seed</span>}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
};
