import React from "react";
import { usePausedPrompts, usePromptModes } from "../hooks/promptSettings.ts";
import { openPromptsPanel } from "../hooks/promptsPanel.ts";
import "./PromptsPanel.css";

/** Shown while any prompt of this browser is switched off (or off in other players' turns), so nobody forgets; opens the Prompts list. */
export const PausedPromptsChip: React.FC = () => {
  const paused = usePausedPrompts();
  const modes = usePromptModes();
  if (paused.length === 0) return null;
  const names = paused.map((p) => (modes.get(p.id) === "ownturn" ? `${p.label} (only in your turn)` : p.label)).join(", ");
  const only = paused[0];
  const text =
    paused.length > 1
      ? `${paused.length} prompts skipped`
      : modes.get(only.id) === "ownturn"
        ? `Prompt skipped in others' turns: ${only.label}`
        : `Prompt skipped: ${only.label}`;
  return (
    <div className="paused-prompts-chip" data-testid="paused-prompts-chip-row">
      <button
        type="button"
        className="paused-prompts-chip__button"
        data-testid="paused-prompts-chip"
        title={`Skipped for you on this browser: ${names}. Click to change.`}
        onClick={openPromptsPanel}
      >
        {text}
      </button>
    </div>
  );
};
