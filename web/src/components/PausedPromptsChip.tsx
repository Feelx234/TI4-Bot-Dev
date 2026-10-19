import React from "react";
import { usePausedPrompts } from "../hooks/promptSettings.ts";
import { openPromptsPanel } from "../hooks/promptsPanel.ts";
import "./PromptsPanel.css";

/** Shown while any prompt of this browser is switched off, so nobody forgets; opens the Prompts list. */
export const PausedPromptsChip: React.FC = () => {
  const paused = usePausedPrompts();
  if (paused.length === 0) return null;
  const names = paused.map((p) => p.label).join(", ");
  return (
    <div className="paused-prompts-chip" data-testid="paused-prompts-chip-row">
      <button
        type="button"
        className="paused-prompts-chip__button"
        data-testid="paused-prompts-chip"
        title={`Skipped for you on this browser: ${names}. Click to change.`}
        onClick={openPromptsPanel}
      >
        {paused.length === 1 ? `Prompt skipped: ${paused[0].label}` : `${paused.length} prompts skipped`}
      </button>
    </div>
  );
};
